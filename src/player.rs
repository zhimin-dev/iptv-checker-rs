//! 播放器 API 与「流畅播放中继（服务器缓冲）」会话管理。
//!
//! 功能：
//! 1. GET /api/player/channels          —— 输出服务端当前已有的频道列表（JSON）
//! 2. POST /api/player/relay/start     —— 服务端接管源链接：ffmpeg 拉流并切片缓存到本地
//! 3. GET /api/player/relay/{sid}/{file} —— 播放服务端本地生成的 HLS（index.m3u8 / 分片）
//! 4. GET /api/player/relay/{sid}/status —— 查询中继会话状态
//! 5. DELETE /api/player/relay/{sid}   —— 停止中继会话
//! 6. GET /api/player/relay            —— 列出所有中继会话
//!
//! 原理：当客户端直连源站播放卡顿时，可以让服务端用 ffmpeg 持续把源流
//! （HLS/m3u8、mp4、rtmp 等）拉到服务器本地，切成 HLS 分片缓存在磁盘上。
//! 客户端改为播放服务端本地的 playlist，由于数据已经完整落在服务端，
//! 客户端只需要和服务端之间维持稳定的内网/短距离连接，即可保证播放不卡顿。
//! 代价是画面相比直播源会有几秒到几十秒的延迟（可接受）。

use crate::common::m3u::m3u::list_str2obj;
use crate::common::m3u::SearchOptions;
use crate::epg_mapping::get_best_tvg_id;
use crate::r#const::constant::{INPUT_SEARCH_FOLDER, PLAYER_FOLDER};
use actix_files::NamedFile;
use actix_web::{delete, get, post, web, HttpRequest, HttpResponse, Responder};
use chrono::Local;
use lazy_static::lazy_static;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use futures::StreamExt;

const MAX_SESSIONS: usize = 16;
/// 桌面端会话：超过该秒数未收到心跳即自动停止（手动添加的会话不受影响）
const HEARTBEAT_TIMEOUT_SECS: u64 = 60;
const CLEAN_INTERVAL_SECS: u64 = 30;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ============================== 频道列表 ==============================

/// 播放器频道条目（JSON 输出给客户端）
#[derive(Serialize, Debug, Clone)]
pub struct PlayerChannel {
    pub id: String,
    pub name: String,
    pub group: String,
    pub logo: String,
    pub url: String,
    pub epg_id: String,
    /// 源站要求的 User-Agent（直连播放时浏览器无法设置，仅中继模式可用）
    pub user_agent: String,
}

#[derive(Deserialize)]
pub struct PlayerChannelQuery {
    #[serde(default = "default_channel_filter")]
    pub filter: String,
    /// 频道源：all=全部爬取 | like=喜欢 | checked=定时检查过的（兼容旧 filter 参数）
    #[serde(default)]
    pub source: String,
    /// refresh=1 时强制重新生成（绕过缓存）
    #[serde(default)]
    pub refresh: String,
}

fn default_channel_filter() -> String {
    "all".to_string()
}

/// 获取服务端当前已有的频道列表（来自今日搜索数据）。
/// filter: all | like（应用收藏关键词过滤）
pub async fn build_player_channels(filter: &str) -> Result<Vec<PlayerChannel>, String> {
    let today = Local::now().format("%Y%m%d").to_string();
    let search_path = format!("{}/{}", INPUT_SEARCH_FOLDER, today);

    let mut files = Vec::new();
    let dir_entries = std::fs::read_dir(&search_path)
        .map_err(|e| format!("search data not ready: {}", e))?;
    for entry in dir_entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(p) = path.to_str() {
                files.push(p.to_string());
            }
        }
    }
    if files.is_empty() {
        return Err("server has no channel data yet, run a search task first".to_string());
    }

    let bodies = crate::common::m3u::m3u::from_arr(files, 0).await;
    let mut data = list_str2obj(bodies, false);
    data.t2s();
    data.remove_useless_char();
    if filter == "like" {
        let keyword_full_match = crate::config::favourite::get_favourite_list("equal");
        let keyword_like = crate::config::favourite::get_favourite_list("like");
        data.search(SearchOptions {
            keyword_full_match,
            keyword_like,
            keyword_dislike: vec![],
            ipv4: false,
            ipv6: false,
            exclude_url: vec![],
            exclude_host: vec![],
        })
        .await;
    }

    let mut seen = HashSet::new();
    let mut channels = Vec::new();
    for obj in data.get_list() {
        let url = obj.get_url();
        if url.trim().is_empty() {
            continue;
        }
        // 相同 url 去重（多个源文件中可能重复出现）
        if !seen.insert(url.clone()) {
            continue;
        }
        let name = obj.get_display_name().to_string();
        let ext = obj.get_extend();
        let (group, logo, tv_name, user_agent) = match ext {
            Some(e) => (e.group_title, e.tv_logo, e.tv_name, e.user_agent),
            None => (String::new(), String::new(), String::new(), String::new()),
        };
        let epg_id =
            get_best_tvg_id(if tv_name.is_empty() { None } else { Some(&tv_name) }, &name);
        let id = format!("{:x}", md5::compute(url.as_bytes()));
        channels.push(PlayerChannel {
            id,
            name,
            group,
            logo,
            url,
            epg_id,
            user_agent,
        });
    }
    channels.sort_by(|a, b| a.group.cmp(&b.group).then_with(|| a.name.cmp(&b.name)));
    Ok(channels)
}

/// 获取定时检查任务中「检查成功」的频道列表（static/output/*.json）
pub async fn build_checked_channels() -> Result<Vec<PlayerChannel>, String> {
    let dir_entries = std::fs::read_dir(crate::r#const::constant::OUTPUT_FOLDER)
        .map_err(|e| format!("checked data not ready: {}", e))?;
    let mut files = Vec::new();
    for entry in dir_entries.flatten() {
        let p = entry.path();
        if p.extension().map(|e| e == "json").unwrap_or(false) {
            if let Some(s) = p.to_str() {
                files.push(s.to_string());
            }
        }
    }
    if files.is_empty() {
        return Err(
            "no checked channel data yet, run a check task on the server first".to_string(),
        );
    }
    let mut seen = HashSet::new();
    let mut channels = Vec::new();
    for f in files {
        let content =
            std::fs::read_to_string(&f).map_err(|e| format!("read {} failed: {}", f, e))?;
        let m3u: crate::common::M3uObjectList = match serde_json::from_str(&content) {
            Ok(m) => m,
            Err(_) => continue,
        };
        for obj in m3u.get_list() {
            // 只保留检查成功的频道
            if obj.get_status() != crate::common::CheckDataStatus::Success {
                continue;
            }
            let url = obj.get_url();
            if url.trim().is_empty() || !seen.insert(url.clone()) {
                continue;
            }
            let name = obj.get_display_name().to_string();
            let ext = obj.get_extend();
            let (group, logo, tv_name, tv_id, user_agent) = match ext {
                Some(e) => (e.group_title, e.tv_logo, e.tv_name, e.tv_id, e.user_agent),
                None => (
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                ),
            };
            // 检查数据里的 tv_id 是精确匹配的 EPG id，优先使用
            let epg_id = if !tv_id.is_empty() {
                tv_id
            } else {
                get_best_tvg_id(if tv_name.is_empty() { None } else { Some(&tv_name) }, &name)
            };
            let id = format!("{:x}", md5::compute(url.as_bytes()));
            channels.push(PlayerChannel {
                id,
                name,
                group,
                logo,
                url,
                epg_id,
                user_agent,
            });
        }
    }
    channels.sort_by(|a, b| a.group.cmp(&b.group).then_with(|| a.name.cmp(&b.name)));
    Ok(channels)
}

// ============================== 频道列表缓存 ==============================

struct ChannelCacheEntry {
    list: Vec<PlayerChannel>,
    built_at: u64,
}

lazy_static! {
    static ref CHANNEL_CACHE: Mutex<HashMap<String, ChannelCacheEntry>> =
        Mutex::new(HashMap::new());
}

/// 获取频道缓存有效期（秒），0 表示不缓存
pub fn get_channel_cache_ttl_secs() -> u64 {
    (crate::config::base::get_base_config().player_cache_ttl_hours as u64).saturating_mul(3600)
}

/// 清空频道列表缓存
pub fn clear_channel_cache() {
    CHANNEL_CACHE.lock().unwrap().clear();
    info!("channel cache cleared");
}

/// 带缓存获取频道列表：缓存未过期直接返回，否则重新解析生成
/// source: all | like | checked
/// 返回 (列表, 是否来自缓存, 生成时间戳)
pub async fn get_player_channels_cached(
    source: &str,
    force_refresh: bool,
) -> Result<(Vec<PlayerChannel>, bool, u64), String> {
    let ttl_secs = get_channel_cache_ttl_secs();
    if !force_refresh && ttl_secs > 0 {
        let hit = {
            let map = CHANNEL_CACHE.lock().unwrap();
            map.get(source)
                .filter(|c| now_secs().saturating_sub(c.built_at) < ttl_secs)
                .map(|c| (c.list.clone(), c.built_at))
        };
        if let Some((list, at)) = hit {
            return Ok((list, true, at));
        }
    }
    let list = match source {
        "like" => build_player_channels("like").await?,
        "checked" => build_checked_channels().await?,
        _ => build_player_channels("all").await?,
    };
    let at = now_secs();
    if ttl_secs > 0 {
        CHANNEL_CACHE
            .lock()
            .unwrap()
            .insert(source.to_string(), ChannelCacheEntry { list: list.clone(), built_at: at });
    }
    Ok((list, false, at))
}

#[get("/api/player/channels")]
async fn player_channels(q: web::Query<PlayerChannelQuery>) -> impl Responder {
    let force = q.refresh == "1" || q.refresh.eq_ignore_ascii_case("true");
    // source 参数优先，兼容旧的 filter 参数（all|like）
    let source = if !q.source.is_empty() {
        q.source.clone()
    } else {
        q.filter.clone()
    };
    match get_player_channels_cached(&source, force).await {
        Ok((list, cached, cached_at)) => {
            let total = list.len();
            HttpResponse::Ok().json(serde_json::json!({
                "list": list,
                "total": total,
                "source": source,
                "cached": cached,
                "cached_at": cached_at,
                "ttl_hours": crate::config::base::get_base_config().player_cache_ttl_hours,
                "generated_at": now_secs(),
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({ "msg": e })),
    }
}

/// 手动清除频道列表缓存
#[delete("/api/player/channels/cache")]
async fn clear_channel_cache_api() -> impl Responder {
    clear_channel_cache();
    HttpResponse::Ok().json(serde_json::json!({ "msg": "channel cache cleared" }))
}

/// 缓存配置（TTL 小时数）
#[derive(Deserialize)]
pub struct CacheConfigReq {
    pub ttl_hours: u64,
}

#[get("/api/player/cache-config")]
async fn get_cache_config_api() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "ttl_hours": crate::config::base::get_base_config().player_cache_ttl_hours,
    }))
}

#[post("/api/player/cache-config")]
async fn set_cache_config_api(req: web::Json<CacheConfigReq>) -> impl Responder {
    let ttl = req.ttl_hours.min(24 * 30);
    let mut cfg = crate::config::base::get_base_config();
    cfg.player_cache_ttl_hours = ttl as u32;
    match crate::config::base::update_base_config(cfg) {
        Ok(_) => {
            clear_channel_cache();
            HttpResponse::Ok().json(serde_json::json!({ "ttl_hours": ttl, "msg": "success" }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({ "msg": e })),
    }
}

// ============================== HLS 中继会话 ==============================

struct RelaySession {
    sid: String,
    url: String,
    folder: std::path::PathBuf,
    child: Arc<Mutex<tokio::process::Child>>,
    started_at: u64,
    last_active: Arc<AtomicU64>,
    /// 最后一次收到播放心跳的时间（仅桌面端会话需要，手动会话不受限）
    last_heartbeat: Arc<AtomicU64>,
    hls_time: u32,
    keep_segments: u32,
    /// true = 后台手动添加的 m3u8 会话（永不自动停止，需手动停止）；
    /// false = 桌面端启动的会话（60 秒无心跳自动停止）
    manual: bool,
}

lazy_static! {
    static ref RELAY_SESSIONS: Mutex<HashMap<String, Arc<RelaySession>>> =
        Mutex::new(HashMap::new());
    /// 正在启动中的会话 sid（目录已创建、尚未注册进 RELAY_SESSIONS），
    /// 防止清理任务把启动过程中的会话目录误删
    static ref RELAY_STARTING: Mutex<std::collections::HashSet<String>> =
        Mutex::new(std::collections::HashSet::new());
}

#[derive(Deserialize)]
pub struct RelayStartReq {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default = "default_hls_time")]
    pub hls_time: u32,
    #[serde(default = "default_keep_segments")]
    pub keep_segments: u32,
    /// 手动添加（后台）的会话标记：true 时不自动停止
    #[serde(default)]
    pub manual: bool,
}

fn default_hls_time() -> u32 {
    let v = crate::config::base::get_base_config().relay_hls_time;
    if v == 0 {
        4
    } else {
        v.clamp(2, 30)
    }
}

fn default_keep_segments() -> u32 {
    let v = crate::config::base::get_base_config().relay_keep_segments;
    if v == 0 {
        30
    } else {
        v.clamp(3, 60)
    }
}

#[derive(Serialize)]
pub struct RelayStartResp {
    pub sid: String,
    pub playlist_url: String,
    pub hls_time: u32,
    pub keep_segments: u32,
    pub manual: bool,
    pub msg: String,
}

/// 启动一个中继会话：服务端用 ffmpeg 把源流下载并切片到本地磁盘
pub async fn start_relay(
    url: String,
    mut headers: HashMap<String, String>,
    hls_time: u32,
    keep_segments: u32,
    manual: bool,
) -> Result<RelayStartResp, String> {
    if !(url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("rtmp")
        || url.starts_with("rtsp"))
    {
        return Err("invalid stream url".to_string());
    }
    // 兜底：未指定 UA 时使用浏览器 UA（大量 IPTV 源会拒绝非浏览器 UA）
    if !headers.contains_key("User-Agent") {
        headers.insert(
            "User-Agent".to_string(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36"
                .to_string(),
        );
    }
    let hls_time = hls_time.clamp(2, 30);
    let keep_segments = keep_segments.clamp(3, 60);

    let sid = uuid::Uuid::new_v4().simple().to_string();
    let folder = std::path::PathBuf::from(format!("{}{}", PLAYER_FOLDER, sid));
    std::fs::create_dir_all(&folder).map_err(|e| format!("create session folder failed: {}", e))?;
    // 标记“启动中”，防止后台清理任务误删刚创建、尚未注册的会话目录
    RELAY_STARTING.lock().unwrap().insert(sid.clone());

    // 优先使用项目自带的 ffmpeg（tools/ffmpeg/ffmpeg.exe），
    // 部分 IPTV 源与系统新版 ffmpeg 存在兼容问题（如 mp2/mp3 流变化）
    let ffmpeg_bin = if std::path::Path::new("./tools/ffmpeg/ffmpeg.exe").exists() {
        "./tools/ffmpeg/ffmpeg.exe"
    } else if std::path::Path::new("./tools/ffmpeg/ffmpeg").exists() {
        "./tools/ffmpeg/ffmpeg"
    } else {
        "ffmpeg"
    };
    let mut cmd = tokio::process::Command::new(ffmpeg_bin);
    // 中继拉流遵循网络代理配置（系统代理 / 后台配置的代理）
    crate::common::util::apply_proxy_to_command(&mut cmd);
    cmd.arg("-hide_banner")
        .arg("-loglevel")
        .arg("warning")
        .arg("-y")
        // 网络读写超时 15s：源站卡死时自动重连，保证中继本身足够健壮
        .arg("-rw_timeout")
        .arg("15000000")
        // 断线自动重连（IPTV 源网络抖动很常见）
        .arg("-reconnect")
        .arg("1")
        .arg("-reconnect_streamed")
        .arg("1")
        .arg("-reconnect_delay_max")
        .arg("5");
    if !headers.is_empty() {
        let header_str = headers
            .iter()
            .map(|(k, v)| format!("{}: {}\r\n", k, v))
            .collect::<String>();
        cmd.arg("-headers").arg(header_str);
    }
    cmd.arg("-i")
        .arg(&url)
        // 只保留首条视频/音频流（音频可选），避免多音轨导致播放器异常
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a:0?")
        // 音频统一转 AAC：部分源音频编码会中途变化（如 mp3->mp2）
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("128k");
    // 视频编码：h264 直接复制；其他编码（HEVC 等浏览器无法解码）转码为 h264，
    // 避免出现「只有声音没有画面」的问题
    let mut need_transcode = false;
    if url.starts_with("http") {
        let codec = probe_video_codec(&url).await;
        if let Some(c) = codec.as_deref() {
            need_transcode = c != "h264";
            if need_transcode {
                info!("relay: source video codec '{}' is not h264, transcoding to h264", c);
            }
        }
    }
    if need_transcode {
        cmd.arg("-c:v")
            .arg("libx264")
            .arg("-preset")
            .arg("veryfast")
            .arg("-crf")
            .arg("23")
            .arg("-maxrate")
            .arg("3000k")
            .arg("-bufsize")
            .arg("6000k");
    } else {
        cmd.arg("-c:v")
            .arg("copy");
    }
    cmd
        .arg("-f")
        .arg("hls")
        .arg("-hls_time")
        .arg(hls_time.to_string())
        .arg("-hls_list_size")
        .arg(keep_segments.to_string())
        .arg("-hls_flags")
        .arg("delete_segments")
        .arg("-hls_segment_filename")
        .arg(folder.join("segment_%05d.ts"))
        .arg(folder.join("index.m3u8"))
        .stdout(Stdio::null())
        .stderr(Stdio::from(
            std::fs::File::create(folder.join("ffmpeg.log"))
                .map_err(|e| format!("create log file failed: {}", e))?,
        ));

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&folder);
            RELAY_STARTING.lock().unwrap().remove(&sid);
            if e.kind() == std::io::ErrorKind::NotFound {
                return Err(
                    "ffmpeg not found on server, install ffmpeg to enable relay mode".to_string(),
                );
            }
            return Err(format!("spawn ffmpeg failed: {}", e));
        }
    };

    // 会话数达到上限时，淘汰最久没有心跳的桌面端会话（手动添加的会话永不淘汰）
    let evict = {
        let map = RELAY_SESSIONS.lock().unwrap();
        if map.len() >= MAX_SESSIONS {
            map.values()
                .filter(|s| !s.manual)
                .min_by_key(|s| s.last_heartbeat.load(Ordering::Relaxed))
                .map(|s| s.sid.clone())
        } else {
            None
        }
    };
    if let Some(old_sid) = evict {
        warn!("relay session limit reached, evict oldest session {}", old_sid);
        stop_relay(&old_sid).await;
    } else if RELAY_SESSIONS.lock().unwrap().len() >= MAX_SESSIONS {
        let _ = std::fs::remove_dir_all(&folder);
        RELAY_STARTING.lock().unwrap().remove(&sid);
        return Err("relay session limit reached".to_string());
    }

    let session = Arc::new(RelaySession {
        sid: sid.clone(),
        url: url.clone(),
        folder: folder.clone(),
        child: Arc::new(Mutex::new(child)),
        started_at: now_secs(),
        last_active: Arc::new(AtomicU64::new(now_secs())),
        last_heartbeat: Arc::new(AtomicU64::new(now_secs())),
        hls_time,
        keep_segments,
        manual,
    });
    RELAY_SESSIONS.lock().unwrap().insert(sid.clone(), session);
    RELAY_STARTING.lock().unwrap().remove(&sid);
    info!("relay session {} started for {}", sid, url);

    Ok(RelayStartResp {
        playlist_url: format!("/api/player/relay/{}/index.m3u8", sid),
        sid,
        hls_time,
        keep_segments,
        manual,
        msg: "relay session started".to_string(),
    })
}

/// 停止并清理一个中继会话
pub async fn stop_relay(sid: &str) -> bool {
    let session = RELAY_SESSIONS.lock().unwrap().remove(sid);
    let Some(session) = session else {
        return false;
    };
    // 解开外层 Arc，取得会话所有权
    let session = match Arc::try_unwrap(session) {
        Ok(s) => s,
        Err(_) => return false,
    };
    // 取出子进程所有权，避免在 await 期间持有互斥锁（保证 future 可 Send）
    let mut child = match Arc::try_unwrap(session.child) {
        Ok(mutex) => mutex.into_inner().unwrap_or_else(|e| e.into_inner()),
        Err(arc) => {
            // 极少数情况下仍有其它请求持有该 Arc：直接发送终止信号，
            // 目录清理交给后台孤儿目录扫描任务
            let mut guard = arc.lock().unwrap();
            let _ = guard.start_kill();
            drop(guard);
            info!("relay session {} stop signal sent", sid);
            return true;
        }
    };
    let _ = child.start_kill();
    let _ = child.wait().await;
    let _ = std::fs::remove_dir_all(&session.folder);
    info!("relay session {} stopped", sid);
    true
}

#[derive(Serialize)]
pub struct RelayStatusResp {
    pub sid: String,
    pub url: String,
    pub alive: bool,
    pub playlist_ready: bool,
    pub playlist_url: String,
    pub started_at: u64,
    pub age_secs: u64,
    pub idle_secs: u64,
    pub hls_time: u32,
    pub keep_segments: u32,
    pub segment_count: usize,
    pub last_error: String,
    pub manual: bool,
    /// 距上次心跳的秒数（仅桌面端会话有自动停止要求）
    pub heartbeat_secs: u64,
}

/// 读取文件尾部若干字节（用于展示 ffmpeg 报错）
fn tail_file(path: &Path, max_bytes: usize) -> String {
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut buf = String::new();
    if f.read_to_string(&mut buf).is_err() {
        return String::new();
    }
    if buf.len() > max_bytes {
        let start = buf
            .char_indices()
            .rev()
            .nth(max_bytes)
            .map(|(i, _)| i)
            .unwrap_or(0);
        buf = buf[start..].to_string();
    }
    buf.trim().to_string()
}

pub fn relay_status(sid: &str) -> Option<RelayStatusResp> {
    let map = RELAY_SESSIONS.lock().unwrap();
    let session = map.get(sid)?;
    let alive = match session.child.lock().unwrap().try_wait() {
        Ok(None) => true,
        _ => false,
    };
    let playlist_ready = session.folder.join("index.m3u8").is_file();
    let segment_count = std::fs::read_dir(&session.folder)
        .map(|d| {
            d.flatten()
                .filter(|e| e.path().extension().map(|x| x == "ts").unwrap_or(false))
                .count()
        })
        .unwrap_or(0);
    let now = now_secs();
    Some(RelayStatusResp {
        sid: sid.to_string(),
        url: session.url.clone(),
        alive,
        playlist_ready,
        playlist_url: format!("/api/player/relay/{}/index.m3u8", sid),
        started_at: session.started_at,
        age_secs: now.saturating_sub(session.started_at),
        idle_secs: now.saturating_sub(session.last_active.load(Ordering::Relaxed)),
        hls_time: session.hls_time,
        keep_segments: session.keep_segments,
        segment_count,
        last_error: tail_file(&session.folder.join("ffmpeg.log"), 4000),
        manual: session.manual,
        heartbeat_secs: now.saturating_sub(session.last_heartbeat.load(Ordering::Relaxed)),
    })
}

/// 后台清理任务：回收闲置/已死会话，删除孤儿目录
pub fn spawn_cleanup_task() {
    let _ = std::fs::create_dir_all(PLAYER_FOLDER);
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(CLEAN_INTERVAL_SECS));
        loop {
            interval.tick().await;
            let stale: Vec<String> = {
                let map = RELAY_SESSIONS.lock().unwrap();
                map.values()
                    .filter(|s| {
                        let dead = matches!(s.child.lock().unwrap().try_wait(), Ok(Some(_)) | Err(_));
                        if s.manual {
                            // 手动添加的 m3u8 会话：永不自动停止，由用户手动停止
                            false
                        } else {
                            // 桌面端会话：60 秒没有播放心跳自动停止；进程已死也回收
                            let hb = now_secs().saturating_sub(s.last_heartbeat.load(Ordering::Relaxed));
                            hb > HEARTBEAT_TIMEOUT_SECS || dead
                        }
                    })
                    .map(|s| s.sid.clone())
                    .collect()
            };
            for sid in stale {
                info!("auto stop stale relay session {}", sid);
                stop_relay(&sid).await;
            }
            // 检查黑名单定时自动清理（过期条目移出，防止误伤恢复的源）
            crate::check_blacklist::cleanup_expired();
            // 清理残留目录（服务器重启后遗留的会话目录）
            if let Ok(entries) = std::fs::read_dir(PLAYER_FOLDER) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let active = RELAY_SESSIONS.lock().unwrap().contains_key(&name)
                        || RELAY_STARTING.lock().unwrap().contains(&name);
                    if !active && entry.path().is_dir() {
                        let _ = std::fs::remove_dir_all(entry.path());
                    }
                }
            }
        }
    });
}

// ============================== 路由 ==============================

#[post("/api/player/relay/start")]
async fn relay_start(req: web::Json<RelayStartReq>) -> impl Responder {
    match start_relay(
        req.url.clone(),
        req.headers.clone(),
        req.hls_time,
        req.keep_segments,
        req.manual,
    )
    .await
    {
        Ok(resp) => HttpResponse::Ok().json(resp),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "msg": e })),
    }
}

#[get("/api/player/relay/{sid}/status")]
async fn relay_status_api(path: web::Path<String>) -> impl Responder {
    let sid = path.into_inner();
    match relay_status(&sid) {
        Some(s) => HttpResponse::Ok().json(s),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "msg": "session not found",
            "sid": sid,
        })),
    }
}

#[get("/api/player/relay")]
async fn relay_list() -> impl Responder {
    let sids: Vec<String> = RELAY_SESSIONS.lock().unwrap().keys().cloned().collect();
    let mut list = Vec::new();
    for sid in sids {
        if let Some(s) = relay_status(&sid) {
            list.push(s);
        }
    }
    HttpResponse::Ok().json(serde_json::json!({ "list": list }))
}

#[delete("/api/player/relay/{sid}")]
async fn relay_stop(path: web::Path<String>) -> impl Responder {
    let sid = path.into_inner();
    if stop_relay(&sid).await {
        HttpResponse::Ok().json(serde_json::json!({ "msg": "stopped", "sid": sid }))
    } else {
        HttpResponse::NotFound().json(serde_json::json!({
            "msg": "session not found",
            "sid": sid,
        }))
    }
}

/// 桌面端播放心跳：播放期间周期性上报“仍在播放”，服务端据此判断会话是否还在被使用
#[post("/api/player/relay/{sid}/heartbeat")]
async fn relay_heartbeat(path: web::Path<String>) -> impl Responder {
    let sid = path.into_inner();
    let session = RELAY_SESSIONS.lock().unwrap().get(&sid).cloned();
    let Some(session) = session else {
        return HttpResponse::NotFound().json(serde_json::json!({ "msg": "session not found" }));
    };
    session.last_heartbeat.store(now_secs(), Ordering::Relaxed);
    let alive = matches!(session.child.lock().unwrap().try_wait(), Ok(None));
    HttpResponse::Ok().json(serde_json::json!({ "sid": sid, "alive": alive }))
}

/// 流畅模式分片参数配置（后台管理）：桌面端不再下发参数，统一由服务端配置决定
#[get("/api/player/relay/config")]
async fn relay_config_get() -> impl Responder {
    let cfg = crate::config::base::get_base_config();
    HttpResponse::Ok().json(serde_json::json!({
        "hls_time": default_hls_time(),
        "keep_segments": default_keep_segments(),
        "_raw_hls_time": cfg.relay_hls_time,
        "_raw_keep_segments": cfg.relay_keep_segments,
    }))
}

#[derive(Deserialize)]
pub struct RelayConfigReq {
    #[serde(default = "default_hls_time")]
    pub hls_time: u32,
    #[serde(default = "default_keep_segments")]
    pub keep_segments: u32,
}

#[post("/api/player/relay/config")]
async fn relay_config_set(req: web::Json<RelayConfigReq>) -> impl Responder {
    let hls_time = req.hls_time.clamp(2, 30);
    let keep_segments = req.keep_segments.clamp(3, 60);
    let mut cfg = crate::config::base::get_base_config();
    cfg.relay_hls_time = hls_time;
    cfg.relay_keep_segments = keep_segments;
    match crate::config::base::update_base_config(cfg) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "hls_time": hls_time,
            "keep_segments": keep_segments,
            "msg": "saved",
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({ "msg": e })),
    }
}

/// 隐藏 playlist 中最新（正在写入）的若干分片，避免客户端请求半成品分片导致卡顿
fn hide_live_edge(playlist: &str, hide_count: usize) -> String {
    let mut lines: Vec<String> = playlist.lines().map(|s| s.to_string()).collect();
    let mut removed = 0;
    while removed < hide_count {
        let Some(uri_idx) = lines.iter().rposition(|l| !l.starts_with('#')) else {
            break;
        };
        lines.remove(uri_idx);
        if uri_idx > 0 && lines[uri_idx - 1].starts_with("#EXTINF") {
            lines.remove(uri_idx - 1);
        }
        removed += 1;
    }
    lines.join("\n")
}

/// 提供本地 HLS 文件（index.m3u8 / segment_*.ts / ffmpeg.log）
#[get("/api/player/relay/{sid}/{file}")]
async fn relay_file(path: web::Path<(String, String)>, req: HttpRequest) -> impl Responder {
    let (sid, file) = path.into_inner();
    // 安全校验：禁止路径穿越
    if file.contains("..") || file.contains('\\') || file.starts_with('/') {
        return HttpResponse::BadRequest().body("invalid file name");
    }
    let session = RELAY_SESSIONS.lock().unwrap().get(&sid).cloned();
    let Some(session) = session else {
        return HttpResponse::NotFound().body("session not found");
    };
    let full = session.folder.join(&file);
    if !full.is_file() {
        return HttpResponse::NotFound().body("file not found (playlist not ready yet)");
    }
    // 记录活动时间与心跳（播放器持续拉流等价于“仍在播放”的心跳）
    session.last_active.store(now_secs(), Ordering::Relaxed);
    session.last_heartbeat.store(now_secs(), Ordering::Relaxed);
    // playlist 特殊处理：隐藏 live edge（最新 1 个正在写入的分片），避免卡顿
    if file.ends_with(".m3u8") {
        let content = match std::fs::read_to_string(&full) {
            Ok(c) => c,
            Err(e) => {
                error!("read relay playlist failed: {}", e);
                return HttpResponse::InternalServerError().body("read playlist failed");
            }
        };
        let rewritten = hide_live_edge(&content, 1);
        return HttpResponse::Ok()
            .insert_header(("Content-Type", "application/vnd.apple.mpegurl"))
            .insert_header(("Cache-Control", "no-store"))
            .body(rewritten);
    }
    match NamedFile::open(&full) {
        Ok(f) => f.into_response(&req),
        Err(e) => {
            error!("open relay file failed: {}", e);
            HttpResponse::InternalServerError().body("open file failed")
        }
    }
}

// ============================== 透明流代理（直连播放走服务端） ==============================

/// 拉取源 m3u8（含 master playlist），把其中所有 URI（变体/分片/密钥）重写为
/// 指向本服务的 /api/player/proxy/media?u=...，使客户端全程同源播放，无 CORS 问题。
#[derive(Deserialize)]
pub struct ProxyQuery {
    pub url: String,
    #[serde(default)]
    pub ua: Option<String>,
}

#[derive(Deserialize)]
pub struct MediaQuery {
    pub u: String,
    #[serde(default)]
    pub ua: Option<String>,
}

fn resolve_uri(base: &url::Url, uri: &str) -> Option<String> {
    if let Ok(abs) = base.join(uri) {
        Some(abs.to_string())
    } else {
        None
    }
}

/// 把一行 playlist 文本里的 URI 重写为代理地址（返回 None 表示无需改写）
fn rewrite_line(line: &str, base: &url::Url, ua: Option<&str>) -> String {
    let ua_q = ua
        .map(|u| format!("&ua={}", url::form_urlencoded::byte_serialize(u.as_bytes()).collect::<String>()))
        .unwrap_or_default();
    // 处理 #EXT-X-KEY / #EXT-X-MEDIA 等含 URI="..." 的属性行
    if (line.starts_with("#EXT-X-KEY:") || line.starts_with("#EXT-X-MEDIA:")) && line.contains("URI=") {
        // 提取并重写 URI="xxx" 部分
        let mut result = String::new();
        let mut rest = line;
        while let Some(idx) = rest.find("URI=") {
            result.push_str(&rest[..idx + 4]);
            rest = &rest[idx + 4..];
            let inner = if rest.starts_with('\"') {
                let end = rest[1..].find('\"').map(|e| e + 2).unwrap_or(rest.len());
                let val = &rest[1..end - 1];
                let new_uri = resolve_uri(base, val)
                    .map(|abs| {
                        let enc = url::form_urlencoded::byte_serialize(abs.as_bytes())
                            .collect::<String>();
                        format!("\"/api/player/proxy/media?u={}{}\"", enc, ua_q)
                    })
                    .unwrap_or_else(|| format!("\"{}\"", val));
                rest = &rest[end..];
                new_uri
            } else {
                // 无引号（少见）
                let end = rest.find(',').unwrap_or(rest.len());
                let val = &rest[..end];
                let new_uri = resolve_uri(base, val)
                    .map(|abs| {
                        let enc = url::form_urlencoded::byte_serialize(abs.as_bytes())
                            .collect::<String>();
                        format!("/api/player/proxy/media?u={}{}", enc, ua_q)
                    })
                    .unwrap_or_else(|| val.to_string());
                rest = &rest[end..];
                new_uri
            };
            result.push_str(&inner);
        }
        result.push_str(rest);
        return result;
    }
    // 普通 URI 行（变体 playlist / 分片）
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return line.to_string();
    }
    match resolve_uri(base, trimmed) {
        Some(abs) => {
            let enc = url::form_urlencoded::byte_serialize(abs.as_bytes()).collect::<String>();
            format!("/api/player/proxy/media?u={}{}", enc, ua_q)
        }
        None => line.to_string(),
    }
}

async fn fetch_and_rewrite_playlist(url: &str, ua: Option<&str>) -> Result<String, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("invalid playlist url".to_string());
    }
    let base = url::Url::parse(url).map_err(|e| format!("invalid playlist url: {}", e))?;
    let client = crate::common::util::get_http_client();
    let mut builder = client
        .get(url)
        // 死源快速失败，避免播放器长时间挂起
        .timeout(std::time::Duration::from_secs(30));
    let ua_value = ua.map(|s| s.to_string()).unwrap_or_else(|| {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36".to_string()
    });
    builder = builder.header("User-Agent", &ua_value);
    let resp = builder
        .send()
        .await
        .map_err(|e| format!("fetch playlist failed: {}", e))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("source returned {}", status));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| format!("read playlist failed: {}", e))?;
    let rewritten: Vec<String> = text
        .lines()
        .map(|l| rewrite_line(l, &base, ua))
        .collect();
    Ok(rewritten.join("\n"))
}

/// 透明代理：返回重写后的 m3u8（客户端直连播放入口）
#[get("/api/player/proxy")]
async fn player_proxy(q: web::Query<ProxyQuery>) -> impl Responder {
    match fetch_and_rewrite_playlist(&q.url, q.ua.as_deref()).await {
        Ok(body) => HttpResponse::Ok()
            .insert_header(("Content-Type", "application/vnd.apple.mpegurl; charset=utf-8"))
            .insert_header(("Cache-Control", "no-store"))
            .body(body),
        Err(e) => HttpResponse::BadGateway().json(serde_json::json!({ "msg": e })),
    }
}

/// 透明代理：流式转发分片/媒体内容（支持 Range）
#[get("/api/player/proxy/media")]
async fn player_proxy_media(q: web::Query<MediaQuery>, req: HttpRequest) -> impl Responder {
    let target = q.u.clone();
    if !target.starts_with("http://") && !target.starts_with("https://") {
        return HttpResponse::BadRequest().body("invalid media url");
    }
    let client = crate::common::util::get_http_client();
    let mut builder = client
        .get(&target)
        .timeout(std::time::Duration::from_secs(60));
    // 转发 Range（部分源/MP4 需要）
    if let Some(range) = req.headers().get("range") {
        if let Ok(v) = range.to_str() {
            builder = builder.header("Range", v.to_string());
        }
    }
    let ua_value = q
        .ua
        .clone()
        .unwrap_or_else(|| {
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36".to_string()
        });
    builder = builder.header("User-Agent", ua_value);
    match builder.send().await {
        Ok(resp) => {
            let status_code = actix_web::http::StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(actix_web::http::StatusCode::BAD_GATEWAY);
            let content_type = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let content_length = resp
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let content_range = resp
                .headers()
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            // 流式转发，避免整段读入内存
            let stream = resp.bytes_stream().map(|item| {
                item.map(actix_web::web::Bytes::from)
                    .map_err(|e| {
                        actix_web::error::Error::from(std::io::Error::new(std::io::ErrorKind::Other, e))
                    })
            });
            let mut r = HttpResponse::build(status_code);
            if let Some(ct) = content_type {
                r.insert_header(("Content-Type", ct));
            }
            if let Some(cl) = content_length {
                r.insert_header(("Content-Length", cl));
            }
            if let Some(cr) = content_range {
                r.insert_header(("Content-Range", cr));
            }
            r.insert_header(("Cache-Control", "public, max-age=60"));
            r.streaming(stream)
        }
        Err(e) => HttpResponse::BadGateway().body(format!("fetch media failed: {}", e)),
    }
}

// ============================== 搜索历史与推荐 ==============================

pub static SEARCH_HISTORY_FILE: &str = "./static/core/search_history.json";

#[derive(Serialize, Deserialize, Clone)]
struct SearchHistoryItem {
    count: u32,
    last_at: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct SearchHistoryFile {
    #[serde(default)]
    items: HashMap<String, SearchHistoryItem>,
}

lazy_static! {
    static ref SEARCH_HISTORY: Mutex<HashMap<String, SearchHistoryItem>> =
        Mutex::new(load_search_history());
}

fn load_search_history() -> HashMap<String, SearchHistoryItem> {
    match std::fs::read_to_string(SEARCH_HISTORY_FILE) {
        Ok(s) => serde_json::from_str::<SearchHistoryFile>(&s)
            .map(|f| f.items)
            .unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn save_search_history() {
    let items = SEARCH_HISTORY.lock().unwrap().clone();
    let file = SearchHistoryFile { items };
    if let Ok(json) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(SEARCH_HISTORY_FILE, json);
    }
}

/// 记录一次频道名搜索（用于之后在「添加想看频道」时推荐）
pub fn record_search(name: &str) {
    let name = name.trim().to_string();
    if name.is_empty() || name.len() > 100 {
        return;
    }
    {
        let mut map = SEARCH_HISTORY.lock().unwrap();
        let entry = map
            .entry(name.clone())
            .or_insert(SearchHistoryItem { count: 0, last_at: 0 });
        entry.count += 1;
        entry.last_at = now_secs();
        // 条目上限：超出时淘汰最久未搜索的
        if map.len() > 300 {
            let oldest = map
                .iter()
                .min_by_key(|(_, v)| v.last_at)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                map.remove(&k);
            }
        }
    }
    save_search_history();
}

/// 推荐经常搜索的频道名：按搜索次数排序，支持关键字过滤与分页
/// 返回 (列表, 总数)
pub fn get_search_suggestions(
    keyword: &str,
    page: usize,
    page_size: usize,
) -> (Vec<serde_json::Value>, usize) {
    let kw = keyword.trim().to_lowercase();
    let map = SEARCH_HISTORY.lock().unwrap();
    let mut items: Vec<(String, SearchHistoryItem)> = map
        .iter()
        .filter(|(k, _)| kw.is_empty() || k.to_lowercase().contains(&kw))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    items.sort_by(|a, b| {
        b.1.count
            .cmp(&a.1.count)
            .then_with(|| b.1.last_at.cmp(&a.1.last_at))
    });
    let total = items.len();
    let size = if page_size > 0 { page_size } else { 20 };
    let list = items
        .into_iter()
        .skip(page.saturating_mul(size))
        .take(size)
        .map(|(name, v)| {
            serde_json::json!({
                "name": name,
                "count": v.count,
                "last_at": v.last_at,
            })
        })
        .collect();
    (list, total)
}

#[derive(Deserialize)]
pub struct RecordSearchReq {
    pub name: String,
}

#[derive(Deserialize)]
pub struct SearchHistoryQuery {
    #[serde(default)]
    pub keyword: String,
    #[serde(default = "default_suggest_limit")]
    pub limit: usize,
    #[serde(default)]
    pub page: usize,
    #[serde(default)]
    pub page_size: usize,
}

fn default_suggest_limit() -> usize {
    20
}

#[post("/api/player/search-history")]
async fn record_search_api(req: web::Json<RecordSearchReq>) -> impl Responder {
    record_search(&req.name);
    HttpResponse::Ok().json(serde_json::json!({ "msg": "success" }))
}

#[get("/api/player/search-history")]
async fn get_search_history_api(q: web::Query<SearchHistoryQuery>) -> impl Responder {
    // 兼容旧 limit 参数：未传 page_size 时用 limit 作为每页大小
    let page_size = if q.page_size > 0 { q.page_size } else { q.limit.min(500) };
    let (list, total) = get_search_suggestions(&q.keyword, q.page, page_size);
    HttpResponse::Ok().json(serde_json::json!({
        "list": list,
        "total": total,
        "page": q.page,
        "page_size": page_size,
    }))
}

#[delete("/api/player/search-history/{name}")]
async fn delete_search_history_api(path: web::Path<String>) -> impl Responder {
    let name = path.into_inner();
    let removed = SEARCH_HISTORY.lock().unwrap().remove(&name).is_some();
    if removed {
        save_search_history();
    }
    if removed {
        HttpResponse::Ok().json(serde_json::json!({ "msg": "deleted", "name": name }))
    } else {
        HttpResponse::NotFound().json(serde_json::json!({ "msg": "not found" }))
    }
}

#[delete("/api/player/search-history")]
async fn clear_search_history_api() -> impl Responder {
    let count = SEARCH_HISTORY.lock().unwrap().len();
    SEARCH_HISTORY.lock().unwrap().clear();
    save_search_history();
    HttpResponse::Ok().json(serde_json::json!({ "msg": "cleared", "count": count }))
}

// ============================== 播放历史（后台播放列表） ==============================

pub static PLAY_HISTORY_FILE: &str = "./static/core/play_history.json";

#[derive(Serialize, Deserialize, Clone)]
pub struct PlayHistoryItem {
    pub id: String,
    pub name: String,
    pub url: String,
    /// 该链接最近一次播放是否成功（可播放标识）
    pub playable: bool,
    pub count: u32,
    pub last_at: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct PlayHistoryFile {
    #[serde(default)]
    items: HashMap<String, PlayHistoryItem>,
}

lazy_static! {
    static ref PLAY_HISTORY: Mutex<HashMap<String, PlayHistoryItem>> =
        Mutex::new(load_play_history());
}

fn load_play_history() -> HashMap<String, PlayHistoryItem> {
    match std::fs::read_to_string(PLAY_HISTORY_FILE) {
        Ok(s) => serde_json::from_str::<PlayHistoryFile>(&s)
            .map(|f| f.items)
            .unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn save_play_history() {
    let items = PLAY_HISTORY.lock().unwrap().clone();
    let file = PlayHistoryFile { items };
    if let Ok(json) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(PLAY_HISTORY_FILE, json);
    }
}

/// 记录一次播放：同一链接更新名称/可播放标识/时间/次数
pub fn record_play(name: &str, url: &str, playable: bool) {
    let name = name.trim().to_string();
    let url = url.trim().to_string();
    if url.is_empty() {
        return;
    }
    let display_name = if name.is_empty() { url.clone() } else { name };
    let id = format!("{:x}", md5::compute(url.as_bytes()));
    {
        let mut map = PLAY_HISTORY.lock().unwrap();
        let item = map.entry(id.clone()).or_insert(PlayHistoryItem {
            id: id.clone(),
            name: display_name.clone(),
            url: url.clone(),
            playable,
            count: 0,
            last_at: 0,
        });
        item.name = display_name;
        item.playable = playable;
        item.count += 1;
        item.last_at = now_secs();
        // 上限 1000 条，超出淘汰最久未播放的
        if map.len() > 1000 {
            let oldest = map
                .iter()
                .min_by_key(|(_, v)| v.last_at)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                map.remove(&k);
            }
        }
    }
    save_play_history();
}

/// 查询播放历史：playable 过滤（None=全部 / Some(true)=仅可播放），keyword 过滤名称，支持分页
/// 返回 (列表, 总数)
pub fn get_play_history(
    playable: Option<bool>,
    keyword: &str,
    page: usize,
    page_size: usize,
) -> (Vec<serde_json::Value>, usize) {
    let kw = keyword.trim().to_lowercase();
    let map = PLAY_HISTORY.lock().unwrap();
    let mut items: Vec<PlayHistoryItem> = map
        .values()
        .filter(|v| playable.map(|p| v.playable == p).unwrap_or(true))
        .filter(|v| kw.is_empty() || v.name.to_lowercase().contains(&kw))
        .cloned()
        .collect();
    items.sort_by(|a, b| b.last_at.cmp(&a.last_at).then_with(|| b.count.cmp(&a.count)));
    let total = items.len();
    let size = if page_size > 0 { page_size } else { 20 };
    let list = items
        .into_iter()
        .skip(page.saturating_mul(size))
        .take(size)
        .map(|v| {
            serde_json::json!({
                "id": v.id,
                "name": v.name,
                "url": v.url,
                "playable": v.playable,
                "count": v.count,
                "last_at": v.last_at,
            })
        })
        .collect();
    (list, total)
}

pub fn delete_play_history(id: &str) -> bool {
    let removed = PLAY_HISTORY.lock().unwrap().remove(id).is_some();
    if removed {
        save_play_history();
    }
    removed
}

pub fn clear_play_history() {
    PLAY_HISTORY.lock().unwrap().clear();
    save_play_history();
}

#[derive(Deserialize)]
pub struct RecordPlayReq {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub playable: bool,
}

#[derive(Deserialize)]
pub struct PlayHistoryQuery {
    /// "1"=仅可播放 / "0"=仅不可播放 / 空=全部
    #[serde(default)]
    pub playable: String,
    #[serde(default)]
    pub keyword: String,
    #[serde(default = "default_suggest_limit")]
    pub limit: usize,
    #[serde(default)]
    pub page: usize,
    #[serde(default)]
    pub page_size: usize,
}

#[post("/api/player/play-history")]
async fn record_play_api(req: web::Json<RecordPlayReq>) -> impl Responder {
    record_play(&req.name, &req.url, req.playable);
    HttpResponse::Ok().json(serde_json::json!({ "msg": "success" }))
}

#[get("/api/player/play-history")]
async fn get_play_history_api(q: web::Query<PlayHistoryQuery>) -> impl Responder {
    let filter = if q.playable == "1" {
        Some(true)
    } else if q.playable == "0" {
        Some(false)
    } else {
        None
    };
    let page_size = if q.page_size > 0 { q.page_size } else { q.limit.min(500) };
    let (list, total) = get_play_history(filter, &q.keyword, q.page, page_size);
    HttpResponse::Ok().json(serde_json::json!({
        "list": list,
        "total": total,
        "page": q.page,
        "page_size": page_size,
    }))
}

#[delete("/api/player/play-history/{id}")]
async fn delete_play_history_api(path: web::Path<String>) -> impl Responder {
    let id = path.into_inner();
    if delete_play_history(&id) {
        HttpResponse::Ok().json(serde_json::json!({ "msg": "deleted", "id": id }))
    } else {
        HttpResponse::NotFound().json(serde_json::json!({ "msg": "not found" }))
    }
}

#[delete("/api/player/play-history")]
async fn clear_play_history_api() -> impl Responder {
    clear_play_history();
    HttpResponse::Ok().json(serde_json::json!({ "msg": "cleared" }))
}

// ============================== 频道收藏 ==============================

pub static PLAYER_FAVOURITES_FILE: &str = "./static/core/player_favourites.json";

#[derive(Serialize, Deserialize, Clone)]
pub struct FavouriteChannel {
    pub id: String,
    pub name: String,
    pub url: String,
    pub created_at: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct PlayerFavouritesFile {
    #[serde(default)]
    items: HashMap<String, FavouriteChannel>,
}

lazy_static! {
    static ref PLAYER_FAVOURITES: Mutex<HashMap<String, FavouriteChannel>> =
        Mutex::new(load_player_favourites());
}

fn load_player_favourites() -> HashMap<String, FavouriteChannel> {
    match std::fs::read_to_string(PLAYER_FAVOURITES_FILE) {
        Ok(s) => serde_json::from_str::<PlayerFavouritesFile>(&s)
            .map(|f| f.items)
            .unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn save_player_favourites() {
    let items = PLAYER_FAVOURITES.lock().unwrap().clone();
    let file = PlayerFavouritesFile { items };
    if let Ok(json) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(PLAYER_FAVOURITES_FILE, json);
    }
}

/// 收藏频道（按 url 去重，幂等）
pub fn add_favourite_channel(name: &str, url: &str) -> Option<FavouriteChannel> {
    let name = name.trim().to_string();
    let url = url.trim().to_string();
    if url.is_empty() {
        return None;
    }
    let display_name = if name.is_empty() { url.clone() } else { name };
    let id = format!("{:x}", md5::compute(url.as_bytes()));
    let item = FavouriteChannel {
        id: id.clone(),
        name: display_name,
        url,
        created_at: now_secs(),
    };
    PLAYER_FAVOURITES.lock().unwrap().insert(id.clone(), item.clone());
    save_player_favourites();
    Some(item)
}

pub fn remove_favourite_channel(id: &str) -> bool {
    let removed = PLAYER_FAVOURITES.lock().unwrap().remove(id).is_some();
    if removed {
        save_player_favourites();
    }
    removed
}

/// 某个 url 是否已收藏（返回其 id）
pub fn favourite_id_of_url(url: &str) -> Option<String> {
    let id = format!("{:x}", md5::compute(url.trim().as_bytes()));
    if PLAYER_FAVOURITES.lock().unwrap().contains_key(&id) {
        Some(id)
    } else {
        None
    }
}

/// 收藏列表（支持分页），返回 (列表, 总数)
pub fn get_favourite_channels(page: usize, page_size: usize) -> (Vec<serde_json::Value>, usize) {
    let map = PLAYER_FAVOURITES.lock().unwrap();
    let mut items: Vec<FavouriteChannel> = map.values().cloned().collect();
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let total = items.len();
    let size = if page_size > 0 { page_size } else { 50 };
    let list = items
        .into_iter()
        .skip(page.saturating_mul(size))
        .take(size)
        .map(|v| {
            serde_json::json!({
                "id": v.id,
                "name": v.name,
                "url": v.url,
                "created_at": v.created_at,
            })
        })
        .collect();
    (list, total)
}

#[derive(Deserialize)]
pub struct FavouriteReq {
    pub name: String,
    pub url: String,
}

#[derive(Deserialize)]
pub struct FavouriteCheckQuery {
    pub url: String,
}

#[derive(Deserialize)]
pub struct FavouritesQuery {
    #[serde(default)]
    pub page: usize,
    #[serde(default)]
    pub page_size: usize,
}

#[get("/api/player/favourites")]
async fn get_favourites_api(q: web::Query<FavouritesQuery>) -> impl Responder {
    let (list, total) = get_favourite_channels(q.page, q.page_size);
    HttpResponse::Ok().json(serde_json::json!({
        "list": list,
        "total": total,
        "page": q.page,
        "page_size": if q.page_size > 0 { q.page_size } else { 50 },
    }))
}

#[post("/api/player/favourites")]
async fn add_favourite_api(req: web::Json<FavouriteReq>) -> impl Responder {
    match add_favourite_channel(&req.name, &req.url) {
        Some(item) => HttpResponse::Ok().json(item),
        None => HttpResponse::BadRequest().json(serde_json::json!({ "msg": "invalid url" })),
    }
}

#[delete("/api/player/favourites/{id}")]
async fn remove_favourite_api(path: web::Path<String>) -> impl Responder {
    let id = path.into_inner();
    if remove_favourite_channel(&id) {
        HttpResponse::Ok().json(serde_json::json!({ "msg": "removed", "id": id }))
    } else {
        HttpResponse::NotFound().json(serde_json::json!({ "msg": "not found" }))
    }
}

#[get("/api/player/favourites/check")]
async fn check_favourite_api(q: web::Query<FavouriteCheckQuery>) -> impl Responder {
    let id = favourite_id_of_url(&q.url);
    HttpResponse::Ok().json(serde_json::json!({"favourited": id.is_some(), "id": id }))
}

/// 收藏列表订阅地址：导出为 m3u 供其他播放器本地订阅
#[get("/api/player/favourites.m3u8")]
async fn get_favourites_m3u_api() -> impl Responder {
    let map = PLAYER_FAVOURITES.lock().unwrap();
    let mut items: Vec<FavouriteChannel> = map.values().cloned().collect();
    items.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    let mut out = String::from("#EXTM3U\n");
    for v in items {
        out.push_str(&format!("#EXTINF:-1,{}\n{}\n", v.name, v.url));
    }
    HttpResponse::Ok()
        .insert_header(("Content-Type", "application/vnd.apple.mpegurl; charset=utf-8"))
        .insert_header(("Cache-Control", "no-store"))
        .body(out)
}

// ============================== 频道实时画面快照 ==============================

pub static SNAPSHOT_FOLDER: &str = "./static/thumbnail/channels/";
pub static SNAPSHOT_URL_BASE: &str = "/static/thumbnail/channels/";
// 画面文件只要存在就直接复用（重新抓帧只在 refresh=true 或文件缺失时发生）

/// 探测源视频编码（返回小写 codec_name，失败返回 None）
pub async fn probe_video_codec(url: &str) -> Option<String> {
    let mut cmd = tokio::process::Command::new("ffprobe");
    crate::common::util::apply_proxy_to_command(&mut cmd);
    let child = cmd
        .arg("-v")
        .arg("error")
        .arg("-timeout")
        .arg("8000000")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=codec_name")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let output = tokio::time::timeout(std::time::Duration::from_secs(12), child.wait_with_output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn snapshot_ffmpeg_bin() -> &'static str {
    if std::path::Path::new("./tools/ffmpeg/ffmpeg.exe").exists() {
        "./tools/ffmpeg/ffmpeg.exe"
    } else if std::path::Path::new("./tools/ffmpeg/ffmpeg").exists() {
        "./tools/ffmpeg/ffmpeg"
    } else {
        "ffmpeg"
    }
}

async fn capture_snapshot(url: &str, out_path: &str) -> bool {
    // ffmpeg 抓第一帧（带浏览器 UA，部分源会校验）
    let mut cmd = tokio::process::Command::new(snapshot_ffmpeg_bin());
    crate::common::util::apply_proxy_to_command(&mut cmd);
    let child = cmd
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-headers")
        .arg("User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36\r\n")
        .arg("-i")
        .arg(url)
        .arg("-frames:v")
        .arg("1")
        .arg("-timeout")
        .arg("10000000")
        .arg(out_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    match child {
        Ok(mut c) => {
            match tokio::time::timeout(std::time::Duration::from_secs(12), c.wait()).await {
                Ok(Ok(status)) => status.success(),
                _ => {
                    let _ = c.start_kill();
                    false
                }
            }
        }
        Err(_) => false,
    }
}

/// 批量抓取频道快照（并发 4；10 分钟内复用缓存）。
/// existing_only=true 时不抓帧，只返回上次抓取到的图片与时间（供客户端立即展示旧画面）。
pub async fn get_snapshots(
    urls: Vec<String>,
    refresh: bool,
    existing_only: bool,
) -> Vec<serde_json::Value> {
    let _ = std::fs::create_dir_all(SNAPSHOT_FOLDER);
    let now = now_secs();
    let results = futures::stream::iter(urls.into_iter())
        .map(|url| async move {
            let id = format!("{:x}", md5::compute(url.as_bytes()));
            let file_name = format!("{}.jpg", id);
            let path = format!("{}{}", SNAPSHOT_FOLDER, file_name);
            let snapshot_url = format!("{}{}", SNAPSHOT_URL_BASE, file_name);
            // 文件存在且非空才视为有效（抓帧失败可能残留 0 字节文件）
            let meta = std::fs::metadata(&path)
                .ok()
                .filter(|m| m.len() > 0)
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            // 仅查询已有画面：不重新抓帧，直接返回上次抓取的图片与时间
            if existing_only {
                let exists = meta > 0;
                return serde_json::json!({
                    "url": url,
                    "snapshot": if exists { snapshot_url } else { String::new() },
                    "ok": exists,
                    "captured_at": meta,
                });
            }
            // refresh=false 时：本地已有画面直接复用（图片已保存到本地，无需重新抓帧）；
            // 只有文件缺失（或 refresh=true 强制刷新）才重新抓帧
            let cached = !refresh && meta > 0;
            let ok = if cached {
                true
            } else {
                capture_snapshot(&url, &path).await
            };
            let captured_at = if ok {
                std::fs::metadata(&path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(now)
            } else {
                0
            };
            serde_json::json!({
                "url": url,
                "snapshot": if ok { snapshot_url } else { String::new() },
                "ok": ok,
                "captured_at": captured_at,
            })
        })
        .buffer_unordered(4)
        .collect::<Vec<_>>()
        .await;
    results
}

#[derive(Deserialize)]
pub struct SnapshotsReq {
    pub urls: Vec<String>,
    #[serde(default)]
    pub refresh: bool,
    /// 仅返回已有画面（不抓帧），供客户端刷新页面后立即展示上次抓取的图片
    #[serde(default)]
    pub existing_only: bool,
}

#[derive(Deserialize)]
pub struct SnapshotsConfigReq {
    pub enabled: bool,
}

#[get("/api/player/snapshots/config")]
async fn get_snapshots_config_api() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "enabled": crate::config::base::get_base_config().player_show_snapshots,
    }))
}

#[post("/api/player/snapshots/config")]
async fn set_snapshots_config_api(req: web::Json<SnapshotsConfigReq>) -> impl Responder {
    let mut cfg = crate::config::base::get_base_config();
    cfg.player_show_snapshots = req.enabled;
    match crate::config::base::update_base_config(cfg) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({ "enabled": req.enabled, "msg": "success" })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({ "msg": e })),
    }
}

#[post("/api/player/snapshots")]
async fn get_snapshots_api(req: web::Json<SnapshotsReq>) -> impl Responder {
    let urls: Vec<String> = req
        .urls
        .iter()
        .filter(|u| u.starts_with("http"))
        .take(60)
        .cloned()
        .collect();
    let list = get_snapshots(urls, req.refresh, req.existing_only).await;
    HttpResponse::Ok().json(serde_json::json!({ "list": list }))
}

// ============================== EPG 源文件查看 ==============================

#[derive(Deserialize)]
pub struct EpgFileQuery {
    pub name: String,
}

/// 列出当前 EPG 源文件（static/epg/{today}/）
#[get("/api/epg/files")]
async fn get_epg_files_api() -> impl Responder {
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    let folder = format!("./static/epg/{}/", today);
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&folder) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                let mtime = std::fs::metadata(&p)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                files.push(serde_json::json!({ "name": name, "size": size, "mtime": mtime }));
            }
        }
    }
    files.sort_by(|a, b| b["mtime"].as_u64().cmp(&a["mtime"].as_u64()));
    HttpResponse::Ok().json(serde_json::json!({ "list": files, "total": files.len() }))
}

/// 读取 EPG 源文件内容；gz/zip 自动解压（超长内容截断返回）
#[get("/api/epg/files/content")]
async fn get_epg_file_content_api(q: web::Query<EpgFileQuery>) -> impl Responder {
    let name = q.name.clone();
    // 安全校验：禁止路径穿越
    if name.contains("..") || name.contains('\\') || name.contains('/') {
        return HttpResponse::BadRequest().json(serde_json::json!({ "msg": "invalid name" }));
    }
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    let path = format!("./static/epg/{}/{}", today, name);
    let raw = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            return HttpResponse::NotFound().json(serde_json::json!({ "msg": format!("read failed: {}", e) }));
        }
    };
    // gz 解压
    let decompressed: Vec<u8> = if name.ends_with(".gz") {
        let reader = flate2::read::GzDecoder::new(std::io::Cursor::new(raw));
        use std::io::Read as _;
        match reader.bytes().collect::<Result<Vec<u8>, _>>() {
            Ok(b) => b,
            Err(_) => {
                return HttpResponse::BadRequest().json(serde_json::json!({ "msg": "gzip decompress failed" }));
            }
        }
    } else if name.ends_with(".zip") {
        // zip：取第一个条目
        match zip::ZipArchive::new(std::io::Cursor::new(raw)) {
            Ok(mut archive) => {
                if archive.len() == 0 {
                    return HttpResponse::BadRequest().json(serde_json::json!({ "msg": "empty zip" }));
                }
                match archive.by_index(0) {
                    Ok(mut f) => {
                        let mut out = Vec::new();
                        if std::io::Read::read_to_end(&mut f, &mut out).is_err() {
                            return HttpResponse::BadRequest().json(serde_json::json!({ "msg": "zip read failed" }));
                        }
                        out
                    }
                    Err(e) => return HttpResponse::BadRequest().json(serde_json::json!({ "msg": format!("zip open failed: {}", e) })),
                }
            }
            Err(e) => return HttpResponse::BadRequest().json(serde_json::json!({ "msg": format!("zip parse failed: {}", e) })),
        }
    } else {
        raw
    };
    // 转文本并截断
    let text = String::from_utf8_lossy(&decompressed).to_string();
    const MAX_CHARS: usize = 50 * 1024;
    let truncated = text.chars().count() > MAX_CHARS;
    let body: String = if truncated {
        text.chars().take(MAX_CHARS).collect()
    } else {
        text
    };
    HttpResponse::Ok().json(serde_json::json!({
        "content": body,
        "truncated": truncated,
        "size": decompressed.len(),
    }))
}

// ============================== 网页端 → 桌面端播放请求 ==============================

#[derive(Serialize, Deserialize, Clone)]
pub struct PlayRequestItem {
    pub id: String,
    pub name: String,
    pub url: String,
    pub created_at: u64,
}

lazy_static! {
    static ref PLAY_REQUEST: Mutex<Option<PlayRequestItem>> = Mutex::new(None);
}

#[derive(Deserialize)]
pub struct PlayRequestReq {
    pub name: String,
    pub url: String,
}

#[post("/api/player/play-request")]
async fn create_play_request_api(req: web::Json<PlayRequestReq>) -> impl Responder {
    let name = req.name.trim().to_string();
    let url = req.url.trim().to_string();
    if url.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({ "msg": "url required" }));
    }
    let item = PlayRequestItem {
        id: format!("{:x}", md5::compute(format!("{}{}", name, url).as_bytes())),
        name: if name.is_empty() { url.clone() } else { name },
        url,
        created_at: now_secs(),
    };
    *PLAY_REQUEST.lock().unwrap() = Some(item.clone());
    HttpResponse::Ok().json(serde_json::json!({ "id": item.id, "msg": "success" }))
}

#[get("/api/player/play-request")]
async fn get_play_request_api() -> impl Responder {
    let item = PLAY_REQUEST.lock().unwrap().clone();
    HttpResponse::Ok().json(serde_json::json!({ "request": item }))
}

#[derive(Deserialize)]
pub struct PlayRequestAckReq {
    pub id: String,
}

#[post("/api/player/play-request/ack")]
async fn ack_play_request_api(req: web::Json<PlayRequestAckReq>) -> impl Responder {
    let mut guard = PLAY_REQUEST.lock().unwrap();
    let consumed = guard
        .as_ref()
        .map(|r| r.id == req.id)
        .unwrap_or(false);
    if consumed {
        *guard = None;
    }
    HttpResponse::Ok().json(serde_json::json!({ "msg": "acked", "consumed": consumed }))
}

/// 在 web.rs 的 App 中注册播放器路由
pub fn configure_player_routes(cfg: &mut web::ServiceConfig) {
    info!("registering player routes...");
    cfg.service(player_channels)
        .service(record_search_api)
        .service(get_search_history_api)
        .service(delete_search_history_api)
        .service(clear_search_history_api)
        .service(record_play_api)
        .service(get_play_history_api)
        .service(delete_play_history_api)
        .service(clear_play_history_api)
        .service(get_favourites_api)
        .service(add_favourite_api)
        .service(remove_favourite_api)
        .service(check_favourite_api)
        .service(get_favourites_m3u_api)
        .service(get_epg_files_api)
        .service(get_epg_file_content_api)
        .service(get_snapshots_api)
        .service(get_snapshots_config_api)
        .service(set_snapshots_config_api)
        .service(create_play_request_api)
        .service(get_play_request_api)
        .service(ack_play_request_api)
        .configure(crate::check_blacklist::configure_routes)
        .configure(crate::logo_crawl::configure_routes)
        .service(clear_channel_cache_api)
        .service(get_cache_config_api)
        .service(set_cache_config_api)
        .service(player_proxy)
        .service(player_proxy_media)
        .service(relay_start)
        .service(relay_status_api)
        .service(relay_list)
        .service(relay_stop)
        .service(relay_file)
        .service(relay_heartbeat)
        .service(relay_config_get)
        .service(relay_config_set);
}

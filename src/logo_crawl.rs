//! 频道 logo 自动爬取：搜索配置开启后，爬取过程中自动下载频道台标到本地，
//! 并在「频道封面配置」页提供整理区：批量选择封面、绑定频道名后进入正式封面配置列表。

use actix_web::{delete, get, post, web, HttpResponse, Responder};
use futures::StreamExt;
use lazy_static::lazy_static;
use log::info;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub static LOGO_CRAWL_FOLDER: &str = "./static/core/logos_crawled/";
pub static LOGO_CRAWL_URL_BASE: &str = "/static/core/logos_crawled/";
pub static LOGO_CRAWL_FILE: &str = "./static/core/logos_crawled.json";

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CrawledLogo {
    pub id: String,
    pub file: String,
    pub url: String,
    pub source_url: String,
    pub names: Vec<String>,
    pub created_at: u64,
}

lazy_static! {
    static ref CRAWLED_LOGOS: Mutex<Vec<CrawledLogo>> = Mutex::new(load());
    /// 是否正在爬取（防止定时任务与手动触发并发重复下载）
    static ref CRAWLING: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
}

fn load() -> Vec<CrawledLogo> {
    match std::fs::read_to_string(LOGO_CRAWL_FILE) {
        Ok(s) => serde_json::from_str::<Vec<CrawledLogo>>(&s).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn save() {
    let list = CRAWLED_LOGOS.lock().unwrap().clone();
    if let Ok(json) = serde_json::to_string_pretty(&list) {
        let _ = std::fs::write(LOGO_CRAWL_FILE, json);
    }
}

fn ext_from_content_type(ct: &str) -> &'static str {
    if ct.contains("png") {
        "png"
    } else if ct.contains("jpeg") || ct.contains("jpg") {
        "jpg"
    } else if ct.contains("webp") {
        "webp"
    } else if ct.contains("gif") {
        "gif"
    } else {
        "png"
    }
}

/// 文件名安全化：去掉 Windows/URL 非法字符并限制长度，避免频道名破坏文件路径
fn sanitize_file_name(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "\\/:*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    s = s.trim().trim_matches('.').to_string();
    if s.is_empty() {
        s = "logo".to_string();
    }
    const MAX: usize = 80;
    if s.chars().count() > MAX {
        s = s.chars().take(MAX).collect();
    }
    s
}

/// 下载 logo，文件名带频道名标注：{md5}__{频道名}.{ext}
async fn download_logo(url: &str, name: &str) -> Option<String> {
    let client = crate::common::util::get_http_client();
    let resp = client
        .get(url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !ct.starts_with("image/") {
        return None;
    }
    let bytes = resp.bytes().await.ok()?;
    if bytes.len() < 100 || bytes.len() > 2 * 1024 * 1024 {
        return None;
    }
    let ext = ext_from_content_type(&ct).to_string();
    let safe = sanitize_file_name(name);
    let file_name = format!("{:x}__{}.{}", md5::compute(url.as_bytes()), safe, ext);
    let path = format!("{}{}", LOGO_CRAWL_FOLDER, file_name);
    std::fs::write(&path, bytes).ok()?;
    Some(file_name)
}

/// 从频道列表爬取 logo（带并发保护：同一时间只允许一轮爬取）
pub async fn crawl_logos(m3u: &crate::common::M3uObjectList) {
    if CRAWLING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        info!("logo crawl already running, skip");
        return;
    }
    crawl_logos_inner(m3u).await;
    CRAWLING.store(false, std::sync::atomic::Ordering::SeqCst);
}

/// 实际爬取逻辑：
/// 文件名带频道名标注（优先 tvg-name，其次频道名）：{md5}__{频道名}.{ext}。
/// 已存在但不在索引里的文件会被自动“找回”并重建索引（防止整理区入口消失）。
async fn crawl_logos_inner(m3u: &crate::common::M3uObjectList) {
    let _ = std::fs::create_dir_all(LOGO_CRAWL_FOLDER);
    // 候选：(id, url, 标注名, 已存在的文件名)
    let mut candidates: Vec<(String, String, String, Option<String>)> = Vec::new();
    {
        let mut seen: HashSet<String> = HashSet::new();
        for obj in m3u.get_list_ref() {
            let Some(ext) = obj.get_extend() else { continue };
            let logo_url = ext.tv_logo.trim().to_string();
            if logo_url.is_empty() || !logo_url.starts_with("http") {
                continue;
            }
            if !seen.insert(logo_url.clone()) {
                continue;
            }
            let id = format!("{:x}", md5::compute(logo_url.as_bytes()));
            let in_index = CRAWLED_LOGOS.lock().unwrap().iter().any(|c| c.id == id);
            if in_index {
                continue;
            }
            // 文件名标注：优先使用 tvg-name（tagname），否则使用频道名
            let tag = ext.tv_name.trim().to_string();
            let raw_name = if !tag.is_empty() {
                tag
            } else {
                obj.get_display_name().to_string()
            };
            let name = sanitize_file_name(&raw_name);
            // 查找本地已有文件：旧格式 {id}.{ext} 或新格式 {id}__*.{ext}
            let mut existing: Option<String> = None;
            for e in ["png", "jpg", "webp", "gif"] {
                let f = format!("{}.{}", id, e);
                if std::path::Path::new(&format!("{}{}", LOGO_CRAWL_FOLDER, f)).exists() {
                    existing = Some(f);
                    break;
                }
            }
            if existing.is_none() {
                if let Ok(rd) = std::fs::read_dir(LOGO_CRAWL_FOLDER) {
                    for entry in rd.flatten() {
                        let fname = entry.file_name().to_string_lossy().to_string();
                        if fname.starts_with(&format!("{}__", id)) {
                            existing = Some(fname);
                            break;
                        }
                    }
                }
            }
            candidates.push((id, logo_url, name, existing));
            if candidates.len() >= 200 {
                break;
            }
        }
    }
    if candidates.is_empty() {
        return;
    }

    let mut new_items = Vec::new();
    // 1) 已有文件：找回进索引，旧格式文件名重命名为带频道名的格式，方便后期整理
    for (id, source_url, name, existing) in candidates.iter().filter(|c| c.3.is_some()) {
        let old_file = existing.clone().unwrap();
        let ext = old_file
            .rsplit('.')
            .next()
            .unwrap_or("png")
            .to_string();
        let annotated = format!("{}__{}.{}", id, name, ext);
        let old_path = format!("{}{}", LOGO_CRAWL_FOLDER, old_file);
        let new_path = format!("{}{}", LOGO_CRAWL_FOLDER, annotated);
        let final_file = if old_file == annotated {
            annotated
        } else if std::path::Path::new(&new_path).exists() {
            // 目标名已存在，保留旧文件名
            old_file
        } else if std::fs::rename(&old_path, &new_path).is_ok() {
            annotated
        } else {
            old_file
        };
        new_items.push(CrawledLogo {
            id: id.clone(),
            file: format!("static/core/logos_crawled/{}", final_file),
            url: format!("{}{}", LOGO_CRAWL_URL_BASE, final_file),
            source_url: source_url.clone(),
            names: vec![name.clone()],
            created_at: now_secs(),
        });
    }
    // 2) 缺失文件：8 并发下载（文件名带频道名标注）
    let to_download: Vec<(String, String, String)> = candidates
        .into_iter()
        .filter(|c| c.3.is_none())
        .map(|(id, url, name, _)| (id, url, name))
        .collect();
    let results = futures::stream::iter(to_download.into_iter())
        .map(|(id, url, name)| async move {
            match download_logo(&url, &name).await {
                Some(file_name) => Some((id, url, name, file_name)),
                None => None,
            }
        })
        .buffer_unordered(8)
        .collect::<Vec<_>>()
        .await;
    for item in results.into_iter().flatten() {
        let (id, source_url, name, file_name) = item;
        new_items.push(CrawledLogo {
            id,
            file: format!("static/core/logos_crawled/{}", file_name),
            url: format!("{}{}", LOGO_CRAWL_URL_BASE, file_name),
            source_url,
            names: vec![name],
            created_at: now_secs(),
        });
    }

    if !new_items.is_empty() {
        let count = new_items.len();
        {
            let mut list = CRAWLED_LOGOS.lock().unwrap();
            list.extend(new_items);
            if list.len() > 500 {
                let drain = list.len() - 500;
                list.drain(0..drain);
            }
        }
        // 注意：必须在释放锁之后再 save()，save 内部会再次加同一把锁，否则死锁
        save();
        info!("crawled {} channel logos", count);
    }
}

// ============================== API ==============================

/// 手动触发台标爬取（后台执行，接口立即返回），供「频道封面配置」页调用
#[post("/media/logos-crawled/crawl")]
async fn crawl_logos_api() -> impl Responder {
    match crate::search::load_m3u_data() {
        Ok(m3u) => {
            tokio::spawn(async move {
                crawl_logos(&m3u).await;
            });
            HttpResponse::Ok().json(serde_json::json!({ "msg": "started" }))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "msg": format!("load m3u failed: {}", e) })),
    }
}

#[get("/media/logos-crawled")]
async fn get_crawled_logos_api() -> impl Responder {
    // 已在正式「频道图标」配置（统一配置 channel_icons.json 或旧 logos.json）里的封面视为已存在，不再展示
    let cfg = crate::config::logos::get_logos_config();
    let mut existing: HashSet<String> = cfg.logos.iter().map(|l| l.url.clone()).collect();
    for item in crate::config::channel_icons::get_channel_icons().items {
        if !item.logo.is_empty() {
            existing.insert(item.logo);
        }
    }
    let mut list = CRAWLED_LOGOS.lock().unwrap().clone();
    let before = list.len();
    list.retain(|c| !existing.contains(&c.url));
    if list.len() != before {
        // 同步清理索引，避免一直占着
        *CRAWLED_LOGOS.lock().unwrap() = list.clone();
        save();
    }
    let total = list.len();
    HttpResponse::Ok().json(serde_json::json!({ "list": list, "total": total }))
}

#[derive(Deserialize)]
pub struct BindLogoReq {
    pub ids: Vec<String>,
    pub names: Vec<String>,
}

/// 绑定频道名规范化：繁体转简体、大写转小写
fn normalize_bind_name(name: &str) -> String {
    crate::common::translate::trad_to_simp(name.trim()).to_lowercase()
}

/// 批量绑定：选中若干爬取封面，绑定到频道名，进入正式封面配置列表（logos.json）。
/// 未传 names 时自动使用每个封面爬取时记录的频道名（优先 tvg-name）。
/// 绑定名称统一做繁体转简体、大写转小写处理。
#[post("/media/logos-crawled/bind")]
async fn bind_crawled_logos_api(req: web::Json<BindLogoReq>) -> impl Responder {
    let custom_names: Vec<String> = req
        .names
        .iter()
        .map(|n| normalize_bind_name(n))
        .filter(|n| !n.is_empty())
        .collect();
    if req.ids.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({ "msg": "ids is required" }));
    }
    let ids: HashSet<String> = req.ids.iter().cloned().collect();
    let mut bound = 0;
    {
        let mut crawled = CRAWLED_LOGOS.lock().unwrap();
        let mut cfg = crate::config::logos::get_logos_config();
        for item in crawled.iter().filter(|c| ids.contains(&c.id)) {
            // 未手动输入频道名时，自动使用爬取时记录的频道名（同样做繁体转简体、大写转小写）
            let names: Vec<String> = if custom_names.is_empty() {
                if item.names.is_empty() {
                    continue;
                }
                item.names
                    .iter()
                    .map(|n| normalize_bind_name(n))
                    .filter(|n| !n.is_empty())
                    .collect()
            } else {
                custom_names.clone()
            };
            if let Some(existing) = cfg.logos.iter_mut().find(|l| l.url == item.url) {
                for n in &names {
                    if !existing.name.contains(n) {
                        existing.name.push(n.clone());
                    }
                }
            } else {
                cfg.logos.push(crate::config::logos::LogoItem {
                    url: item.url.clone(),
                    name: names.clone(),
                });
            }
            // 同步写入统一频道图标配置（tvg-id / 分组留空，后续在「频道图标」页编辑）
            crate::config::channel_icons::upsert_item(crate::config::channel_icons::ChannelIconItem {
                name: names[0].clone(),
                aliases: names.iter().skip(1).cloned().collect(),
                tvg_id: String::new(),
                group1: String::new(),
                group2: String::new(),
                group: String::new(),
                logo: item.url.clone(),
            });
            bound += 1;
        }
        if bound > 0 {
            let _ = crate::config::logos::update_logos_config(cfg);
        }
        crawled.retain(|c| !ids.contains(&c.id));
    }
    // 释放锁后再 save（save 内部会重新加锁，避免死锁）
    save();
    info!("bound {} crawled logos to channels", bound);
    HttpResponse::Ok().json(serde_json::json!({ "msg": "bound", "count": bound }))
}

#[delete("/media/logos-crawled/{id}")]
async fn delete_crawled_logo_api(path: web::Path<String>) -> impl Responder {
    let id = path.into_inner();
    let removed = {
        let mut list = CRAWLED_LOGOS.lock().unwrap();
        let idx = list.iter().position(|c| c.id == id);
        idx.map(|i| list.remove(i))
    };
    match removed {
        Some(item) => {
            let _ = std::fs::remove_file(format!("./{}", item.file));
            save();
            HttpResponse::Ok().json(serde_json::json!({ "msg": "deleted", "id": id }))
        }
        None => HttpResponse::NotFound().json(serde_json::json!({ "msg": "not found" })),
    }
}

#[delete("/media/logos-crawled")]
async fn clear_crawled_logos_api() -> impl Responder {
    let count = {
        let mut list = CRAWLED_LOGOS.lock().unwrap();
        let count = list.len();
        list.clear();
        count
    };
    save();
    // 删除爬取文件（已绑定的文件在 logos.json 中仍被引用，仅清理未绑定的索引文件）
    if let Ok(entries) = std::fs::read_dir(LOGO_CRAWL_FOLDER) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    info!("cleared {} crawled logos", count);
    HttpResponse::Ok().json(serde_json::json!({ "msg": "cleared", "count": count }))
}

/// 注册路由
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(get_crawled_logos_api)
        .service(bind_crawled_logos_api)
        .service(delete_crawled_logo_api)
        .service(clear_crawled_logos_api)
        .service(crawl_logos_api);
}

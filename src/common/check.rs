use crate::common;
use crate::common::m3u::m3u::list_str2obj;
use crate::common::util::from_video_resolution;
use crate::common::{AudioInfo, CheckOptions, SearchOptions, VideoInfo};
use crate::config::favourite::get_favourite_list;
use crate::r#const::constant::{INPUT_SEARCH_FOLDER, OUTPUT_FOLDER};
use lazy_static::lazy_static;
use log::info;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Error;
use std::sync::Mutex;

lazy_static! {
    /// 本次检测被 body 校验直接丢弃的无效 m3u8 数量（检测报告统计用，每次检测开始前清零）
    pub static ref CHECK_REPORT_INVALID_M3U8: Mutex<u64> = Mutex::new(0);

    /// 本次检测的失败原因统计（key: 归类后的原因，value: 次数）。
    /// 之前失败原因被直接丢弃，日志里只看到「失败 N 个」，无法定位是 ffprobe 缺参数、
    /// 源站超时还是 UA 被拒，只能靠猜。
    pub static ref CHECK_FAIL_REASONS: Mutex<HashMap<String, u32>> = Mutex::new(HashMap::new());
}

/// 归类失败原因：去掉带具体 URL 的易变部分并截断，便于聚合统计
fn normalize_fail_reason(reason: &str) -> String {
    let mut s = reason.trim().to_string();
    for marker in [" for url (", " for url ", " url: "] {
        if let Some(idx) = s.find(marker) {
            s.truncate(idx);
            break;
        }
    }
    if s.chars().count() > 140 {
        s = s.chars().take(140).collect::<String>() + "…";
    }
    s
}

/// 记录一次失败原因（供检测报告 / 日志汇总）
pub fn record_fail_reason(reason: &str) {
    let key = normalize_fail_reason(reason);
    if key.is_empty() {
        return;
    }
    if let Ok(mut map) = CHECK_FAIL_REASONS.lock() {
        *map.entry(key).or_insert(0) += 1;
    }
}

/// 失败原因 TOP N（按次数倒序）
pub fn top_fail_reasons(n: usize) -> Vec<FailReason> {
    let map = match CHECK_FAIL_REASONS.lock() {
        Ok(m) => m,
        Err(_) => return vec![],
    };
    let mut v: Vec<FailReason> = map
        .iter()
        .map(|(k, c)| FailReason {
            reason: k.clone(),
            count: *c,
        })
        .collect();
    v.sort_by(|a, b| b.count.cmp(&a.count));
    v.truncate(n);
    v
}

/// 取 URL 的 path 部分（去掉查询参数/锚点）并转小写
fn url_path_lower(url: &str) -> String {
    let path = url.split(|c: char| c == '?' || c == '#').next().unwrap_or(url);
    path.to_ascii_lowercase()
}

/// 判断 URL 是否为 m3u8 后缀（忽略大小写与查询参数/锚点）
pub fn url_is_m3u8(url: &str) -> bool {
    url_path_lower(url).ends_with(".m3u8")
}

/// m3u8 链接预校验：HTTP 拉取 body（最多读前 4KB），检查是否为正规 m3u8
async fn http_body_is_m3u8(
    url: &str,
    timeout_ms: u64,
    headers: &[(&str, &str)],
) -> Result<bool, std::io::Error> {
    use futures::StreamExt;
    // 预校验用较短超时（最多 10s），死链快速失败；先直连、失败走代理
    let t = timeout_ms.min(10_000);
    let resp = crate::common::util::request_with_fallback(url, headers, t / 1000 + 1)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("GET failed: {}", e)))?;
    if !resp.status().is_success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("status {}", resp.status()),
        ));
    }
    // 只读前 4KB：判断 m3u8 格式不需要整个 body，避免大 playlist 白白下载拖慢检查
    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(c) => {
                buf.extend_from_slice(&c);
                if buf.len() >= 4096 {
                    break;
                }
            }
            Err(e) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("read failed: {}", e),
                ));
            }
        }
    }
    let head = String::from_utf8_lossy(&buf[..buf.len().min(4096)]).to_string();
    Ok(crate::common::util::check_body_is_m3u8_format(head))
}

/// 检测报告：失败原因统计项
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FailReason {
    pub reason: String,
    pub count: u32,
}

/// 检测报告：按格式统计一次检查的结果
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CheckReport {
    pub output_id: String,
    pub generated_at: u64,
    pub total: usize,
    pub m3u8_total: usize,
    pub m3u8_invalid: usize,
    pub rtmp: usize,
    pub rtsp: usize,
    pub flv: usize,
    pub ts: usize,
    pub mp4: usize,
    pub other: usize,
    pub success: usize,
    pub failed: usize,
    /// 失败原因 TOP（便于直接看出「为什么全失败」）
    #[serde(default)]
    pub fail_reasons: Vec<FailReason>,
}

fn build_check_report(data: &crate::common::M3uObjectList, output_id: &str) -> CheckReport {
    let mut r = CheckReport {
        output_id: output_id.to_string(),
        generated_at: chrono::Utc::now().timestamp() as u64,
        total: 0,
        m3u8_total: 0,
        m3u8_invalid: 0,
        rtmp: 0,
        rtsp: 0,
        flv: 0,
        ts: 0,
        mp4: 0,
        other: 0,
        success: 0,
        failed: 0,
        fail_reasons: vec![],
    };
    for obj in data.get_list_ref() {
        r.total += 1;
        let url = obj.get_url();
        let path = url_path_lower(&url);
        if path.ends_with(".m3u8") {
            r.m3u8_total += 1;
        } else if url.starts_with("rtmp://") {
            r.rtmp += 1;
        } else if url.starts_with("rtsp://") {
            r.rtsp += 1;
        } else if path.ends_with(".flv") {
            r.flv += 1;
        } else if path.ends_with(".ts") {
            r.ts += 1;
        } else if path.ends_with(".mp4") {
            r.mp4 += 1;
        } else {
            r.other += 1;
        }
        match obj.get_status() {
            crate::common::CheckDataStatus::Success => r.success += 1,
            crate::common::CheckDataStatus::Failed => r.failed += 1,
            _ => {}
        }
    }
    r.m3u8_invalid = *CHECK_REPORT_INVALID_M3U8.lock().unwrap() as usize;
    r.fail_reasons = top_fail_reasons(8);
    r
}

/// URL检查响应结构体
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CheckUrlIsAvailableResponse {
    pub delay: i32, // 延迟时间（毫秒）
    pub ffmpeg_info: Option<FfmpegInfo>,
    pub video: Option<VideoInfo>, // 视频信息
    pub audio: Option<AudioInfo>, // 音频信息
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FfmpegInfo {
    pub video: Vec<VideoInfo>,    // 视频信息
    pub audio: Option<AudioInfo>, // 音频信息
}

impl FfmpegInfo {
    pub fn new() -> FfmpegInfo {
        FfmpegInfo {
            video: Vec::new(),
            audio: None,
        }
    }

    pub fn set_audio(&mut self, audio: AudioInfo) {
        self.audio = Some(audio);
    }

    pub fn set_video(&mut self, video: Vec<VideoInfo>) {
        self.video = video;
    }
}
impl CheckUrlIsAvailableResponse {
    /// 创建新的检查响应
    pub fn new() -> CheckUrlIsAvailableResponse {
        CheckUrlIsAvailableResponse {
            delay: 0,
            ffmpeg_info: None,
            video: None,
            audio: None,
        }
    }

    /// 设置延迟时间
    pub fn set_delay(&mut self, delay: i32) {
        self.delay = delay
    }

    /// 设置视频信息
    pub fn set_ffmpeg_info(&mut self, video: FfmpegInfo) {
        self.ffmpeg_info = Some(video)
    }
}

// #[derive(Serialize, Deserialize)]
// pub struct CheckUrlIsAvailableRespAudio {
//     pub codec: String,
//     pub channels: i32,
//     #[serde(rename = "bitRate")]
//     pub bit_rate: i32,
// }

// impl CheckUrlIsAvailableRespAudio {
//     pub fn new() -> CheckUrlIsAvailableRespAudio {
//         CheckUrlIsAvailableRespAudio {
//             codec: "".to_string(),
//             channels: 0,
//             bit_rate: 0,
//         }
//     }
//
//     pub fn set_codec(&mut self, codec: String) {
//         self.codec = codec
//     }
//
//     pub fn set_channels(&mut self, channels: i32) {
//         self.channels = channels
//     }
//     pub fn set_bit_rate(&mut self, bit_rate: i32) {
//         self.bit_rate = bit_rate
//     }
//
//     pub fn get_bit_rate(self) -> i32 {
//         self.bit_rate
//     }
//     pub fn get_channels(self) -> i32 {
//         self.channels
//     }
//     pub fn get_codec(self) -> String {
//         self.codec
//     }
// }

// #[derive(Serialize, Deserialize)]
// pub struct CheckUrlIsAvailableRespVideo {
//     width: i32,
//     height: i32,
//     codec: String,
//     #[serde(rename = "bitRate")]
//     bit_rate: i32,
// }

/// FFprobe输出结构体
#[derive(Debug, Deserialize, Serialize)]
pub struct Ffprobe {
    streams: Vec<FfprobeStream>, // 流信息列表
}

/// FFprobe流信息结构体
#[derive(Debug, Deserialize, Serialize)]
pub struct FfprobeStream {
    #[serde(default)]
    codec_type: String, // 编码类型
    width: Option<i32>,  // 视频宽度
    height: Option<i32>, // 视频高度
    #[serde(default)]
    codec_name: String, // 编码名称
    channels: Option<i32>, // 音频通道数
}

/// 检查模块
pub mod check {
    use crate::common::util::{check_body_is_m3u8_format, get_video_resolution};
    use crate::common::{AudioInfo, CheckUrlIsAvailableResponse, FfmpegInfo, Ffprobe, VideoInfo};
    use chrono::Utc;
    use log::debug;
    use std::io::{Error, ErrorKind, Read};
    use std::process::{Command, ExitStatus, Stdio};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Instant;
    use tokio::time::Duration;
    use url::Url;

    /// 取字符串尾部至多 max 个字符（按字符边界切分，避免 UTF-8 截断 panic）
    fn tail_chars(s: &str, max: usize) -> String {
        let count = s.chars().count();
        if count <= max {
            return s.to_string();
        }
        s.chars().skip(count - max).collect()
    }

    /// 使用 ffprobe 探测流信息（带超时）
    ///
    /// # 参数
    /// * `_url` - 要检查的URL
    /// * `timeout_mill_secs` - 本次探测的整体超时（毫秒）
    /// * `channel_ua` - 播放列表里为该频道声明的 User-Agent（http-user-agent / EXTVLCOPT）
    ///
    /// # 返回值
    /// * `Result<CheckUrlIsAvailableResponse, Error>` - 检查结果
    pub async fn run_command_with_timeout_new(
        _url: String,
        timeout_mill_secs: u64,
        channel_ua: Option<&str>,
    ) -> Result<CheckUrlIsAvailableResponse, Error> {
        let timeout = Duration::from_millis(timeout_mill_secs);
        // 修复：socket I/O 超时的单位是「微秒」。
        // 之前按秒传（-timeout 20 实际是 20 微秒），远程源第一次读 socket 就超时，
        // 于是「播放器能正常播放」的源在 ffmpeg 检查里全部被判失败（检查结果为空）。
        let io_timeout_us = timeout_mill_secs.saturating_mul(1000).max(1_000_000);
        let (ua, headers) = crate::common::util::probe_request_headers(channel_ua);

        // 1. 配置FFprobe命令（优先使用项目自带二进制，缺失时回退到 PATH）
        let mut cmd = Command::new(crate::common::util::ffprobe_bin());
        // ffprobe 探测遵循网络代理配置
        crate::common::util::apply_proxy_to_command(&mut cmd);
        cmd.arg("-v")
            .arg("error")
            .arg("-print_format")
            .arg("json")
            .arg("-show_format")
            .arg("-show_streams")
            .arg("-rw_timeout")
            .arg(io_timeout_us.to_string());
        // 频道自带 UA / 全局自定义请求头：播放器会带，检查链路也必须带，
        // 否则需要特定 UA / Referer 的源会 403（播放器能播、检查全失败）
        if let Some(ua) = ua {
            cmd.arg("-user_agent").arg(ua);
        }
        if !headers.is_empty() {
            let header_str: String = headers
                .iter()
                .map(|(k, v)| format!("{}: {}\r\n", k, v))
                .collect();
            cmd.arg("-headers").arg(header_str);
        }
        cmd.arg(&_url);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // 启动子进程：ffprobe 缺失时返回错误而不是 panic。
        // 之前这里 unwrap() 会让整个检查线程直接退出，上层收不到结果会一直空转死等，
        // 任务永远不结束、报告和输出文件都不生成（表现为「检查没有任何结果」）。
        let mut child = cmd
            .spawn()
            .map_err(|e| Error::new(ErrorKind::Other, format!("failed to run ffprobe: {}", e)))?;

        // 2. 获取标准输出和错误输出的管道句柄
        let stdout_handle = child
            .stdout
            .take()
            .ok_or_else(|| Error::new(ErrorKind::Other, "failed to open stdout pipe"))?;
        let stderr_handle = child
            .stderr
            .take()
            .ok_or_else(|| Error::new(ErrorKind::Other, "failed to open stderr pipe"))?;

        // 3. 创建共享缓冲区用于存储输出
        let stdout_buf = Arc::new(Mutex::new(Vec::new()));
        let stderr_buf = Arc::new(Mutex::new(Vec::new()));

        // 克隆Arc以便在线程间共享
        let stdout_buf_clone = Arc::clone(&stdout_buf);
        let stderr_buf_clone = Arc::clone(&stderr_buf);

        // 4. 启动标准输出读取线程
        let stdout_thread = thread::spawn(move || {
            let mut buffer = [0; 1024];
            let mut handle = stdout_handle;
            loop {
                match handle.read(&mut buffer) {
                    Ok(0) => break, // 文件结束
                    Ok(n) => {
                        let mut locked_buf = stdout_buf_clone.lock().unwrap();
                        locked_buf.extend_from_slice(&buffer[..n]);
                    }
                    Err(ref e) if e.kind() == ErrorKind::BrokenPipe => break,
                    Err(e) => {
                        eprintln!("Error reading stdout: {}", e);
                        break;
                    }
                }
            }
        });

        // 5. 启动标准错误读取线程
        let stderr_thread = thread::spawn(move || {
            let mut buffer = [0; 1024];
            let mut handle = stderr_handle;
            loop {
                match handle.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        let mut locked_buf = stderr_buf_clone.lock().unwrap();
                        locked_buf.extend_from_slice(&buffer[..n]);
                    }
                    Err(ref e) if e.kind() == ErrorKind::BrokenPipe => break,
                    Err(e) => {
                        eprintln!("Error reading stderr: {}", e);
                        break;
                    }
                }
            }
        });

        // 6. 主线程执行超时检查和进程状态监控
        let start = Instant::now();
        let final_status: ExitStatus;
        let mut timed_out = false;

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    final_status = status;
                    break;
                }
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        match child.kill() {
                            Ok(_) => debug!("Process killed due to timeout."),
                            Err(e) => {
                                debug!("Warning: Failed to kill process after timeout: {}", e)
                            }
                        }
                        timed_out = true;
                        thread::sleep(Duration::from_millis(50));
                        final_status = child
                            .try_wait()
                            .map_err(|e| format!("Error checking status after kill: {}", e))
                            .unwrap()
                            .unwrap_or_else(|| {
                                debug!(
                                    "Warning: Process did not exit immediately after kill signal."
                                );
                                ExitStatus::default()
                            });
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    stdout_thread.join().expect("Stdout thread panicked");
                    stderr_thread.join().expect("Stderr thread panicked");
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!("Failed to wait on child process: {}", e),
                    ));
                }
            }
        }

        // 7. 等待读取线程完成
        stdout_thread.join().expect("Stdout thread panicked");
        stderr_thread.join().expect("Stderr thread panicked");

        // 8. 处理超时情况（附带 ffprobe 的报错尾部，方便定位）
        if timed_out {
            let reason = {
                let buf = stderr_buf.lock().unwrap();
                tail_chars(&String::from_utf8_lossy(&buf), 300)
            };
            let reason = reason.trim();
            return Err(Error::new(
                ErrorKind::TimedOut,
                if reason.is_empty() {
                    format!("ffprobe timed out after {}ms", timeout_mill_secs)
                } else {
                    format!("ffprobe timed out after {}ms: {}", timeout_mill_secs, reason)
                },
            ));
        }

        // 9. 检查进程退出状态（把 ffprobe 的报错带出来，之前 -v quiet 什么都看不到）
        if !final_status.success() {
            let reason = {
                let buf = stderr_buf.lock().unwrap();
                tail_chars(&String::from_utf8_lossy(&buf), 300)
            };
            let reason = reason.trim();
            return Err(Error::new(
                ErrorKind::Other,
                if reason.is_empty() {
                    format!("ffprobe exited with status: {}", final_status)
                } else {
                    format!("ffprobe failed: {}", reason)
                },
            ));
        }

        // 10. 解析FFprobe输出
        let stdout_data = stdout_buf.lock().unwrap();
        let output = String::from_utf8_lossy(&stdout_data);
        let ffprobe: Ffprobe = serde_json::from_str(&output).map_err(|e| {
            Error::new(
                ErrorKind::InvalidData,
                format!(
                    "failed to parse ffprobe output: {} (output: {})",
                    e,
                    tail_chars(&output, 200)
                ),
            )
        })?;

        // 11. 处理流信息
        let mut response = CheckUrlIsAvailableResponse::new();
        let mut audio = None;
        let mut video_list = vec![];
        for stream in ffprobe.streams {
            match stream.codec_type.as_str() {
                "video" => {
                    let mut video_info = VideoInfo::new();
                    if let Some(width) = stream.width {
                        video_info.set_width(width);
                    }
                    if let Some(height) = stream.height {
                        video_info.set_height(height);
                        video_info.quality_type = get_video_resolution(height as u32);
                    }
                    video_info.set_codec(stream.codec_name);
                    video_list.push(video_info);
                }
                "audio" => {
                    let mut audio_info = AudioInfo::new();
                    if let Some(channels) = stream.channels {
                        audio_info.set_channels(channels);
                    }
                    audio_info.set_codec(stream.codec_name);
                    audio = Some(audio_info);
                }
                _ => {}
            }
        }
        if audio.is_some() || !video_list.is_empty() {
            let mut ffmpeg_info = FfmpegInfo::new();
            if audio.is_some() {
                ffmpeg_info.set_audio(audio.unwrap());
            }
            ffmpeg_info.set_video(video_list);
            response.set_ffmpeg_info(ffmpeg_info);
        }
        Ok(response)
    }

    /// 检查链接是否有效
    ///
    /// # 参数
    /// * `_url` - 要检查的URL
    /// * `timeout` - 超时时间（毫秒）
    /// * `ffmpeg_check` - 是否使用FFmpeg检查
    /// * `not_http_skip` - 是否跳过非HTTP链接
    /// * `channel_ua` - 播放列表为该频道声明的 User-Agent（播放器会带，检查也必须带）
    ///
    /// # 返回值
    /// * `Result<CheckUrlIsAvailableResponse, Error>` - 检查结果
    pub async fn check_link_is_valid(
        _url: String,
        timeout: u64,
        ffmpeg_check: bool,
        not_http_skip: bool,
        channel_ua: Option<String>,
    ) -> Result<CheckUrlIsAvailableResponse, Error> {
        // 频道自带 UA 优先、其次全局配置；预校验与 ffprobe 使用同一套请求头
        let req_headers = crate::common::util::check_request_headers(channel_ua.as_deref());
        let header_refs: Vec<(&str, &str)> = req_headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        // m3u8 后缀的链接先做 HTTP body 预校验：
        // 不是正规 m3u8（或拉不到 body）的直接丢弃，不再浪费 ffprobe 检测时间，
        // 保证交给 ffprobe 的都是合法链接
        if super::url_is_m3u8(&_url) {
            match super::http_body_is_m3u8(&_url, timeout, &header_refs).await {
                Ok(true) => {}
                Ok(false) => {
                    *super::CHECK_REPORT_INVALID_M3U8.lock().unwrap() += 1;
                    return Err(Error::new(ErrorKind::Other, "not a valid m3u8 playlist"));
                }
                Err(e) => {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!("m3u8 body check failed: {}", e),
                    ));
                }
            }
        }
        if ffmpeg_check {
            // 失败原因原样返回（之前会被套上 "status is not 200"，把真实原因盖掉）
            return run_command_with_timeout_new(_url.to_owned(), timeout, channel_ua.as_deref())
                .await;
        }
        let parsed_info = Url::parse(&_url);
        match parsed_info {
            Ok(parsed_url) => {
                if parsed_url.scheme() != "https" && parsed_url.scheme() != "http" {
                    return if not_http_skip {
                        Ok(CheckUrlIsAvailableResponse::new())
                    } else {
                        Err(Error::new(
                            ErrorKind::Other,
                            "scheme not http, temporary not support",
                        ))
                    };
                }
            }
            Err(e) => {
                return Err(Error::new(ErrorKind::Other, format!("error {}", e)));
            }
        }
        let curr_timestamp = Utc::now().timestamp_millis();
        // 先直连、失败走代理（国内源直连、国外源走代理）
        let http_res = crate::common::util::request_with_fallback(
            _url.as_str(),
            &header_refs,
            (timeout / 1000).max(1),
        )
        .await;
        match http_res {
            Ok(res) => {
                if res.status().is_success() {
                    let delay = Utc::now().timestamp_millis() - curr_timestamp;
                    let _body = res.text().await;
                    match _body {
                        Ok(body) => {
                            if check_body_is_m3u8_format(body.clone()) {
                                let mut body: CheckUrlIsAvailableResponse =
                                    CheckUrlIsAvailableResponse::new();
                                body.set_delay(delay as i32);
                                Ok(body)
                            } else {
                                Err(Error::new(ErrorKind::Other, "not a m3u8 file"))
                            }
                        }
                        Err(e) => Err(Error::new(ErrorKind::Other, format!("{:?}", e))),
                    }
                } else {
                    Err(Error::new(ErrorKind::Other, "status is not 200"))
                }
            }
            Err(e) => {
                return Err(Error::new(ErrorKind::Other, format!("error {}", e)));
            }
        }
    }

    /// 测试模块
    mod tests {
        // use crate::common::check::check::run_command_with_timeout_new;
        // use std::sync::mpsc;
        // use std::thread;
        // #[tokio::test]
        // async fn test_timeout() {
        //     let (tx, rx) = mpsc::channel();
        //
        //     // 模拟从channel里收到一条命令
        //     thread::spawn(move || {
        //         tx.send(("https://cd-live-stream.news.cctvplus.com/live/smil:CHANNEL2.smil/playlist.m3u8", 5000)).unwrap();
        //         // 比如要执行sleep 5秒
        //     });
        //
        //     if let Ok((_url, timeout)) = rx.recv() {
        //         println!("Running command: {} {:?}", _url, timeout);
        //         match run_command_with_timeout_new(_url.to_string(), (timeout as u64)).await {
        //             Ok(ed) => {
        //                 let v = ed.video.clone().unwrap();
        //                 println!("Command finished successfully.{} {}", v.width, v.height)
        //             }
        //             Err(e) => println!("Command failed: {}", e),
        //         }
        //     }
        // }
    }
}

pub async fn get_favourite_channel(channel_type: String) -> Result<String, Error> {
    // 获取今日日期对应目录
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    let search_path = format!("{}/{}", INPUT_SEARCH_FOLDER, today);

    let mut all_files = Vec::new();
    let dir_entries = match std::fs::read_dir(&search_path) {
        Ok(entries) => entries,
        Err(e) => {
            return Ok("".to_string());
        }
    };

    for entry in dir_entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_file() {
                    if let Some(file_str) = path.to_str() {
                        all_files.push(file_str.to_string());
                    }
                }
            }
            Err(e) => {
                // 遇到单个文件错误也继续处理其他文件
                continue;
            }
        }
    }

    // 将文件转换为数组
    let list = common::m3u::m3u::from_arr(all_files.to_owned(), 0).await;
    // 将数组转换为对象
    let mut data = list_str2obj(list, false);
    // 将频道名繁体转简体
    data.t2s();
    // 去除name中无效的字符
    data.remove_useless_char();
    // 频道图标取数：从本次频道数据（favourite-channel / 检查任务源数据）自动收集 tvg-logo，
    // 只补充还没有图标的频道，不覆盖人工配置
    crate::config::channel_icons::collect_from_m3u(&data);
    let mut keyword_full_match = vec![];
    let mut keyword_like = vec![];
    if channel_type == "like" {
        keyword_full_match = get_favourite_list("equal").to_owned();
        keyword_like = get_favourite_list("like").to_owned();
    }
    // 搜索关键字
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
    let rename_channel_type = crate::config::base::get_base_config().rename_channel_type;
    let host = {
        let base_host = crate::config::base::get_base_config().host;
        if !base_host.trim().is_empty() {
            base_host
        } else {
            crate::config::logos::get_logos_config().host
        }
    };
    if !host.is_empty() {
        // 统一频道图标配置优先，旧 logos.json 作为兜底
        let mut logos_map = crate::config::channel_icons::get_logo_map();
        for (k, v) in crate::config::logos::get_logos_map() {
            logos_map.entry(k).or_insert(v);
        }
        data.replace_logos(host, &logos_map);
    }
    return Ok(data.get_m3u_content_str(rename_channel_type,false));
}

pub async fn do_check(
    input_files: Vec<String>,
    output_id: String,
    timeout: i32,
    print_result: bool,
    request_timeout: i32,
    concurrent: i32,
    keyword_like: Vec<String>,
    keyword_dislike: Vec<String>,
    sort: bool,
    no_check: bool,
    ffmpeg_check: bool,
    same_save_num: i32,
    not_http_skip: bool,
    video_quality: Vec<String>,
    export_file: bool,
    rename_channel_type: i8,
    fast_sort: bool,
) -> Result<bool, std::io::Error> {
    // ffmpeg 检查依赖 ffprobe：缺失时立刻报错中止。
    // 否则每个频道都会被判失败并写进黑名单，用户看到的就是「检查没有任何结果」，
    // 而且后续检查还会因为黑名单继续空转。
    if ffmpeg_check && !no_check {
        if let Err(e) = crate::common::util::check_ffprobe_available() {
            log::error!(
                "ffmpeg 检查已开启，但服务端 ffprobe 不可用（{}）。已中止本次检查：\
                 请安装完整的 ffmpeg（含 ffprobe）或改用 http 快速检查。",
                e
            );
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("ffprobe unavailable: {}", e),
            ));
        }
    }
    // 将文件转换为数组
    let list = common::m3u::m3u::from_arr(input_files.to_owned(), timeout as u64).await;
    // 将数组转换为对象
    let mut data = list_str2obj(list, false);
    // 获取域名对应的ip类型
    data.to_ip_address();
    // 将频道名繁体转简体
    data.t2s();
    // 去除name中无效的字符
    data.remove_useless_char();
    // 频道图标取数：从每个检查任务的源数据自动收集 tvg-logo，
    // 只补充还没有图标的频道，不覆盖人工配置
    crate::config::channel_icons::collect_from_m3u(&data);
    // 搜索关键字
    data.search(SearchOptions {
        keyword_full_match: vec![],
        keyword_like,
        keyword_dislike,
        ipv4: false,
        ipv6: false,
        exclude_url: vec![],
        exclude_host: vec![],
    })
    .await;
    // 重置本次检测的无效 m3u8 计数与失败原因统计
    *CHECK_REPORT_INVALID_M3U8.lock().unwrap() = 0;
    CHECK_FAIL_REASONS.lock().unwrap().clear();
    // 过滤黑名单源（连续失败达到阈值的源直接跳过，提升检查速度）
    let blacklisted = crate::check_blacklist::get_blacklisted_urls();
    if !blacklisted.is_empty() {
        let filtered = data.filter_urls(&blacklisted);
        if filtered > 0 {
            info!("check blacklist filtered {} sources before checking", filtered);
        }
    }
    // 检查数据
    data.check_data_new(CheckOptions {
        request_time: request_timeout,
        concurrent,
        sort,
        no_check,
        ffmpeg_check,
        same_save_num,
        not_http_skip,
        fast_sort,
    })
    .await;
    println!("entry video quality {:?}", video_quality.clone());
    if ffmpeg_check {
        if no_check {
            log::warn!(
                "video_quality filter skipped: no_check=true, ffmpeg info not collected. \
                 Set no_check=false to enable quality filtering."
            );
        } else {
            data.search_video_quality(from_video_resolution(video_quality));
        }
    } else if !video_quality.is_empty() {
        log::warn!(
            "video_quality is set to {:?} but ffmpeg_check=false, quality filter will not apply. \
             Set ffmpeg_check=true to enable quality filtering.",
            video_quality
        );
    }
    let output_file = format!("{}{}.json", OUTPUT_FOLDER, output_id);
    if print_result {
        info!("输出文件: {}", output_file);
    }
    data.save_raw_data(output_file);
    // 生成并保存检测报告（按格式统计，方便后期人工查看）
    let report = build_check_report(&data, &output_id);
    let report_path = format!("{}{}_report.json", OUTPUT_FOLDER, output_id);
    if let Ok(json) = serde_json::to_string_pretty(&report) {
        let _ = std::fs::write(&report_path, json);
    }
    info!(
        "检测报告[{}]: 总数 {} | m3u8 {} (无效 {}) | rtmp {} | rtsp {} | flv {} | ts {} | mp4 {} | 其他 {} | 成功 {} | 失败 {}",
        output_id,
        report.total,
        report.m3u8_total,
        report.m3u8_invalid,
        report.rtmp,
        report.rtsp,
        report.flv,
        report.ts,
        report.mp4,
        report.other,
        report.success,
        report.failed,
    );
    // 失败原因 TOP：直接回答「为什么全失败」，不用再去翻 debug 日志
    if !report.fail_reasons.is_empty() {
        let detail = report
            .fail_reasons
            .iter()
            .map(|r| format!("{} ×{}", r.reason, r.count))
            .collect::<Vec<String>>()
            .join(" | ");
        log::warn!("检测失败原因[{}]: {}", output_id, detail);
    }
    // 导出数据
    if export_file {
        data.output_file(
            format!("{}{}.m3u", OUTPUT_FOLDER, output_id),
            true,
            rename_channel_type,
        )
        .await;
    }
    if print_result {
        if !no_check {
            let status_string = data.print_result();
            info!("\n{}", status_string);
        }
        info!("解析完成----")
    }
    // 更新检查黑名单统计：成功移出、失败累计（达到阈值即拉黑）
    if !no_check {
        let list = data.get_list();
        let mut success_count = 0;
        let mut failed_count = 0;
        for obj in list {
            let url = obj.get_url();
            if !url.starts_with("http://") && !url.starts_with("https://") {
                continue;
            }
            match obj.get_status() {
                crate::common::CheckDataStatus::Success => {
                    crate::check_blacklist::mark_success(&url);
                    success_count += 1;
                }
                crate::common::CheckDataStatus::Failed => {
                    crate::check_blacklist::mark_failed(&url);
                    failed_count += 1;
                }
                _ => {}
            }
        }
        if failed_count > 0 {
            info!(
                "check blacklist updated: {} success, {} failed",
                success_count, failed_count
            );
        }
    }
    Ok(true)
}

// 测试模块
#[cfg(test)]
mod tests {
    use crate::common::check::check::run_command_with_timeout_new;
    use std::sync::mpsc;
    use std::thread;
    #[tokio::test]
    async fn test_timeout() {
        let (tx, rx) = mpsc::channel();

        // 模拟从channel里收到一条命令
        thread::spawn(move || {
            tx.send((
                "https://cd-live-stream.news.cctvplus.com/live/smil:CHANNEL2.smil/playlist.m3u8",
                5000,
            ))
            .unwrap(); // 比如要执行sleep 5秒
        });

        if let Ok((_url, timeout)) = rx.recv() {
            println!("Running command: {} {:?}", _url, timeout);
            match run_command_with_timeout_new(_url.to_string(), (timeout as u64), None).await {
                Ok(ed) => {
                    for v in ed.ffmpeg_info.unwrap().video.clone() {
                        println!("Command finished successfully.{} {}", v.width, v.height)
                    }
                }
                Err(e) => println!("Command failed: {}", e),
            }
        }
    }

    /// BOM 播放列表必须能被识别（否则会解析出 0 个频道）
    #[test]
    fn test_bom_playlist_is_normal_format() {
        use crate::common::m3u::m3u::check_source_type;
        let with_bom = "\u{feff}#EXTM3U\n#EXTINF:-1 tvg-name=\"A\",A\nhttp://127.0.0.1/a.m3u8\n";
        assert!(check_source_type(with_bom.to_string()).is_some());
        assert!(crate::common::util::check_body_is_m3u8_format(
            with_bom.to_string()
        ));
        assert!(crate::common::util::check_body_is_m3u8_format(
            "#EXTM3U\n".to_string()
        ));
        assert!(!crate::common::util::check_body_is_m3u8_format(
            "<html>403</html>".to_string()
        ));
    }

    /// 失败原因要能归类（去掉 URL 等易变内容），否则统计会变成一堆独立条目
    #[test]
    fn test_normalize_fail_reason() {
        let a = super::normalize_fail_reason(
            "m3u8 body check failed: GET failed: error sending request for url (http://a.b/c.m3u8)",
        );
        let b = super::normalize_fail_reason(
            "m3u8 body check failed: GET failed: error sending request for url (http://x.y/z.m3u8)",
        );
        assert_eq!(a, b);
        assert!(a.starts_with("m3u8 body check failed"));
        assert!(super::normalize_fail_reason(&"x".repeat(500)).chars().count() <= 141);
    }
}

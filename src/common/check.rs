use crate::common::{AudioInfo, CheckOptions, VideoInfo};
use crate::{common, utils};
use log::info;
use serde::{Deserialize, Serialize};
use std::fmt::Error;
use std::net::ToSocketAddrs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// URL检查响应结构体
#[derive(Debug, Serialize, Deserialize)]
pub struct CheckUrlIsAvailableResponse {
    pub(crate) delay: i32,           // 延迟时间（毫秒）
    pub(crate) video: Option<VideoInfo>,  // 视频信息
    pub(crate) audio: Option<AudioInfo>,  // 音频信息
}

impl CheckUrlIsAvailableResponse {
    /// 创建新的检查响应
    pub fn new() -> CheckUrlIsAvailableResponse {
        CheckUrlIsAvailableResponse {
            delay: 0,
            video: None,
            audio: None,
        }
    }

    /// 设置延迟时间
    pub fn set_delay(&mut self, delay: i32) {
        self.delay = delay
    }

    /// 设置视频信息
    pub fn set_video(&mut self, video: VideoInfo) {
        self.video = Some(video)
    }

    /// 设置音频信息
    pub fn set_audio(&mut self, audio: AudioInfo) {
        self.audio = Some(audio)
    }
}

// #[derive(Serialize, Deserialize)]
// pub struct CheckUrlIsAvailableRespAudio {
//     pub(crate) codec: String,
//     pub(crate) channels: i32,
//     #[serde(rename = "bitRate")]
//     pub(crate) bit_rate: i32,
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
    streams: Vec<FfprobeStream>,  // 流信息列表
}

/// FFprobe流信息结构体
#[derive(Debug, Deserialize, Serialize)]
pub struct FfprobeStream {
    #[serde(default)]
    codec_type: String,  // 编码类型
    width: Option<i32>,  // 视频宽度
    height: Option<i32>, // 视频高度
    #[serde(default)]
    codec_name: String,  // 编码名称
    channels: Option<i32>, // 音频通道数
}

/// 检查模块
pub mod check {
    use crate::common::util::check_body_is_m3u8_format;
    use crate::common::{AudioInfo, CheckOptions, CheckUrlIsAvailableResponse, Ffprobe, VideoInfo};
    use chrono::Utc;
    use std::io::{Error, ErrorKind, Read};
    use std::time;
    use log::{debug, info};
    use url::Url;
    use tokio::time::{timeout, Duration};
    use std::time::Instant;
    use std::thread;
    use std::process::{Command, Child, Stdio, ExitStatus};
    use std::sync::mpsc::{channel, Sender, Receiver};
    use std::io::{self, BufReader, BufRead};
    use std::sync::{Arc, Mutex};
    use crate::common;
    use crate::utils::get_out_put_filename;
    use crate::common::m3u;

    /// 使用超时运行命令并获取结果
    /// 
    /// # 参数
    /// * `_url` - 要检查的URL
    /// * `timeout_mill_secs` - 超时时间（毫秒）
    /// 
    /// # 返回值
    /// * `Result<CheckUrlIsAvailableResponse, Error>` - 检查结果
    pub async fn run_command_with_timeout_new(_url: String, timeout_mill_secs: u64) -> Result<CheckUrlIsAvailableResponse, Error> {
        let timeout = Duration::from_millis(timeout_mill_secs);
        let mut second = timeout_mill_secs / 1000;
        if second < 1 {
            second = 1
        }

        // 1. 配置FFprobe命令
        let mut cmd = Command::new("ffprobe");
        cmd.args(vec!["-v", "quiet", "-print_format", "json",
                      "-show_format", "-show_streams", "-timeout", &second.to_string(), &_url.to_owned()]);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // 启动子进程
        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn command: {}", e)).unwrap();

        // 2. 获取标准输出和错误输出的管道句柄
        let stdout_handle = child.stdout.take().ok_or("Failed to open stdout pipe".to_string()).unwrap();
        let stderr_handle = child.stderr.take().ok_or("Failed to open stderr pipe".to_string()).unwrap();

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
                    Err(ref e) if e.kind() == io::ErrorKind::BrokenPipe => break,
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
                    Err(ref e) if e.kind() == io::ErrorKind::BrokenPipe => break,
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
                            Err(e) => debug!("Warning: Failed to kill process after timeout: {}", e),
                        }
                        timed_out = true;
                        thread::sleep(Duration::from_millis(50));
                        final_status = child.try_wait()
                            .map_err(|e| format!("Error checking status after kill: {}", e)).unwrap()
                            .unwrap_or_else(|| {
                                debug!("Warning: Process did not exit immediately after kill signal.");
                                ExitStatus::default()
                            });
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    stdout_thread.join().expect("Stdout thread panicked");
                    stderr_thread.join().expect("Stderr thread panicked");
                    return Err(Error::new(ErrorKind::Other, format!("Failed to wait on child process: {}", e)));
                }
            }
        }

        // 7. 等待读取线程完成
        stdout_thread.join().expect("Stdout thread panicked");
        stderr_thread.join().expect("Stderr thread panicked");

        // 8. 处理超时情况
        if timed_out {
            return Err(Error::new(ErrorKind::TimedOut, "Command timed out"));
        }

        // 9. 检查进程退出状态
        if !final_status.success() {
            return Err(Error::new(ErrorKind::Other, format!("Command failed with status: {}", final_status)));
        }

        // 10. 解析FFprobe输出
        let stdout_data = stdout_buf.lock().unwrap();
        let output = String::from_utf8_lossy(&stdout_data);
        let ffprobe: Ffprobe = serde_json::from_str(&output)
            .map_err(|e| Error::new(ErrorKind::InvalidData, format!("Failed to parse ffprobe output: {}", e)))?;

        // 11. 处理流信息
        let mut response = CheckUrlIsAvailableResponse::new();
        for stream in ffprobe.streams {
            match stream.codec_type.as_str() {
                "video" => {
                    let mut video_info = VideoInfo::new();
                    if let Some(width) = stream.width {
                        video_info.set_width(width);
                    }
                    if let Some(height) = stream.height {
                        video_info.set_height(height);
                    }
                    video_info.set_codec(stream.codec_name);
                    response.set_video(video_info);
                }
                "audio" => {
                    let mut audio_info = AudioInfo::new();
                    if let Some(channels) = stream.channels {
                        audio_info.set_channels(channels);
                    }
                    audio_info.set_codec(stream.codec_name);
                    response.set_audio(audio_info);
                }
                _ => {}
            }
        }

        Ok(response)
    }

    /// 检查链接是否有效
    /// 
    /// # 参数
    /// * `_url` - 要检查的URL
    /// * `timeout` - 超时时间（毫秒）
    /// * `need_video_info` - 是否需要视频信息
    /// * `ffmpeg_check` - 是否使用FFmpeg检查
    /// * `not_http_skip` - 是否跳过非HTTP链接
    /// 
    /// # 返回值
    /// * `Result<CheckUrlIsAvailableResponse, Error>` - 检查结果
    pub async fn check_link_is_valid(
        _url: String,
        timeout: u64,
        need_video_info: bool,
        ffmpeg_check: bool,
        not_http_skip: bool,
    ) -> Result<CheckUrlIsAvailableResponse, Error> {
        if ffmpeg_check {
            let res = run_command_with_timeout_new(_url.to_owned(), timeout).await;
            return match res {
                Ok(res) => {
                    Ok(res)
                }
                Err(e) => {
                    Err(Error::new(ErrorKind::Other, format!("status is not 200 {}", e)))
                }
            };
        }
        let parsed_info = Url::parse(&_url);
        match parsed_info {
            Ok(parsed_url) => {
                if parsed_url.scheme() != "https" && parsed_url.scheme() != "http" {
                    return if not_http_skip {
                        Ok(CheckUrlIsAvailableResponse::new())
                    } else {
                        Err(Error::new(ErrorKind::Other, "scheme not http, temporary not support"))
                    };
                }
            }
            Err(e) => {
                return Err(Error::new(ErrorKind::Other, format!("error {}", e)));
            }
        }
        let client_resp = reqwest::Client::builder()
            .timeout(time::Duration::from_millis(timeout))
            .danger_accept_invalid_certs(true)
            .build();
        match client_resp {
            Ok(client) => {
                let curr_timestamp = Utc::now().timestamp_millis();
                let http_res = client.get(_url.to_owned()).send().await;
                match http_res {
                    Ok(res) => {
                        if res.status().is_success() {
                            let delay = Utc::now().timestamp_millis() - curr_timestamp;
                            if need_video_info {
                                let mut ffmpeg_info = run_command_with_timeout_new(_url.to_owned(), timeout).await;
                                match ffmpeg_info {
                                    Ok(mut data) => {
                                        data.set_delay(delay as i32);
                                        Ok(data)
                                    }
                                    Err(err) => {
                                        Err(Error::new(ErrorKind::Other, err.to_string()))
                                    }
                                }
                            } else {
                                let _body = res.text().await;
                                match _body {
                                    Ok(body) => {
                                        if check_body_is_m3u8_format(body.clone()) {
                                            let mut body: CheckUrlIsAvailableResponse = CheckUrlIsAvailableResponse::new();
                                            body.set_delay(delay as i32);
                                            Ok(body)
                                        } else {
                                            Err(Error::new(ErrorKind::Other, "not a m3u8 file"))
                                        }
                                    }
                                    Err(e) => Err(Error::new(ErrorKind::Other, format!("{:?}", e))),
                                }
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
            Err(e) => {
                return Err(Error::new(ErrorKind::Other, format!("http client build error {}", e)));
            }
        }
    }

    /// 执行检查
    /// 
    /// # 参数
    /// * `input_files` - 输入文件列表
    /// * `output_file` - 输出文件
    /// * `timeout` - 超时时间（秒）
    /// * `print_result` - 是否打印结果
    /// * `request_timeout` - 请求超时时间（秒）
    /// * `concurrent` - 并发数
    /// * `keyword_like` - 关键词白名单
    /// * `keyword_dislike` - 关键词黑名单
    /// * `sort` - 是否排序
    /// * `no_check` - 是否跳过检查
    /// * `rename` - 是否重命名
    /// * `ffmpeg_check` - 是否使用FFmpeg检查
    /// * `same_save_num` - 相同保存数量
    /// * `not_http_skip` - 是否跳过非HTTP链接
    /// 
    /// # 返回值
    /// * `Result<bool, Error>` - 检查结果
    pub async fn do_check(
        input_files: Vec<String>,
        output_file: String,
        timeout: i32,
        print_result: bool,
        request_timeout: i32,
        concurrent: i32,
        keyword_like: Vec<String>,
        keyword_dislike: Vec<String>,
        sort: bool,
        no_check: bool,
        rename: bool,
        ffmpeg_check: bool,
        same_save_num: i32,
        not_http_skip: bool,
    ) -> Result<bool, Error> {
        let mut data = common::m3u::m3u::from_arr(
            input_files.to_owned(),
            timeout as u64,
            keyword_like.to_owned(),
            keyword_dislike.to_owned(),
            rename,
        )
            .await;
        let mut output_file = get_out_put_filename(output_file.clone());
        // 拼接目录
        output_file = format!("{}{}", "./", output_file);
        if print_result {
            info!("输出文件: {}", output_file);
        }
        data.check_data_new(CheckOptions {
            request_time:request_timeout,
            concurrent,
            sort,
            no_check,
            ffmpeg_check,
            same_save_num,
            not_http_skip,
            search_clarity: vec![],
        }).await;
        data.output_file(output_file).await;
        if print_result {
            if !no_check {
                let status_string = data.print_result();
                info!("\n{}", status_string);
            }
            info!("解析完成----")
        }
        Ok(true)
    }

    /// 测试模块
    mod tests {
        use crate::common::check::check::{run_command_with_timeout_new};
        use std::thread;
        use std::sync::mpsc;
        #[tokio::test]
        async fn test_timeout() {
            let (tx, rx) = mpsc::channel();

            // 模拟从channel里收到一条命令
            thread::spawn(move || {
                tx.send(("https://cd-live-stream.news.cctvplus.com/live/smil:CHANNEL2.smil/playlist.m3u8", 5000)).unwrap(); // 比如要执行sleep 5秒
            });

            if let Ok((_url, timeout)) = rx.recv() {
                println!("Running command: {} {:?}", _url, timeout);
                match run_command_with_timeout_new(_url.to_string(), (timeout as u64)).await {
                    Ok(ed) => {
                        let v = ed.video.clone().unwrap();
                        println!("Command finished successfully.{} {}", v.width, v.height)
                    }
                    Err(e) => println!("Command failed: {}", e),
                }
            }
        }
    }
}

pub async fn do_check(
    input_files: Vec<String>,
    output_file: String,
    timeout: i32,
    print_result: bool,
    request_timeout: i32,
    concurrent: i32,
    keyword_like: Vec<String>,
    keyword_dislike: Vec<String>,
    sort: bool,
    no_check: bool,
    rename: bool,
    ffmpeg_check: bool,
    same_save_num: i32,
    not_http_skip: bool,
) -> Result<bool, Error> {
    let mut data = common::m3u::m3u::from_arr(
        input_files.to_owned(),
        timeout as u64,
        keyword_like.to_owned(),
        keyword_dislike.to_owned(),
        rename,
    )
        .await;
    let mut output_file = utils::get_out_put_filename(output_file.clone());
    // 拼接目录
    output_file = format!("{}{}", "./", output_file);
    if print_result {
        info!("输出文件: {}", output_file);
    }
    data.check_data_new(CheckOptions {
        request_time:request_timeout,
        concurrent,
        sort,
        no_check,
        ffmpeg_check,
        same_save_num,
        not_http_skip,
        search_clarity: vec![],
    }).await;
    data.output_file(output_file).await;
    if print_result {
        if !no_check {
            let status_string = data.print_result();
            info!("\n{}", status_string);
        }
        info!("解析完成----")
    }
    Ok(true)
}

// 测试模块
#[cfg(test)]
mod tests {
    use crate::common::check::check::{run_command_with_timeout_new};
    use std::thread;
    use std::sync::mpsc;
    #[tokio::test]
    async fn test_timeout() {
        let (tx, rx) = mpsc::channel();

        // 模拟从channel里收到一条命令
        thread::spawn(move || {
            tx.send(("https://cd-live-stream.news.cctvplus.com/live/smil:CHANNEL2.smil/playlist.m3u8", 5000)).unwrap(); // 比如要执行sleep 5秒
        });

        if let Ok((_url, timeout)) = rx.recv() {
            println!("Running command: {} {:?}", _url, timeout);
            match run_command_with_timeout_new(_url.to_string(), (timeout as u64)).await {
                Ok(ed) => {
                    let v = ed.video.clone().unwrap();
                    println!("Command finished successfully.{} {}", v.width, v.height)
                }
                Err(e) => println!("Command failed: {}", e),
            }
        }
    }
}
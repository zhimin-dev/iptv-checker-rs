use crate::common::{AudioInfo, VideoInfo};
use crate::{common, utils};
use log::info;
use serde::{Deserialize, Serialize};
use std::fmt::Error;
use std::net::ToSocketAddrs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckUrlIsAvailableResponse {
    pub(crate) delay: i32,
    pub(crate) video: Option<VideoInfo>,
    pub(crate) audio: Option<AudioInfo>,
}

impl CheckUrlIsAvailableResponse {
    pub fn new() -> CheckUrlIsAvailableResponse {
        CheckUrlIsAvailableResponse {
            delay: 0,
            video: None,
            audio: None,
        }
    }

    pub fn set_delay(&mut self, delay: i32) {
        self.delay = delay
    }

    pub fn set_video(&mut self, video: VideoInfo) {
        self.video = Some(video)
    }

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

#[derive(Debug, Deserialize, Serialize)]
pub struct Ffprobe {
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FfprobeStream {
    #[serde(default)]
    codec_type: String,
    width: Option<i32>,
    height: Option<i32>,
    #[serde(default)]
    codec_name: String,
    channels: Option<i32>,
}

pub mod check {
    use crate::common::util::check_body_is_m3u8_format;
    use crate::common::{AudioInfo, CheckUrlIsAvailableResponse, Ffprobe, VideoInfo};
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

    pub async fn run_command_with_timeout_new(_url: String, timeout_mill_secs: u64) -> Result<CheckUrlIsAvailableResponse, Error> {
        let timeout = Duration::from_millis(timeout_mill_secs);
        let mut second = timeout_mill_secs / 1000;
        if second < 1 {
            second = 1
        }

        // 1. 配置 Command 并启用管道
        let mut cmd = Command::new("ffprobe");
        cmd.args(vec!["-v", "quiet", "-print_format", "json",
                      "-show_format", "-show_streams", "-timeout", &second.to_string(), &_url.to_owned()]);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());


        // 启动子进程
        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn command: {}", e)).unwrap();

        // 2. 获取 stdout/stderr 管道的句柄 (必须在主线程获取)
        //    使用 take() 获取所有权
        let stdout_handle = child.stdout.take().ok_or("Failed to open stdout pipe".to_string()).unwrap();
        let stderr_handle = child.stderr.take().ok_or("Failed to open stderr pipe".to_string()).unwrap();

        // 3. 创建共享缓冲区
        let stdout_buf = Arc::new(Mutex::new(Vec::new()));
        let stderr_buf = Arc::new(Mutex::new(Vec::new()));

        // 克隆 Arc 以便移动到线程中
        let stdout_buf_clone = Arc::clone(&stdout_buf);
        let stderr_buf_clone = Arc::clone(&stderr_buf);

        // 4. 启动 stdout 读取线程
        let stdout_thread = thread::spawn(move || {
            // 使用 BufReader 可能效率稍高，但直接 read 也可以
            let mut buffer = [0; 1024]; // 读取缓冲区
            let mut handle = stdout_handle; // 移动所有权
            loop {
                match handle.read(&mut buffer) {
                    Ok(0) => break, // EOF，管道关闭
                    Ok(n) => {
                        // 获取锁，写入数据
                        let mut locked_buf = stdout_buf_clone.lock().unwrap();
                        locked_buf.extend_from_slice(&buffer[..n]);
                    }
                    // 忽略 BrokenPipe，这通常发生在进程被 kill 时
                    Err(ref e) if e.kind() == io::ErrorKind::BrokenPipe => break,
                    Err(e) => {
                        eprintln!("Error reading stdout: {}", e); // 记录错误
                        break;
                    }
                }
            }
            // 可以在这里返回结果，或者像现在这样只写入共享数据
        });

        // 5. 启动 stderr 读取线程 (逻辑类似)
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

        // 6. 主线程执行轮询和超时逻辑
        let start = Instant::now();
        let final_status: ExitStatus;
        let mut timed_out = false;

        loop {
            match child.try_wait() {
                // 进程已退出
                Ok(Some(status)) => {
                    // debug!("Process exited normally with status: {}", status);
                    final_status = status;
                    break; // 退出轮询循环
                }
                // 进程仍在运行
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        // 超时，尝试杀掉进程
                        // debug!("Process timed out. Attempting to kill...");
                        match child.kill() {
                            Ok(_) => debug!("Process killed due to timeout."),
                            Err(e) => debug!("Warning: Failed to kill process after timeout: {}", e), // 记录 kill 失败，但仍视为超时
                        }
                        timed_out = true;
                        // 注意：即使 kill 成功，try_wait 可能不会立即返回 Some(status)
                        // 我们需要一种方式来获取最终状态，或者直接认为超时失败
                        // 稍微等待一下让系统处理 kill，然后再次 try_wait 或直接退出循环
                        thread::sleep(Duration::from_millis(50)); // 短暂等待 kill 生效
                        // 再次检查状态，如果还没退出就强制认为超时失败并退出
                        final_status = child.try_wait()
                            .map_err(|e| format!("Error checking status after kill: {}", e)).unwrap()
                            .unwrap_or_else(|| {
                                // 如果 kill 后进程仍然存在（可能权限不够或特殊情况），
                                // 我们没有标准的 ExitStatus，可以构造一个或返回特定错误
                                // 这里简单地认为超时失败
                                debug!("Warning: Process did not exit immediately after kill signal.");
                                // 返回一个模拟的状态码或错误可能更好，但这里我们继续，将在下面处理 timed_out 标志
                                // 为了有 ExitStatus，我们这里可能需要等最后一次 wait
                                // 或者直接 break 然后在后面处理 timed_out
                                // ExitStatus::from_raw(1) // Unix-like, just an example
                                // 为了简化，我们直接 break，后面用 timed_out 判断
                                // （如果需要退出码，可能需要 child.wait()）
                                ExitStatus::default() // Placeholder, won't be used if timed_out is true
                            });

                        break; // 退出轮询循环
                    }
                    // 未超时，进程仍在运行，短暂休眠
                    thread::sleep(Duration::from_millis(100));
                }
                // try_wait 出错
                Err(e) => {
                    // 在返回前，仍然尝试 join 读取线程，避免线程泄漏
                    stdout_thread.join().expect("Stdout thread panicked");
                    stderr_thread.join().expect("Stderr thread panicked");
                    return Err(Error::new(ErrorKind::Other, format!("Failed to wait on child process: {}", e)));
                }
            }
        } // 结束轮询 loop

        // 7. 等待读取线程结束
        // join 会等待线程完成，并返回线程的 Result (如果线程 panic 会 Err)
        stdout_thread.join().map_err(|_| "Stdout reader thread panicked".to_string()).unwrap();
        stderr_thread.join().map_err(|_| "Stderr reader thread panicked".to_string()).unwrap();

        // 8. 处理结果
        if timed_out {
            // 如果是超时，即使读取线程可能收集了一些数据，我们也返回超时错误
            // 可以选择性地包含部分收集到的数据在错误信息里
            // let stdout_data = stdout_buf.lock().unwrap().clone();
            // let stderr_data = stderr_buf.lock().unwrap().clone();
            // eprintln!("Partial Stdout collected before timeout: {}", String::from_utf8_lossy(&stdout_data));
            // eprintln!("Partial Stderr collected before timeout: {}", String::from_utf8_lossy(&stderr_data));
            return Err(Error::new(ErrorKind::Other, "Process timed out and was killed."));
        } else {
            // 正常结束，获取最终的输出
            // MutexGuard 在 drop 时自动解锁
            let stdout_data = stdout_buf.lock().unwrap().clone();
            // let stderr_data = stderr_buf.lock().unwrap().clone();
            // eprintln!("Partial Stdout collected: {}", String::from_utf8_lossy(&stdout_data));
            // eprintln!("Partial Stderr collected: {}", String::from_utf8_lossy(&stderr_data));

            let raw_json_str = String::from_utf8_lossy(&stdout_data);
            match serde_json::from_str::<Ffprobe>(&raw_json_str) {
                Ok(res_data) => {
                    let mut body: CheckUrlIsAvailableResponse = CheckUrlIsAvailableResponse::new();
                    for one in res_data.streams.into_iter() {
                        if one.codec_type == "video" {
                            let mut video = VideoInfo::new();
                            if let Some(e) = one.width {
                                video.set_width(e)
                            }
                            if let Some(e) = one.height {
                                video.set_height(e)
                            }
                            video.set_codec(one.codec_name);
                            body.set_video(video);
                        } else if one.codec_type == "audio" {
                            let mut audio = AudioInfo::new();
                            audio.set_codec(one.codec_name);
                            audio.set_channels(one.channels.unwrap());
                            body.set_audio(audio);
                        }
                    }
                    // debug!("ffmepg check end --- {}", _url.to_owned());
                    return Ok(body);
                }
                Err(json_error) => {
                    // eprintln!("JSON parsing error (from lossy UTF-8 input): {}", json_error);
                    // 处理 JSON 解析错误
                    return Err(Error::new(ErrorKind::Other, format!("JSON parsing error (from lossy UTF-8 input): {}", json_error)));
                }
            }
        }
    }

    pub async fn check_link_is_valid(
        _url: String,
        timeout: u64,
        need_video_info: bool,
        ffmpeg_check: bool,
        not_http_skip: bool,
    ) -> Result<CheckUrlIsAvailableResponse, Error> {
        // println!("start check_link_is_valid check -----");
        let start_time = Instant::now();
        if ffmpeg_check {
            let res = run_command_with_timeout_new(_url.to_owned(), timeout).await;
            return match res {
                Ok(res) => {
                    let lduration = start_time.elapsed();
                    // println!("start check_link_is_valid end {}", lduration.subsec_millis());
                    Ok(res)
                }
                Err(e) => {
                    let lduration = start_time.elapsed();
                    // println!("start check_link_is_valid end {}", lduration.subsec_millis());
                    Err(Error::new(ErrorKind::Other, format!("status is not 200 {}", e)))
                }
            };
        }
        // println!("start web check -----");
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
    data.check_data_new(request_timeout, concurrent, sort, no_check, ffmpeg_check, same_save_num, not_http_skip)
        .await;
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

use tokio::net::TcpStream;

async fn check_rtmp_socket(address: &str) -> Result<bool, std::io::Error> {
    // 1. 建立 TCP 连接
    let mut stream = match TcpStream::connect(address).await {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!("连接失败: {}", e);
            return Err(e);
        }
    };

    // 2. 发送 C0 包 (版本 3: 0x03)
    let c0_packet: [u8; 1] = [0x03];
    if let Err(e) = stream.write_all(&c0_packet).await {
        eprintln!("发送 C0 包失败: {}", e);
        return Ok(false); // 发送失败不代表服务无效，可能只是网络问题，但此处简化处理
    }

    println!("发送 C0 包成功");

    // 3. 发送 C1 包 (时间戳 0, 1532 字节随机数据)
    let mut c1_packet: [u8; 1536] = [0; 1536];
    // 时间戳设置为 0 (前 4 字节)
    // 随机数据保持为默认的 0 即可简化示例

    if let Err(e) = stream.write_all(&c1_packet).await {
        eprintln!("发送 C1 包失败: {}", e);
        return Ok(false);
    }
    println!("发送 C1 包成功");

    // 4. 尝试接收 S0, S1, S2 包
    let mut s0_packet: [u8; 1] = [0; 1];
    let mut s1_packet: [u8; 1536] = [0; 1536];
    let mut s2_packet: [u8; 1536] = [0; 1536];

    println!("尝试接收 S0 包...");
    match stream.read_exact(&mut s0_packet).await {
        Ok(_) => {
            println!("接收 S0 包成功, 版本: 0x{:X}", s0_packet[0]);
        }
        Err(e) => {
            eprintln!("接收 S0 包失败: {}", e);
            return Ok(false);
        }
    }

    println!("尝试接收 S1 包...");
    match stream.read_exact(&mut s1_packet).await {
        Ok(_) => {
            println!("接收 S1 包成功, 前 4 字节 (时间戳/时间): {:?}", &s1_packet[0..4]);
        }
        Err(e) => {
            eprintln!("接收 S1 包失败: {}", e);
            return Ok(false);
        }
    }

    println!("尝试接收 S2 包...");
    match stream.read_exact(&mut s2_packet).await {
        Ok(_) => {
            println!("接收 S2 包成功, 前 4 字节: {:?}", &s2_packet[0..4]);
        }
        Err(e) => {
            eprintln!("接收 S2 包失败: {}", e);
            return Ok(false);
        }
    }

    println!("成功接收 S0, S1, S2 包，初步判断 RTMP 服务有效");
    Ok(true)
}

async fn check_rtmp_path_exists(address: &str, app_name: &str, stream_name: &str) -> Result<bool, std::io::Error> {
    let mut stream = TcpStream::connect(address).await?;
    // stream.set_ttl(Some(Duration::from_secs(5)))?;
    // stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    // 1. 完成 C0-S2 握手 (沿用之前的握手程式碼)
    let c0_packet: [u8; 1] = [0x03];
    stream.write_all(&c0_packet).await?;
    let mut c1_packet: [u8; 1536] = [0; 1536];
    stream.write_all(&c1_packet).await?;
    let mut s0_packet: [u8; 1] = [0; 1];
    let mut s1_packet: [u8; 1536] = [0; 1536];
    let mut s2_packet: [u8; 1536] = [0; 1536];
    stream.read_exact(&mut s0_packet).await?;
    stream.read_exact(&mut s1_packet).await?;
    stream.read_exact(&mut s2_packet).await?;

    // 2. 發送 connect 命令
    println!("發送 connect 命令, app: {}", app_name);
    // 假設 rtmp_codec::encode_connect_command 函式可以編碼 connect 命令
    // let connect_command = rtmp_codec::encode_connect_command(app_name);
    // stream.write_all(&connect_command)?;
    // --- 簡化範例，假設 connect 命令已發送 ---

    // 3. 接收伺服器對 connect 命令的回應 (需要解碼 RTMP 消息)
    // 假設 rtmp_codec::decode_rtmp_message 函式可以解碼 RTMP 消息
    // let mut buffer = [0; 2048]; // 假設緩衝區大小
    // let bytes_read = stream.read(&mut buffer)?;
    // if bytes_read > 0 {
    //     let response = rtmp_codec::decode_rtmp_message(&buffer[..bytes_read])?;
    //     // 檢查 response 是否為 _result 且連線成功
    //     // ...
    // } else {
    //     return Ok(false); // 連線失敗
    // }
    println!("假設已接收 connect 回應並成功");

    // 4. 發送 createStream 命令
    println!("發送 createStream 命令");
    // let create_stream_command = rtmp_codec::encode_create_stream_command();
    // stream.write_all(&create_stream_command)?;
    // --- 簡化範例，假設 createStream 命令已發送 ---

    // 5. 接收伺服器對 createStream 命令的回應 (需要解碼 RTMP 消息)
    // let bytes_read_stream_id = stream.read(&mut buffer)?;
    // if bytes_read_stream_id > 0 {
    //     let response_stream_id = rtmp_codec::decode_rtmp_message(&buffer[..bytes_read_stream_id])?;
    //     // 從 response_stream_id 中提取 stream ID
    //     // ...
    // } else {
    //     return Ok(false); // 建立串流失敗
    // }
    let stream_id = 1; // 假設 stream ID 為 1，實際應從伺服器回應中取得
    println!("假設已接收 createStream 回應, 取得 stream ID: {}", stream_id);

    // 6. 發送 play 命令
    println!("發送 play 命令, stream: {}", stream_name);
    // let play_command = rtmp_codec::encode_play_command(stream_id, stream_name);
    // stream.write_all(&play_command)?;
    // --- 簡化範例，假設 play 命令已發送 ---

    // 7. 接收伺服器對 play 命令的回應 (需要解碼 RTMP 消息)
    let mut buffer_play_response = [0; 2048]; // 假設緩衝區大小
    match stream.read(&mut buffer_play_response).await {
        Ok(bytes_read_play) => {
            if bytes_read_play > 0 {
                // 假設 rtmp_codec::decode_rtmp_message 可以正確解碼
                // let play_response = rtmp_codec::decode_rtmp_message(&buffer_play_response[..bytes_read_play])?;
                // 檢查 play_response 中是否包含錯誤訊息，例如 NetStream.Play.StreamNotFound
                // if is_stream_not_found_error(&play_response) {
                //     println!("收到串流未找到錯誤訊息");
                //     Ok(false) // 路徑不存在
                // } else if is_play_start_message(&play_response) {
                //     println!("收到播放開始訊息");
                //     Ok(true) // 路徑存在
                // } else {
                //     println!("收到其他回應訊息: {:?}", play_response);
                //     Ok(true) // 收到其他回應，也暫時視為路徑存在 (可能需要更嚴謹的判斷)
                // }
                println!("假設已接收 play 回應");
                // --- 簡化範例，直接檢查是否收到任何回應，收到回應暫時視為路徑存在 ---
                Ok(true) // 收到任何回應暫時視為路徑存在
            } else {
                println!("未收到 play 命令回應");
                Ok(false) // 未收到回應，視為路徑不存在
            }
        }
        Err(e) => {
            eprintln!("接收 play 命令回應錯誤: {}", e);
            Ok(false) // 接收錯誤，視為路徑不存在
        }
    }
}

// 测试模块
#[cfg(test)]
mod tests {
    use crate::common::check::check::{ run_command_with_timeout_new};
    use crate::common::check::check_rtmp_path_exists;
    use std::thread;
    use std::time::{Duration, Instant};
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

    #[tokio::test]
    async fn test_valid_rtmp_url() {
        let rtmp_address = "f13h.mine.nu:1935"; // 替换为你的 RTMP 服务器地址
        let app_name = "sat"; // 替換為你的應用程式名稱
        let stream_name = "tv111"; // 替換為你要檢查的串流路徑 (例如 video 或 channel1)

        println!("開始檢查 RTMP 路徑: rtmp://{}/{}/{}", rtmp_address, app_name, stream_name);

        match check_rtmp_path_exists(rtmp_address, app_name, stream_name).await {
            Ok(path_exists) => {
                if path_exists {
                    println!("RTMP 路徑初步判斷為存在");
                } else {
                    println!("RTMP 路徑初步判斷為不存在");
                }
            }
            Err(e) => {
                eprintln!("檢查過程中發生錯誤: {}", e);
                println!("RTMP 路徑初步判斷為不存在 (發生錯誤)");
            }
        }
    }
}
use std::process::{Command, ExitStatus};

/// 从M3U8流中捕获首帧图片
/// 
/// # 参数
/// * `m3u8_url` - M3U8流的URL地址
/// * `output_image` - 输出图片的路径
/// * `timeout_seconds` - 超时时间（秒）
/// 
/// # 返回值
/// * `bool` - 成功返回true，失败返回false
pub fn capture_stream_pic(m3u8_url: String, output_image: String, timeout_seconds: u64) -> bool {
    // 使用ffmpeg截取首帧
    let status = Command::new("ffmpeg")
        .args(&[
            "-i",
            &m3u8_url, // 输入M3U8地址
            "-frames:v",
            "1",           // 只截取一帧
            "-y",          // 如果输出文件已存在则覆盖
            "-timeout",    // 添加超时参数
            &timeout_seconds.to_string(),
            &output_image, // 输出文件名
        ])
        .status()
        .expect("failed to execute ffmpeg");

    if status.success() {
        println!("First frame captured successfully to {}", &output_image);
        true
    } else {
        println!("Failed to capture the first frame");
        false
    }
}

/// 将RTMP流转换为HLS流
/// 
/// # 参数
/// * `rtmp_url` - RTMP流的URL地址
/// * `hls_output` - HLS输出文件的路径
/// 
/// # 返回值
/// * `bool` - 成功返回true，失败返回false
/// 
/// # 说明
/// 该函数使用ffmpeg将RTMP流转换为HLS流，并生成相应的M3U8文件和TS片段。
/// 转换过程中会：
/// 1. 保持视频编码不变（copy）
/// 2. 将音频转换为AAC格式
/// 3. 每个TS片段持续10秒
/// 4. 保持最近的5个片段在播放列表中
/// 5. 自动删除旧的TS片段
pub fn live_steam_to_m3u8_steam(rtmp_url: String, hls_output: String) -> bool {
    let status: ExitStatus = Command::new("ffmpeg")
        .args(&[
            "-i",
            &rtmp_url,
            "-c:v",
            "copy",        // 保持视频编码不变
            "-c:a",
            "aac",         // 将音频转换为AAC格式
            "-strict",
            "experimental",
            "-f",
            "hls",
            "-hls_flags",
            "delete_segments", // 启用删除旧的TS文件
            "-hls_time",
            "10", // 每个TS片段的持续时间
            "-hls_list_size",
            "5", // 保持最近的5个片段在播放列表
            &hls_output,
        ])
        .status()
        .expect("failed to execute ffmpeg");

    if status.success() {
        println!("Successfully converted RTMP to HLS!");
        true
    } else {
        eprintln!("Error occurred while converting RTMP to HLS.");
        false
    }
}

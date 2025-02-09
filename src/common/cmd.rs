use std::process::{Command, ExitStatus};

// capture_stream_pic("http://127.0.0.1:8089/static/input/live/live.m3u8".to_string(), "./static/input/222.jpeg".to_string());
pub fn capture_stream_pic(m3u8_url: String, output_image: String) -> bool {
    // 使用 ffmpeg 截取首帧
    let status = Command::new("ffmpeg")
        .args(&[
            "-i",
            &m3u8_url, // 输入 M3U8 地址
            "-frames:v",
            "1",           // 只截取一帧
            "-y",          // 如果输出文件已存在则覆盖
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

pub fn live_steam_to_m3u8_steam(rtmp_url: String, hls_output: String) -> bool {
    // 启动 RTMP 服务器（假设你用其他工具启动，比如 NGINX）
    // 然后使用 FFmpeg 处理 RTMP 流转为 HLS

    let status: ExitStatus = Command::new("ffmpeg")
        .args(&[
            "-i",
            &rtmp_url,
            "-c:v",
            "copy",
            "-c:a",
            "aac",
            "-strict",
            "experimental",
            "-f",
            "hls",
            "-hls_flags",
            "delete_segments", // 启用删除旧的TS文件
            "-hls_time",
            "10", // 每个 TS 片段的持续时间
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

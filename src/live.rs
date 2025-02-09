use crate::common::cmd::live_steam_to_m3u8_steam;
use std::io::Error;

pub fn do_ob(rtmp_url: String) -> Result<String, Error> {
    // 启动 RTMP 服务器（假设你用其他工具启动，比如 NGINX）
    // 然后使用 FFmpeg 处理 RTMP 流转为 HLS
    // HLS 输出路径
    let hls_output = "./static/input/live/live.m3u8";
    let res = live_steam_to_m3u8_steam(rtmp_url, hls_output.to_string());
    if res {
        Ok(hls_output.replace("./", "{{your_host}}"))
    } else {
        Ok("".to_string())
    }
}

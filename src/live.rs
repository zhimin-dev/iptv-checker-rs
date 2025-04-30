// 导入所需的模块
use crate::common::cmd::live_steam_to_m3u8_steam;
use std::io::Error;

/// 将RTMP直播流转码为HLS流
/// 
/// # 参数
/// * `rtmp_url` - RTMP直播流地址
/// 
/// # 返回值
/// * `Result<String, Error>` - 成功返回HLS流地址，失败返回错误
/// 
/// # 说明
/// 该函数将RTMP直播流转换为HLS流，并返回可访问的HLS流地址。
/// 转换后的HLS流文件将保存在 `./static/input/live/live.m3u8` 路径下。
pub fn do_ob(rtmp_url: String) -> Result<String, Error> {
    // HLS输出文件路径
    let hls_output = "./static/input/live/live.m3u8";
    
    // 调用转码函数将RTMP流转为HLS流
    let res = live_steam_to_m3u8_steam(rtmp_url, hls_output.to_string());
    
    // 根据转码结果返回相应的URL
    if res {
        // 成功时返回替换了主机地址的HLS流URL
        Ok(hls_output.replace("./", "{{your_host}}"))
    } else {
        // 失败时返回空字符串
        Ok("".to_string())
    }
}

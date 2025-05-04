use crate::common::QualityType::{
    Quality1080P, Quality240P, Quality2K, Quality360P, Quality480P, Quality4K,
    Quality720P, Quality8K, QualityUnknown,
};
use crate::common::{M3uExt, M3uExtend, M3uObject, M3uObjectList, QualityType};
use crate::utils::translator_t2s;
use reqwest::Error;
use url::Url;

/// 获取URL的内容
///
/// # 参数
/// * `_url` - 要获取内容的URL
/// * `timeout` - 超时时间（毫秒）
///
/// # 返回值
/// * `Result<String, Error>` - 成功返回URL内容，失败返回错误
pub async fn get_url_body(_url: String, timeout: u64) -> Result<String, Error> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(timeout))
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    client.get(_url.to_owned()).send().await?.text().await
}

/// 检查内容是否为M3U8格式
///
/// # 参数
/// * `_body` - 要检查的内容
///
/// # 返回值
/// * `bool` - 如果是M3U8格式返回true，否则返回false
pub fn check_body_is_m3u8_format(_body: String) -> bool {
    _body.starts_with("#EXTM3U")
}

/// 检查字符串是否为IPv6格式
///
/// # 参数
/// * `s` - 要检查的字符串
///
/// # 返回值
/// * `bool` - 如果是IPv6格式返回true，否则返回false
// pub fn match_ipv6_format(s: &str) -> bool {
//     // 检查是否包含IPv6地址的典型特征：冒号
//     if !s.contains(':') {
//         return false;
//     }
//
//     // 如果包含方括号，则去掉方括号
//     let s = if s.starts_with('[') && s.ends_with(']') {
//         &s[1..s.len() - 1]
//     } else {
//         s
//     };
//
//     // 解析URL并检查主机部分是否为IPv6地址
//     let parsed_url = Url::parse(s).unwrap();
//     let host = parsed_url.host_str().unwrap();
//     host.parse::<std::net::Ipv6Addr>().is_ok()
// }

/// 检查URL的主机地址类型
///
/// # 参数
/// * `url_str` - 要检查的URL
///
/// # 返回值
/// * `io::Result<Option<IpAddress>>` - 成功返回IP地址类型，失败返回错误
// pub fn check_url_host_ip_type(url_str: &str) -> io::Result<Option<IpAddress>> {
//     let parsed_url = Url::parse(url_str).unwrap();
//     let host = parsed_url.host_str().unwrap();
//     if let Ok(ip) = host.parse::<IpAddr>() {
//         match ip {
//             IpAddr::V4(_) => Ok(Some(IpAddress::Ipv4Addr)),
//             IpAddr::V6(_) => Ok(Some(IpAddress::Ipv6Addr)),
//         }
//     } else {
//         Ok(None)
//     }
// }

/// 解析标准M3U格式的字符串
///
/// # 参数
/// * `_body` - M3U格式的字符串
///
/// # 返回值
/// * `M3uObjectList` - 解析后的M3U对象列表
pub fn parse_normal_str(_body: String) -> M3uObjectList {
    let mut result = M3uObjectList::new();
    let mut list = Vec::new();
    let exp_line = _body.lines();
    let mut m3u_ext = M3uExt { x_tv_url: vec![] };
    let mut index = 1;
    let mut one_m3u = Vec::new();
    let mut save_mode = false;

    // 逐行解析M3U内容
    for x in exp_line {
        if x.starts_with("#EXTM3U") {
            m3u_ext = parse_m3u_header(x.to_owned());
        } else {
            if x.starts_with("#EXTINF") {
                save_mode = true;
                one_m3u.push(x);
            } else {
                if save_mode {
                    one_m3u.push(x);
                    if is_url(x.to_string()) {
                        let item = parse_one_m3u(one_m3u.clone(), index);
                        match item {
                            Some(data) => {
                                index += 1;
                                list.push(data);
                                one_m3u = Vec::new();
                            }
                            None => {}
                        }
                        save_mode = false
                    }
                }
            }
        }
    }
    result.set_list(list);
    result.set_header(m3u_ext);
    result
}

/// 解析M3U头部信息
///
/// # 参数
/// * `_str` - M3U头部字符串
///
/// # 返回值
/// * `M3uExt` - 解析后的M3U扩展信息
fn parse_m3u_header(_str: String) -> M3uExt {
    let mut x_tv_url_arr: Vec<String> = Vec::new();
    if let Some(title) = _str.split("x-tvg-url=\"").nth(1) {
        let exp_str = title.split('"').next().unwrap();
        let list: Vec<&str> = exp_str.split(',').collect();
        for x in list {
            x_tv_url_arr.push(x.to_string())
        }
    }
    M3uExt {
        x_tv_url: x_tv_url_arr.to_owned(),
    }
}

/// 解析单个M3U条目
///
/// # 参数
/// * `_arr` - M3U条目字符串数组
/// * `index` - 条目索引
///
/// # 返回值
/// * `Option<M3uObject>` - 解析后的M3U对象
fn parse_one_m3u(_arr: Vec<&str>, index: i32) -> Option<M3uObject> {
    let url = _arr.last().unwrap().to_string();
    if _arr.first().unwrap().starts_with("#EXTINF") && is_url(url.to_owned()) {
        let mut extend = M3uExtend::new();

        // 解析各种扩展属性
        if let Some(title) = _arr.first().unwrap().split("group-title=\"").nth(1) {
            extend.set_group_title(title.split('"').next().unwrap().to_owned())
        }
        if let Some(tv_id) = _arr.first().unwrap().split("tvg-id=\"").nth(1) {
            extend.set_tv_id(tv_id.split('"').next().unwrap().to_owned())
        }
        if let Some(tv_logo) = _arr.first().unwrap().split("tvg-logo=\"").nth(1) {
            extend.set_tv_logo(tv_logo.split('"').next().unwrap().to_owned())
        }
        if let Some(tv_country) = _arr.first().unwrap().split("tvg-country=\"").nth(1) {
            extend.set_tv_country(tv_country.split('"').next().unwrap().to_owned())
        }
        if let Some(tv_language) = _arr.first().unwrap().split("tvg-language=\"").nth(1) {
            extend.set_tv_language(tv_language.split('"').next().unwrap().to_owned())
        }
        if let Some(user_agent) = _arr.first().unwrap().split("user-agent=\"").nth(1) {
            extend.set_user_agent(user_agent.split('"').next().unwrap().to_owned())
        }

        // 解析频道名称
        let exp: Vec<&str> = _arr.first().unwrap().split(',').collect();
        let name = exp.last().unwrap();

        // 创建M3U对象并设置属性
        let mut m3u_obj = M3uObject::new();
        let simple_name = translator_t2s(&name.to_string());
        m3u_obj.set_extend(extend);
        m3u_obj.set_index(index);
        m3u_obj.set_url(url.to_string());
        m3u_obj.set_name(name.to_string());
        m3u_obj.set_search_name(simple_name);
        m3u_obj.set_raw(_arr.join("\n").to_string());
        return Some(m3u_obj);
    }
    return None;
}

/// 解析带引号的M3U格式字符串
///
/// # 参数
/// * `_body` - 带引号的M3U格式字符串
///
/// # 返回值
/// * `M3uObjectList` - 解析后的M3U对象列表
pub fn parse_quota_str(_body: String) -> M3uObjectList {
    let mut result = M3uObjectList::new();
    let mut list = Vec::new();
    let exp_line = _body.lines();
    let mut now_group = String::from("");
    let mut index = 1;

    // 逐行解析M3U内容
    for x in exp_line {
        let one_c: Vec<&str> = x.split(',').collect();
        let mut name = String::from("");
        let mut url = String::from("");

        // 解析名称和URL
        match one_c.first() {
            Some(pname) => {
                name = pname.to_string();
            }
            None => {}
        }

        match one_c.get(1) {
            Some(purl) => {
                url = purl.replace('\r', "").to_string();
            }
            None => {}
        }

        // 处理分组和频道信息
        if !name.is_empty() && !url.is_empty() {
            if !is_url(url.clone()) {
                now_group = name.to_string();
            } else {
                let simple_name = translator_t2s(&name.to_string());
                let mut m3u_obj = M3uObject::new();
                let mut extend = M3uExtend::new();
                extend.set_group_title(now_group.clone());
                m3u_obj.set_extend(extend);
                m3u_obj.set_index(index);
                m3u_obj.set_url(url.to_string());
                m3u_obj.set_name(name.to_string());
                m3u_obj.set_search_name(simple_name.to_string());
                m3u_obj.set_raw(x.replace('\r', "").to_owned());
                index += 1;
                list.push(m3u_obj)
            }
        }
    }
    result.set_list(list);
    return result;
}

/// 检查字符串是否为有效的URL
///
/// # 参数
/// * `_str` - 要检查的字符串
///
/// # 返回值
/// * `bool` - 如果是有效的URL返回true，否则返回false
pub fn is_url(_str: String) -> bool {
    let _url = &_str;
    let check_url = Url::parse(_url);
    return match check_url {
        Ok(_) => true,
        Err(_) => false,
    };
}

pub fn get_video_resolution(height: u32) -> QualityType {
    match height {
        h if h <= 240 => Quality240P,              // 240p: 高度 <= 240
        h if h > 240 && h <= 360 => Quality360P,   // 360p: 240 < 高度 <= 360
        h if h > 360 && h <= 480 => Quality480P,   // 480p: 360 < 高度 <= 480
        h if h > 480 && h <= 720 => Quality720P,   // 720p: 480 < 高度 <= 720
        h if h > 720 && h <= 1080 => Quality1080P, // 1080p: 720 < 高度 <= 1080
        h if h > 1080 && h <= 1440 => Quality2K,   // 2K: 1080 < 高度 <= 1440
        h if h > 1440 && h <= 2160 => Quality4K,   // 4K: 1440 < 高度 <= 2160
        h if h > 2160 => Quality8K,                // 8K: 高度 > 2160
        _ => QualityUnknown,                       // 未知分辨率
    }
}

pub fn from_video_resolution(list: Vec<String>) -> Vec<QualityType> {
    let mut result = Vec::new();
    for x in list {
        if x.to_lowercase().eq("240p") {
            result.push(Quality240P);
        } else if x.to_lowercase().eq("360p") {
            result.push(Quality360P);
        } else if x.to_lowercase().eq("480p") {
            result.push(Quality480P);
        } else if x.to_lowercase().eq("720p") {
            result.push(Quality720P);
        } else if x.to_lowercase().eq("1080p") {
            result.push(Quality1080P);
        } else if x.to_lowercase().eq("2k") {
            result.push(Quality2K);
        } else if x.to_lowercase().eq("4k") {
            result.push(Quality4K);
        }
    }
    result
}

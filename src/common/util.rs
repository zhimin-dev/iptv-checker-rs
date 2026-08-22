use crate::common::QualityType::{
    Quality1080P, Quality240P, Quality2K, Quality360P, Quality480P, Quality4K, Quality720P,
    Quality8K, QualityUnknown,
};
use crate::common::{M3uExt, M3uExtend, M3uObject, M3uObjectList, QualityType};
use crate::utils::translator_t2s;
use once_cell::sync::Lazy;
use reqwest::Error;
use std::sync::RwLock;
use url::Url;

/// Default timeout for HTTP requests (20 seconds)
pub const DEFAULT_HTTP_TIMEOUT: u64 = 20000;

/// Version string for constructing the default User-Agent header
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Get the effective User-Agent string (custom if configured, otherwise default)
pub fn get_user_agent() -> String {
    let config = crate::config::network::get_network_config();
    if !config.user_agent.trim().is_empty() {
        config.user_agent.clone()
    } else {
        format!("iptv-checker/v{}", APP_VERSION)
    }
}

/// Rebuildable HTTP client stored behind a RwLock so proxy/header settings
/// can be changed at runtime without restarting the server.
static HTTP_CLIENT_INNER: Lazy<RwLock<reqwest::Client>> = Lazy::new(|| {
    RwLock::new(build_http_client())
});

/// Build a reqwest::Client from the current NetworkConfig settings.
fn build_http_client() -> reqwest::Client {
    let config = crate::config::network::get_network_config();
    let mut builder = reqwest::Client::builder()
        .danger_accept_invalid_certs(true);

    if config.use_system_proxy {
        // Follow system proxy (env vars like HTTP_PROXY from Clash, etc.)
        log::info!("HTTP client using system proxy (if configured)");
        // Windows 全局代理（系统代理开关）不体现在环境变量里，需从注册表读取
        #[cfg(target_os = "windows")]
        {
            if let Some(url) = get_windows_system_proxy() {
                if let Ok(proxy) = reqwest::Proxy::all(&url) {
                    builder = builder.proxy(proxy);
                    log::info!("HTTP client using Windows system proxy: {}", url);
                }
            }
        }
    } else {
        // Disable system proxy detection
        builder = builder.no_proxy();
        // Apply user-specified proxy if configured
        if !config.proxy_url.trim().is_empty() {
            match reqwest::Proxy::all(&config.proxy_url) {
                Ok(proxy) => {
                    builder = builder.proxy(proxy);
                    log::info!("HTTP client using custom proxy: {}", config.proxy_url);
                }
                Err(e) => {
                    log::error!("Invalid proxy URL '{}': {}", config.proxy_url, e);
                }
            }
        }
    }

    // Build default headers: User-Agent first, then custom headers
    let mut headers = reqwest::header::HeaderMap::new();
    let ua = get_user_agent();
    if let Ok(hv) = reqwest::header::HeaderValue::from_str(&ua) {
        headers.insert(reqwest::header::USER_AGENT, hv);
    }
    for (key, value) in &config.custom_headers {
        if let (Ok(hk), Ok(hv)) = (
            reqwest::header::HeaderName::from_bytes(key.as_bytes()),
            reqwest::header::HeaderValue::from_str(value),
        ) {
            headers.insert(hk, hv);
        }
    }
    builder = builder.default_headers(headers);

    builder.build().expect("Failed to build shared HTTP client")
}

/// Rebuild the shared HTTP client from current config (call after config changes).
pub fn rebuild_http_client() {
    let new_client = build_http_client();
    let mut client = HTTP_CLIENT_INNER.write().unwrap();
    *client = new_client;
    log::info!("HTTP client rebuilt with current proxy/header settings");
}

/// Convenience accessor — clones the current client for use in requests.
pub fn get_http_client() -> reqwest::Client {
    HTTP_CLIENT_INNER.read().unwrap().clone()
}

/// 读取 Windows 系统代理（WinINET，即“全局代理”开关，Clash/浏览器等开启系统代理时写入此处）。
/// 返回形如 http://127.0.0.1:7890 的代理地址；未开启或读取失败返回 None。
#[cfg(target_os = "windows")]
pub fn get_windows_system_proxy() -> Option<String> {
    use std::os::windows::process::CommandExt;
    let query = |value: &str| -> Option<String> {
        let out = std::process::Command::new("reg")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
                "/v",
                value,
            ])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output()
            .ok()?;
        String::from_utf8(out.stdout).ok()
    };
    // ProxyEnable=0x1 表示系统代理已开启
    if !query("ProxyEnable")?.contains("0x1") {
        return None;
    }
    let server_raw = query("ProxyServer")?;
    let line = server_raw
        .lines()
        .find(|l| l.contains("REG_SZ") || l.contains("REG_EXPAND_SZ"))?;
    let raw = line.split_whitespace().next_back()?.trim().to_string();
    // 形如 http=127.0.0.1:7890;https=...;socks=... 或直接是 127.0.0.1:7890
    let mut server = raw.clone();
    for proto in ["http=", "socks=", "https="] {
        if raw.contains(proto) {
            server = raw
                .split(proto)
                .nth(1)
                .unwrap_or("")
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            break;
        }
    }
    if server.is_empty() {
        return None;
    }
    Some(if server.contains("://") {
        server
    } else {
        format!("http://{}", server)
    })
}

/// 给子进程（ffmpeg/ffprobe 等）注入代理环境变量，使 ffmpeg 任务遵循网络代理配置。
/// - use_system_proxy=true：继承进程环境（ffmpeg 自带读取 http_proxy/https_proxy），
///   Windows 下另把系统代理（WinINET 全局代理）写入子进程环境；
/// - use_system_proxy=false 且配置了 proxy_url：子进程强制走该代理；
/// - use_system_proxy=false 且未配置：清除子进程的代理环境变量，强制直连。
pub fn apply_proxy_to_command<C: ProxyEnv>(cmd: &mut C) {
    let config = crate::config::network::get_network_config();
    if config.use_system_proxy {
        // 继承环境变量代理；Windows 系统代理不体现在环境变量里，需显式注入
        #[cfg(target_os = "windows")]
        if let Some(url) = get_windows_system_proxy() {
            for key in [
                "http_proxy",
                "https_proxy",
                "HTTP_PROXY",
                "HTTPS_PROXY",
                "all_proxy",
                "ALL_PROXY",
            ] {
                cmd.proxy_env(key, &url);
            }
        }
    } else if !config.proxy_url.trim().is_empty() {
        let url = config.proxy_url.trim().to_string();
        for key in [
            "http_proxy",
            "https_proxy",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "all_proxy",
            "ALL_PROXY",
        ] {
            cmd.proxy_env(key, &url);
        }
    } else {
        for key in [
            "http_proxy",
            "https_proxy",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "all_proxy",
            "ALL_PROXY",
        ] {
            cmd.proxy_env_remove(key);
        }
    }
}

/// std::process::Command 与 tokio::process::Command 的统一环境变量注入接口
pub trait ProxyEnv {
    fn proxy_env(&mut self, key: &str, val: &str) -> &mut Self;
    fn proxy_env_remove(&mut self, key: &str) -> &mut Self;
}

impl ProxyEnv for std::process::Command {
    fn proxy_env(&mut self, key: &str, val: &str) -> &mut Self {
        self.env(key, val)
    }
    fn proxy_env_remove(&mut self, key: &str) -> &mut Self {
        self.env_remove(key)
    }
}

impl ProxyEnv for tokio::process::Command {
    fn proxy_env(&mut self, key: &str, val: &str) -> &mut Self {
        self.env(key, val)
    }
    fn proxy_env_remove(&mut self, key: &str) -> &mut Self {
        self.env_remove(key)
    }
}

/// Shared reqwest Client for GitHub API requests (unauthenticated).
/// Does NOT disable certificate verification — GitHub always has valid TLS.
/// For authenticated requests, callers should add `.bearer_auth(token)` on each request.
pub static GITHUB_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("iptv-checker-rs"),
    );
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/vnd.github+json"),
    );
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .expect("Failed to build GitHub API client")
});

/// Get the GitHub token from base.json config, if configured.
pub fn get_github_token() -> Option<String> {
    let token = crate::config::base::get_base_config().github_token;
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

/// 获取URL的内容
///
/// # 参数
/// * `_url` - 要获取内容的URL
/// * `timeout` - 超时时间（毫秒）
///
/// # 返回值
/// * `Result<String, Error>` - 成功返回URL内容，失败返回错误
pub async fn get_url_body(_url: String, timeout: u64) -> Result<String, Error> {
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(timeout))
        .danger_accept_invalid_certs(true);

    // Apply proxy and headers from config
    let config = crate::config::network::get_network_config();
    if config.use_system_proxy {
        // Windows 全局代理（系统代理开关）不体现在环境变量里，需从注册表读取
        #[cfg(target_os = "windows")]
        if let Some(url) = get_windows_system_proxy() {
            if let Ok(proxy) = reqwest::Proxy::all(&url) {
                builder = builder.proxy(proxy);
            }
        }
    } else {
        builder = builder.no_proxy();
        if !config.proxy_url.trim().is_empty() {
            if let Ok(proxy) = reqwest::Proxy::all(&config.proxy_url) {
                builder = builder.proxy(proxy);
            }
        }
    }
    let mut headers = reqwest::header::HeaderMap::new();
    let ua = get_user_agent();
    if let Ok(hv) = reqwest::header::HeaderValue::from_str(&ua) {
        headers.insert(reqwest::header::USER_AGENT, hv);
    }
    for (key, value) in &config.custom_headers {
        if let (Ok(hk), Ok(hv)) = (
            reqwest::header::HeaderName::from_bytes(key.as_bytes()),
            reqwest::header::HeaderValue::from_str(value),
        ) {
            headers.insert(hk, hv);
        }
    }
    builder = builder.default_headers(headers);

    let client = builder.build().unwrap();
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
        if let Some(tv_name) = _arr.first().unwrap().split("tvg-name=\"").nth(1) {
            extend.set_tv_name(tv_name.split('"').next().unwrap().to_owned())
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
        // Always try EPG matching. Use tv_name if set from M3U tags (more specific),
        // otherwise fall back to display name.
        let match_target = if !extend.tv_name.is_empty() {
            extend.tv_name.clone()
        } else {
            name.to_string()
        };
        m3u_obj.set_extend(extend);
        m3u_obj.set_index(index);
        m3u_obj.set_url(url.to_string());
        m3u_obj.set_name(name.to_string());
        m3u_obj.set_search_name(simple_name);
        m3u_obj.set_raw(_arr.join("\n").to_string());
        if let Some((epg_name, epg_id)) = crate::epg_mapping::match_epg_channel(&match_target) {
            if let Some(ext) = m3u_obj.get_extend_mut() {
                ext.set_tv_name(epg_name);
                ext.set_tv_id(epg_id);
            }
        }
        // Apply local group mapping: tvg-name → group-title
        if let Some(ref ext) = m3u_obj.get_extend_ref() {
            if !ext.tv_name.is_empty() {
                if let Some(mapped_group) = crate::config::group::get_group_for_channel(&ext.tv_name) {
                    if let Some(ext_mut) = m3u_obj.get_extend_mut() {
                        ext_mut.set_group_title(mapped_group);
                    }
                }
            }
        }
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
                // EPG matching: try to find tvg-name and tvg-id from EPG mapping
                if let Some((epg_name, epg_id)) = crate::epg_mapping::match_epg_channel(&name) {
                    if let Some(ext) = m3u_obj.get_extend_mut() {
                        ext.set_tv_name(epg_name);
                        ext.set_tv_id(epg_id);
                    }
                }
                // Apply local group mapping: tvg-name → group-title
                {
                    let tv_name = m3u_obj.get_extend_ref()
                        .and_then(|e| if e.tv_name.is_empty() { None } else { Some(e.tv_name.clone()) });
                    if let Some(tvn) = tv_name {
                        if let Some(mapped_group) = crate::config::group::get_group_for_channel(&tvn) {
                            if let Some(ext) = m3u_obj.get_extend_mut() {
                                ext.set_group_title(mapped_group);
                            }
                        }
                    }
                }
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
        } else if x.to_lowercase().eq("8k") {
            result.push(Quality8K);
        }
    }
    result
}

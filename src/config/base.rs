use crate::r#const::constant::{BASE_CONFIG_JSON_CONTENT, BASE_JSON};
use crate::utils::file_exists;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::RwLock;
use log::{error, info, warn};

/// Base 配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseConfig {
    pub host: String,
    pub replace_string: bool,
    pub remote_url2local_images: bool,
    #[serde(default)]
    pub github_token: String,
    #[serde(default)]
    pub rename_channel_type: i8,
    // Note: network-related fields (proxy_url, custom_headers, user_agent)
    // have been moved to config::network::NetworkConfig (network.json)
}

impl BaseConfig {
    fn new() -> Self {
        BaseConfig {
            host: String::default(),
            replace_string: false,
            remote_url2local_images: false,
            github_token: String::default(),
            rename_channel_type: 0,
        }
    }
}

static BASE_MAP: Lazy<RwLock<BaseConfig>> = Lazy::new(|| {
    let p = Path::new(get_base_file_path().as_str()).to_owned();
    RwLock::new(read_base_json(&p))
});

pub fn get_base_config() -> BaseConfig {
    BASE_MAP.read().unwrap().clone()
}

/// 获取 Base 配置的 JSON 字符串
pub fn get_base_json() -> Result<String, String> {
    let config = BASE_MAP.read().unwrap();
    serde_json::to_string_pretty(&*config)
        .map_err(|e| format!("Failed to serialize base config: {}", e))
}

/// 从 JSON 字符串解析并更新 Base 配置
pub fn update_base_from_json(json: &str) -> Result<(), String> {
    let config: BaseConfig = serde_json::from_str(json)
        .map_err(|e| format!("Failed to parse base JSON: {}", e))?;
    update_base_config(config)
}

/// 读取 base.json 文件内容（字符串形式）
pub fn read_base_json_string() -> Result<String, String> {
    fs::read_to_string(get_base_file_path())
        .map_err(|e| format!("Failed to read base.json: {}", e))
}

/// 获取有效的 host，优先 base.json，回退 logos.json，自动补 http://
pub fn get_effective_host() -> String {
    let raw = {
        let base_host = get_base_config().host;
        if !base_host.trim().is_empty() {
            base_host.trim_end_matches('/').to_string()
        } else {
            let logos_host = crate::config::logos::get_logos_config().host;
            if !logos_host.trim().is_empty() {
                logos_host.trim_end_matches('/').to_string()
            } else {
                String::new()
            }
        }
    };
    if raw.is_empty() || raw.starts_with("http://") || raw.starts_with("https://") {
        raw
    } else {
        format!("http://{}", raw)
    }
}

/// 部分更新 Base 配置（host、replace_string、remote_url2local_images、github_token）
/// Also normalizes host to include http:// if protocol is missing.
pub fn partial_update_base_config(
    host: String,
    replace_string: bool,
    remote_url2local_images: bool,
    github_token: String,
    rename_channel_type: i8,
) -> Result<(), String> {
    let mut config = get_base_config();
    config.host = host;
    config.replace_string = replace_string;
    config.remote_url2local_images = remote_url2local_images;
    config.github_token = github_token;
    config.rename_channel_type = rename_channel_type;
    update_base_config(config)
}

/// Validate a GitHub personal access token by making a test API call.
/// Returns Ok(()) if the token is valid, Err(msg) otherwise.
pub async fn validate_github_token(token: &str) -> Result<(), String> {
    if token.is_empty() {
        return Ok(()); // empty token is always "valid" (means no auth)
    }

    let client = reqwest::Client::builder()
        .user_agent("iptv-checker-rs")
        .build()
        .map_err(|e| format!("Failed to build client: {}", e))?;

    let resp = client
        .get("https://api.github.com/rate_limit")
        .header("Authorization", &format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Failed to validate token: {}", e))?;

    match resp.status().as_u16() {
        200 => Ok(()),
        401 => Err("GitHub token is invalid (401 Unauthorized). Please check your token.".to_string()),
        403 => Err("GitHub token returned 403 Forbidden. The token may lack permissions.".to_string()),
        s if s >= 400 => Err(format!("GitHub API returned HTTP {} while validating token", s)),
        _ => Ok(()),
    }
}

/// 重新加载 base.json 文件
pub fn reload_base_map() -> Result<(), String> {
    let p = Path::new(get_base_file_path().as_str()).to_owned();
    let new_map = read_base_json(&p);
    let mut map = BASE_MAP.write().unwrap();
    *map = new_map;
    Ok(())
}

fn read_base_json<P: AsRef<Path>>(path: P) -> BaseConfig {
    match fs::read_to_string(&path) {
        Ok(s) => {
            if s.trim().is_empty() {
                warn!("base: file {:?} is empty", path.as_ref());
                return BaseConfig::new();
            }
            match serde_json::from_str::<BaseConfig>(&s) {
                Ok(m) => {
                    error!(
                        "base: successfully loaded from {:?}",
                        path.as_ref()
                    );
                    m
                }
                Err(e) => {
                    error!(
                        "base: failed to parse JSON from {:?}: {}",
                        path.as_ref(),
                        e
                    );
                    error!("base: file content: {}", s);
                    BaseConfig::new()
                }
            }
        }
        Err(e) => {
            error!("base: failed to read {:?}: {}", path.as_ref(), e);
            BaseConfig::new()
        }
    }
}

/// 更新整个 Base 配置
pub fn update_base_config(config: BaseConfig) -> Result<(), String> {
    let mut map = BASE_MAP.write().unwrap();
    *map = config;
    drop(map);
    save_base_to_file()
}

/// 保存 Base 配置到文件
pub fn save_base_to_file() -> Result<(), String> {
    let map = BASE_MAP.read().unwrap();
    let json = serde_json::to_string_pretty(&*map)
        .map_err(|e: serde_json::Error| format!("Failed to serialize base config: {}", e))?;
    fs::write(get_base_file_path(), json)
        .map_err(|e| format!("Failed to write base config: {}", e))?;
    Ok(())
}

pub fn get_base_file_path() -> String {
    format!("./{}", BASE_JSON)
}

pub fn create_base_file() {
    if !file_exists(&get_base_file_path()) {
        if let Some(parent) = std::path::Path::new(get_base_file_path().as_str()).parent() {
            fs::create_dir_all(parent).expect(&format!("Failed to create directory: {:?}", parent));
        }
        let mut fd = fs::File::create(get_base_file_path())
            .expect(&format!("Failed to create file: {}", get_base_file_path()));
        fd.write_all(BASE_CONFIG_JSON_CONTENT.to_string().as_bytes())
            .expect(&format!("Failed to write file: {}", get_base_file_path()));
        fd.flush()
            .expect(&format!("Failed to flush file: {}", get_base_file_path()));
    }
}

/// 兼容逻辑：若 logos.json 中 host 已配置且 base.json 中 host 为空，
/// 则将 logos.json 的 host 同步到 base.json
pub fn sync_host_from_logos_if_needed() {
    let logos_host = crate::config::logos::get_logos_config().host;
    if logos_host.trim().is_empty() {
        return;
    }
    let base_config = get_base_config();
    if !base_config.host.trim().is_empty() {
        return;
    }
    if let Err(e) = partial_update_base_config(
        logos_host,
        base_config.replace_string,
        base_config.remote_url2local_images,
        base_config.github_token,
        base_config.rename_channel_type,
    ) {
        error!("sync_host_from_logos: failed to sync host to base.json: {}", e);
    } else {
        info!("sync_host_from_logos: synced host from logos.json to base.json");
    }
}

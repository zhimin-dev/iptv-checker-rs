use crate::r#const::constant::{NETWORK_CONFIG_JSON_CONTENT, NETWORK_JSON};
use crate::utils::file_exists;
use crate::utils::deserialize_bool_flexible;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::RwLock;
use log::{error, info, warn};

/// 网络配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// HTTP proxy URL, e.g. "http://127.0.0.1:7890"
    #[serde(default)]
    pub proxy_url: String,
    /// Whether to follow system proxy (env vars like HTTP_PROXY).
    /// Default true — when enabled, system proxy takes precedence over proxy_url.
    #[serde(default = "default_true", deserialize_with = "deserialize_bool_flexible")]
    pub use_system_proxy: bool,
    /// Custom HTTP headers as key-value pairs
    #[serde(default)]
    pub custom_headers: HashMap<String, String>,
    /// Custom User-Agent; defaults to "iptv-checker/v{version}" if empty
    #[serde(default)]
    pub user_agent: String,
}

fn default_true() -> bool {
    true
}

impl NetworkConfig {
    fn new() -> Self {
        NetworkConfig {
            proxy_url: String::default(),
            use_system_proxy: true,
            custom_headers: HashMap::new(),
            user_agent: String::default(),
        }
    }
}

static NETWORK_MAP: Lazy<RwLock<NetworkConfig>> = Lazy::new(|| {
    let p = Path::new(get_network_file_path().as_str()).to_owned();
    RwLock::new(read_network_json(&p))
});

pub fn get_network_config() -> NetworkConfig {
    NETWORK_MAP.read().unwrap().clone()
}

/// 获取网络配置的 JSON 字符串
pub fn get_network_json() -> Result<String, String> {
    let config = NETWORK_MAP.read().unwrap();
    serde_json::to_string_pretty(&*config)
        .map_err(|e| format!("Failed to serialize network config: {}", e))
}

/// 从 JSON 字符串解析并更新网络配置
pub fn update_network_from_json(json: &str) -> Result<(), String> {
    let config: NetworkConfig = serde_json::from_str(json)
        .map_err(|e| format!("Failed to parse network JSON: {}", e))?;
    update_network_config(config)
}

/// 读取 network.json 文件内容（字符串形式）
pub fn read_network_json_string() -> Result<String, String> {
    fs::read_to_string(get_network_file_path())
        .map_err(|e| format!("Failed to read network.json: {}", e))
}

/// 重新加载 network.json 文件
pub fn reload_network_map() -> Result<(), String> {
    let p = Path::new(get_network_file_path().as_str()).to_owned();
    let new_map = read_network_json(&p);
    let mut map = NETWORK_MAP.write().unwrap();
    *map = new_map;
    Ok(())
}

fn read_network_json<P: AsRef<Path>>(path: P) -> NetworkConfig {
    match fs::read_to_string(&path) {
        Ok(s) => {
            if s.trim().is_empty() {
                warn!("network: file {:?} is empty", path.as_ref());
                return NetworkConfig::new();
            }
            match serde_json::from_str::<NetworkConfig>(&s) {
                Ok(m) => {
                    info!("network: successfully loaded from {:?}", path.as_ref());
                    m
                }
                Err(e) => {
                    error!(
                        "network: failed to parse JSON from {:?}: {}",
                        path.as_ref(), e
                    );
                    NetworkConfig::new()
                }
            }
        }
        Err(e) => {
            error!("network: failed to read {:?}: {}", path.as_ref(), e);
            NetworkConfig::new()
        }
    }
}

/// 更新整个网络配置
pub fn update_network_config(config: NetworkConfig) -> Result<(), String> {
    let mut map = NETWORK_MAP.write().unwrap();
    *map = config;
    drop(map);
    save_network_to_file()
}

/// 保存网络配置到文件
pub fn save_network_to_file() -> Result<(), String> {
    let map = NETWORK_MAP.read().unwrap();
    let json = serde_json::to_string_pretty(&*map)
        .map_err(|e: serde_json::Error| format!("Failed to serialize network config: {}", e))?;
    fs::write(get_network_file_path(), json)
        .map_err(|e| format!("Failed to write network config: {}", e))?;
    Ok(())
}

pub fn get_network_file_path() -> String {
    format!("./{}", NETWORK_JSON)
}

pub fn create_network_file() {
    if !file_exists(&get_network_file_path()) {
        if let Some(parent) = std::path::Path::new(get_network_file_path().as_str()).parent() {
            fs::create_dir_all(parent).expect(&format!("Failed to create directory: {:?}", parent));
        }
        let mut fd = fs::File::create(get_network_file_path())
            .expect(&format!("Failed to create file: {}", get_network_file_path()));

        // Try to migrate from base.json if network fields exist there
        let content = migrate_network_from_base();
        fd.write_all(content.as_bytes())
            .expect(&format!("Failed to write file: {}", get_network_file_path()));
        fd.flush()
            .expect(&format!("Failed to flush file: {}", get_network_file_path()));
    }
}

/// Migrate network settings from base.json if present (one-time migration)
fn migrate_network_from_base() -> String {
    let base_path = crate::config::base::get_base_file_path();
    if let Ok(content) = fs::read_to_string(&base_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            let proxy_url = json.get("proxy_url").and_then(|v| v.as_str()).unwrap_or("");
            let use_system_proxy = json.get("use_system_proxy").and_then(|v| v.as_bool()).unwrap_or(true);
            let custom_headers = json.get("custom_headers").cloned().unwrap_or(serde_json::Value::Object(Default::default()));
            let user_agent = json.get("user_agent").and_then(|v| v.as_str()).unwrap_or("");

            let migrated = serde_json::json!({
                "proxy_url": proxy_url,
                "use_system_proxy": use_system_proxy,
                "custom_headers": custom_headers,
                "user_agent": user_agent,
            });
            info!("network: migrated settings from base.json");
            return serde_json::to_string_pretty(&migrated).unwrap_or_else(|_| NETWORK_CONFIG_JSON_CONTENT.to_string());
        }
    }
    NETWORK_CONFIG_JSON_CONTENT.to_string()
}

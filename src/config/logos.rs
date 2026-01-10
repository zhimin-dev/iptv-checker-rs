use crate::r#const::constant::{LOGOS_CONFIG_JSON_CONTENT, LOGOS_JSON};
use crate::utils::file_exists;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::RwLock;

/// Logos配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogosConfig {
    pub host: String,
    pub remote_url2local_images: bool,
    pub logos: Vec<LogoItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoItem {
    pub url: String,
    pub name: Vec<String>,
}

impl LogosConfig {
    fn new() -> Self {
        LogosConfig {
            host: String::default(),
            remote_url2local_images: false,
            logos: Vec::new(),
        }
    }
}

static LOGOS_MAP: Lazy<RwLock<LogosConfig>> = Lazy::new(|| {
    let p = Path::new(get_logos_file_path().as_str()).to_owned();
    RwLock::new(read_logos_json(p))
});

pub fn get_logos_config() -> LogosConfig {
    LOGOS_MAP.read().unwrap().clone()
}

/// 获取 Logos 配置的 JSON 字符串
pub fn get_logos_json() -> Result<String, String> {
    let config = LOGOS_MAP.read().unwrap();
    serde_json::to_string_pretty(&*config)
        .map_err(|e| format!("Failed to serialize logos config: {}", e))
}

/// 从 JSON 字符串解析并更新 Logos 配置
pub fn update_logos_from_json(json: &str) -> Result<(), String> {
    let config: LogosConfig = serde_json::from_str(json)
        .map_err(|e| format!("Failed to parse logos JSON: {}", e))?;
    update_logos_config(config)
}

/// 读取 logos.json 文件内容（字符串形式）
pub fn read_logos_json_string() -> Result<String, String> {
    fs::read_to_string(get_logos_file_path())
        .map_err(|e| format!("Failed to read logos.json: {}", e))
}

/// 部分更新 Logos 配置（用于 API）
pub fn partial_update_logos_config(
    host: String,
    remote_url2local_images: bool,
) -> Result<(), String> {
    let mut config = get_logos_config();
    config.host = host;
    config.remote_url2local_images = remote_url2local_images;
    
    update_logos_config(config)
}

/// 根据 URL 查找 LogoItem
pub fn find_logo_by_url(url: &str) -> Option<LogoItem> {
    let config = get_logos_config();
    config.logos.into_iter().find(|logo| logo.url == url)
}

/// 更新某个 Logo 的名称列表
pub fn update_logo_names(url: &str, new_names: Vec<String>) -> Result<(), String> {
    let mut config = get_logos_config();
    
    if let Some(logo) = config.logos.iter_mut().find(|l| l.url == url) {
        logo.name = new_names;
        update_logos_config(config)
    } else {
        Err(format!("Logo with URL {} not found", url))
    }
}

/// 从旧格式迁移并保存（支持多种格式）
pub fn migrate_and_save_logos(
    existing_data: std::collections::HashMap<String, std::collections::HashSet<String>>,
    host: String,
    remote_url2local_images: bool,
) -> Result<(), String> {
    use std::collections::HashSet;
    
    let mut final_list: Vec<LogoItem> = Vec::new();
    let mut processed_urls = HashSet::new();
    
    for (url, names) in existing_data {
        if !processed_urls.contains(&url) {
            final_list.push(LogoItem {
                url: url.clone(),
                name: names.into_iter().collect(),
            });
            processed_urls.insert(url);
        }
    }
    
    let full_config = LogosConfig {
        host,
        remote_url2local_images,
        logos: final_list,
    };
    
    update_logos_config(full_config)
}

/// 获取 Logo 映射表（name -> url），用于 M3U 文件的 logo 替换
pub fn get_logos_map() -> std::collections::HashMap<String, String> {
    let mut logos_map = std::collections::HashMap::new();
    let config = get_logos_config();
    
    for item in config.logos {
        for name in item.name {
            logos_map.insert(name, item.url.clone());
        }
    }
    
    logos_map
}

/// 从 JSON 字符串构建 Logo 映射表（兼容旧格式）
pub fn get_logos_map_from_json(json: &str) -> std::collections::HashMap<String, String> {
    let mut logos_map = std::collections::HashMap::new();
    
    // 尝试解析为新的完整格式
    if let Ok(config) = serde_json::from_str::<LogosConfig>(json) {
        for item in config.logos {
            for name in item.name {
                logos_map.insert(name, item.url.clone());
            }
        }
    }
    // 尝试解析为 List 格式
    else if let Ok(list) = serde_json::from_str::<Vec<LogoItem>>(json) {
        for item in list {
            for name in item.name {
                logos_map.insert(name, item.url.clone());
            }
        }
    }
    // 尝试解析为旧的 Map 格式
    else if let Ok(map) = serde_json::from_str::<std::collections::HashMap<String, String>>(json) {
        logos_map = map;
    }
    
    logos_map
}

/// 重新加载 logos.json 文件
pub fn reload_logos_map() -> Result<(), String> {
    let p = Path::new(get_logos_file_path().as_str()).to_owned();
    let new_map = read_logos_json(&p);
    let mut map = LOGOS_MAP.write().unwrap();
    *map = new_map;
    Ok(())
}

/// 根据频道名获取对应的 Logo URL
pub fn get_logo_url_by_name(channel_name: &str) -> Option<String> {
    let config = get_logos_config();
    for logo in config.logos {
        if logo.name.iter().any(|n| n == channel_name) {
            return Some(logo.url.clone());
        }
    }
    None
}

/// 获取所有 Logo 配置列表
pub fn get_logos_list() -> Vec<LogoItem> {
    let config = get_logos_config();
    config.logos
}

fn read_logos_json<P: AsRef<Path>>(path: P) -> LogosConfig {
    match fs::read_to_string(&path) {
        Ok(s) => {
            if s.trim().is_empty() {
                eprintln!("logos: file {:?} is empty", path.as_ref());
                return LogosConfig::new();
            }
            match serde_json::from_str::<LogosConfig>(&s) {
                Ok(m) => {
                    eprintln!(
                        "logos: successfully loaded {} entries from {:?}",
                        m.logos.len(),
                        path.as_ref()
                    );
                    m
                }
                Err(e) => {
                    eprintln!(
                        "logos: failed to parse JSON from {:?}: {}",
                        path.as_ref(),
                        e
                    );
                    eprintln!("logos: file content: {}", s);
                    LogosConfig::new()
                }
            }
        }
        Err(e) => {
            eprintln!("logos: failed to read {:?}: {}", path.as_ref(), e);
            LogosConfig::new()
        }
    }
}

/// 添加 Logo 配置
pub fn add_logo(logo: LogoItem) -> Result<(), String> {
    let mut map = LOGOS_MAP.write().unwrap();
    map.logos.push(logo);
    drop(map);
    save_logos_to_file()
}

/// 删除 Logo 配置（根据 URL）
pub fn remove_logo_by_url(url: &str) -> Result<(), String> {
    let mut map = LOGOS_MAP.write().unwrap();
    map.logos.retain(|logo| logo.url != url);
    drop(map);
    save_logos_to_file()
}

/// 更新整个 Logos 配置
pub fn update_logos_config(config: LogosConfig) -> Result<(), String> {
    let mut map = LOGOS_MAP.write().unwrap();
    *map = config;
    drop(map);
    save_logos_to_file()
}

/// 保存 Logos 配置到文件
pub fn save_logos_to_file() -> Result<(), String> {
    let map = LOGOS_MAP.read().unwrap();
    let json = serde_json::to_string_pretty(&*map)
        .map_err(|e: serde_json::Error| format!("Failed to serialize logos config: {}", e))?;
    fs::write(get_logos_file_path(), json)
        .map_err(|e| format!("Failed to write logos config: {}", e))?;
    Ok(())
}

pub fn get_logos_file_path() -> String {
    format!("./{}", LOGOS_JSON)
}

pub fn create_logos_file() {
    if !file_exists(&get_logos_file_path()) {
        // 确保 core 目录存在
        if let Some(parent) = std::path::Path::new(get_logos_file_path().as_str()).parent() {
            fs::create_dir_all(parent).expect(&format!("Failed to create directory: {:?}", parent));
        }
        let mut fd = fs::File::create(get_logos_file_path()).expect(&format!(
            "Failed to create file: {}",
            get_logos_file_path()
        ));
        fd.write_all(LOGOS_CONFIG_JSON_CONTENT.to_string().as_bytes())
            .expect(&format!("Failed to write file: {}", get_logos_file_path()));
        fd.flush()
            .expect(&format!("Failed to flush file: {}", get_logos_file_path()));
    }
}
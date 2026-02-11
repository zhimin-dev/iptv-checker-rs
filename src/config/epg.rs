use crate::r#const::constant::{EPG_CONFIG_JSON_CONTENT, EPG_JSON};
use crate::utils::file_exists;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::RwLock;

/// EPG 配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpgConfig {
    pub source: EpgSource,
}

/// EPG 源结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpgSource {
    pub list: Vec<String>,
}

impl EpgConfig {
    fn new() -> Self {
        EpgConfig {
            source: EpgSource { list: Vec::new() },
        }
    }
}

static EPG_MAP: Lazy<RwLock<EpgConfig>> = Lazy::new(|| {
    let p = Path::new(get_epg_file_path().as_str()).to_owned();
    RwLock::new(read_epg_json(&p))
});

pub fn get_epg_config() -> EpgConfig {
    EPG_MAP.read().unwrap().clone()
}

/// 获取 EPG 配置的 JSON 字符串
pub fn get_epg_json() -> Result<String, String> {
    let config = EPG_MAP.read().unwrap();
    serde_json::to_string_pretty(&*config)
        .map_err(|e| format!("Failed to serialize epg config: {}", e))
}

/// 从 JSON 字符串解析并更新 EPG 配置
pub fn update_epg_from_json(json: &str) -> Result<(), String> {
    let config: EpgConfig = serde_json::from_str(json)
        .map_err(|e| format!("Failed to parse epg JSON: {}", e))?;
    update_epg_config(config)
}

/// 读取 epg.json 文件内容（字符串形式）
pub fn read_epg_json_string() -> Result<String, String> {
    fs::read_to_string(get_epg_file_path())
        .map_err(|e| format!("Failed to read epg.json: {}", e))
}

/// 获取 EPG 源 URL 列表
pub fn get_epg_list() -> Vec<String> {
    let config = get_epg_config();
    config.source.list
}

/// 添加 EPG 源 URL
pub fn add_epg_url(url: String) -> Result<(), String> {
    let mut map = EPG_MAP.write().unwrap();
    if !map.source.list.contains(&url) {
        map.source.list.push(url);
    }
    drop(map);
    save_epg_to_file()
}

/// 删除 EPG 源 URL（根据索引）
pub fn remove_epg_url_by_index(index: usize) -> Result<(), String> {
    let mut map = EPG_MAP.write().unwrap();
    if index < map.source.list.len() {
        map.source.list.remove(index);
        drop(map);
        save_epg_to_file()
    } else {
        Err(format!("Index {} out of bounds", index))
    }
}

/// 删除 EPG 源 URL（根据 URL 字符串）
pub fn remove_epg_url_by_url(url: &str) -> Result<(), String> {
    let mut map = EPG_MAP.write().unwrap();
    if let Some(pos) = map.source.list.iter().position(|u| u == url) {
        map.source.list.remove(pos);
        drop(map);
        save_epg_to_file()
    } else {
        Err(format!("EPG URL {} not found", url))
    }
}

/// 更新 EPG 源 URL 列表
pub fn update_epg_list(list: Vec<String>) -> Result<(), String> {
    let mut config = get_epg_config();
    config.source.list = list;
    update_epg_config(config)
}

/// 重新加载 epg.json 文件
pub fn reload_epg_map() -> Result<(), String> {
    let p = Path::new(get_epg_file_path().as_str()).to_owned();
    let new_map = read_epg_json(&p);
    let mut map = EPG_MAP.write().unwrap();
    *map = new_map;
    Ok(())
}

fn read_epg_json<P: AsRef<Path>>(path: P) -> EpgConfig {
    match fs::read_to_string(&path) {
        Ok(s) => {
            if s.trim().is_empty() {
                eprintln!("epg: file {:?} is empty", path.as_ref());
                return EpgConfig::new();
            }
            match serde_json::from_str::<EpgConfig>(&s) {
                Ok(m) => {
                    eprintln!(
                        "epg: successfully loaded {} URLs from {:?}",
                        m.source.list.len(),
                        path.as_ref()
                    );
                    m
                }
                Err(e) => {
                    eprintln!(
                        "epg: failed to parse JSON from {:?}: {}",
                        path.as_ref(),
                        e
                    );
                    eprintln!("epg: file content: {}", s);
                    EpgConfig::new()
                }
            }
        }
        Err(e) => {
            eprintln!("epg: failed to read {:?}: {}", path.as_ref(), e);
            EpgConfig::new()
        }
    }
}

/// 更新整个 EPG 配置
pub fn update_epg_config(config: EpgConfig) -> Result<(), String> {
    let mut map = EPG_MAP.write().unwrap();
    *map = config;
    drop(map);
    save_epg_to_file()
}

/// 保存 EPG 配置到文件
pub fn save_epg_to_file() -> Result<(), String> {
    let map = EPG_MAP.read().unwrap();
    let json = serde_json::to_string_pretty(&*map)
        .map_err(|e: serde_json::Error| format!("Failed to serialize epg config: {}", e))?;
    fs::write(get_epg_file_path(), json)
        .map_err(|e| format!("Failed to write epg config: {}", e))?;
    Ok(())
}

pub fn get_epg_file_path() -> String {
    format!("./{}", EPG_JSON)
}

pub fn create_epg_file() {
    if !file_exists(&get_epg_file_path()) {
        if let Some(parent) = std::path::Path::new(get_epg_file_path().as_str()).parent() {
            fs::create_dir_all(parent).expect(&format!("Failed to create directory: {:?}", parent));
        }
        let mut fd = fs::File::create(get_epg_file_path())
            .expect(&format!("Failed to create file: {}", get_epg_file_path()));
        fd.write_all(EPG_CONFIG_JSON_CONTENT.to_string().as_bytes())
            .expect(&format!("Failed to write file: {}", get_epg_file_path()));
        fd.flush()
            .expect(&format!("Failed to flush file: {}", get_epg_file_path()));
    }
}

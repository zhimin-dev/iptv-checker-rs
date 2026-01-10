use crate::r#const::constant::{FAVOURITE_CONFIG_JSON_CONTENT, FAVOURITE_JSON};
use crate::utils::file_exists;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::RwLock;

/// Replace配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavouriteConfig {
    #[serde(rename = "like")]
    pub like: Vec<String>,
    #[serde(rename = "equal")]
    pub equal: Vec<String>,
}

impl FavouriteConfig {
    fn new() -> Self {
        FavouriteConfig {
            like: Vec::new(),
            equal: Vec::new(),
        }
    }
}

static FAVOURITE_MAP: Lazy<RwLock<FavouriteConfig>> = Lazy::new(|| {
    let p = Path::new(format!("{}", FAVOURITE_JSON).as_mut_str()).to_owned();
    RwLock::new(read_favourite_json(p))
});

pub fn get_favourite_map() -> FavouriteConfig {
    FAVOURITE_MAP.read().unwrap().clone()
}

/// 获取收藏配置的 JSON 字符串
pub fn get_favourite_json() -> Result<String, String> {
    let config = FAVOURITE_MAP.read().unwrap();
    serde_json::to_string_pretty(&*config)
        .map_err(|e| format!("Failed to serialize favourite config: {}", e))
}

/// 重新加载 favourite.json 文件
pub fn reload_favourite_map() -> Result<(), String> {
    let p = Path::new(format!("{}", FAVOURITE_JSON).as_mut_str()).to_owned();
    let new_map = read_favourite_json(&p);
    let mut map = FAVOURITE_MAP.write().unwrap();
    *map = new_map;
    Ok(())
}


/// 获取某个分组（如 "频道收藏夹"）下的所有收藏频道列表
pub fn get_favourite_list(group: &str) -> Vec<String> {
    let map = get_favourite_map();
    if group == "like" {
        map.like
    } else if group == "equal" {
        map.equal
    } else {
        vec![]
    }
}

fn read_favourite_json<P: AsRef<Path>>(path: P) -> FavouriteConfig {
    match fs::read_to_string(&path) {
        Ok(s) => {
            if s.trim().is_empty() {
                eprintln!("favourite: file {:?} is empty", path.as_ref());
                return FavouriteConfig::new();
            }
            match serde_json::from_str::<FavouriteConfig>(&s) {
                Ok(m) => {
                    eprintln!(
                        "favourite: successfully loaded entries from {:?}",
                        path.as_ref()
                    );
                    m
                }
                Err(e) => {
                    eprintln!(
                        "favourite: failed to parse JSON from {:?}: {}",
                        path.as_ref(),
                        e
                    );
                    eprintln!("favourite: file content: {}", s);
                    FavouriteConfig::new()
                }
            }
        }
        Err(e) => {
            eprintln!("favourite: failed to read {:?}: {}", path.as_ref(), e);
            FavouriteConfig::new()
        }
    }
}

/// 添加频道到收藏列表
pub fn add_to_favourite(group: &str, channel: String) -> Result<(), String> {
    let mut map = FAVOURITE_MAP.write().unwrap();
    if group == "like" {
        if !map.like.contains(&channel) {
            map.like.push(channel);
        }
    } else if group == "equal" {
        if !map.equal.contains(&channel) {
            map.equal.push(channel);
        }
    } else {
        return Err(format!("Unknown group: {}", group));
    }
    drop(map);
    save_favourite_to_file()
}

/// 从收藏列表移除频道
pub fn remove_from_favourite(group: &str, channel: &str) -> Result<(), String> {
    let mut map = FAVOURITE_MAP.write().unwrap();
    if group == "like" {
        map.like.retain(|c| c != channel);
    } else if group == "equal" {
        map.equal.retain(|c| c != channel);
    } else {
        return Err(format!("Unknown group: {}", group));
    }
    drop(map);
    save_favourite_to_file()
}

/// 更新整个收藏配置
pub fn update_favourite_config(config: FavouriteConfig) -> Result<(), String> {
    let mut map = FAVOURITE_MAP.write().unwrap();
    *map = config;
    drop(map);
    save_favourite_to_file()
}

/// 保存收藏配置到文件
pub fn save_favourite_to_file() -> Result<(), String> {
    let map = FAVOURITE_MAP.read().unwrap();
    let json = serde_json::to_string_pretty(&*map)
        .map_err(|e| format!("Failed to serialize favourite config: {}", e))?;
    fs::write(FAVOURITE_JSON, json)
        .map_err(|e| format!("Failed to write favourite config: {}", e))?;
    Ok(())
}

pub fn create_favourite_file() {
    if !file_exists(&FAVOURITE_JSON.to_string()) {
        // 确保 core 目录存在
        if let Some(parent) = std::path::Path::new(FAVOURITE_JSON).parent() {
            fs::create_dir_all(parent).expect(&format!("Failed to create directory: {:?}", parent));
        }
        let mut fd = fs::File::create(FAVOURITE_JSON.to_string()).expect(&format!(
            "Failed to create file: {}",
            FAVOURITE_JSON.to_string()
        ));
        fd.write_all(FAVOURITE_CONFIG_JSON_CONTENT.to_string().as_bytes())
            .expect(&format!(
                "Failed to write file: {}",
                FAVOURITE_JSON.to_string()
            ));
        fd.flush().expect(&format!(
            "Failed to flush file: {}",
            FAVOURITE_JSON.to_string()
        ));
    }
}

use crate::r#const::constant::{SEARCH_CONFIG_JSON_CONTENT, SEARCH_JSON};
use crate::utils::file_exists;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::RwLock;

/// 搜索配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    pub source: Vec<SearchSource>,
    pub extensions: Vec<String>,
}

/// 搜索源配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSource {
    pub urls: Vec<String>,
    pub include_files: Vec<String>,
    pub parse_type: String,
}

impl SearchConfig {
    pub fn new() -> Self {
        SearchConfig {
            source: Vec::new(),
            extensions: Vec::new(),
        }
    }
}

static SEARCH_MAP: Lazy<RwLock<SearchConfig>> = Lazy::new(|| {
    let p = Path::new(format!("{}", SEARCH_JSON).as_mut_str()).to_owned();
    RwLock::new(read_search_json(p))
});

pub fn get_search_config() -> SearchConfig {
    SEARCH_MAP.read().unwrap().clone()
}

/// 重新加载 search.json 文件
pub fn reload_search_map() -> Result<(), String> {
    let p = Path::new(format!("{}", SEARCH_JSON).as_mut_str()).to_owned();
    let new_map = read_search_json(&p);
    let mut map = SEARCH_MAP.write().unwrap();
    *map = new_map;
    Ok(())
}

/// 获取所有搜索源列表
pub fn get_search_sources() -> Vec<SearchSource> {
    let config = get_search_config();
    config.source
}

/// 获取支持的文件扩展名列表
pub fn get_supported_extensions() -> Vec<String> {
    let config = get_search_config();
    config.extensions
}

/// 根据解析类型获取搜索源
pub fn get_sources_by_parse_type(parse_type: &str) -> Vec<SearchSource> {
    let config = get_search_config();
    config
        .source
        .into_iter()
        .filter(|s| s.parse_type == parse_type)
        .collect()
}

/// 获取所有 URL 列表（扁平化）
pub fn get_all_urls() -> Vec<String> {
    let config = get_search_config();
    config
        .source
        .into_iter()
        .flat_map(|s| s.urls)
        .collect()
}

fn read_search_json<P: AsRef<Path>>(path: P) -> SearchConfig {
    match fs::read_to_string(&path) {
        Ok(s) => {
            if s.trim().is_empty() {
                eprintln!("search: file {:?} is empty", path.as_ref());
                return SearchConfig::new();
            }
            match serde_json::from_str::<SearchConfig>(&s) {
                Ok(m) => {
                    eprintln!(
                        "search: successfully loaded {} sources from {:?}",
                        m.source.len(),
                        path.as_ref()
                    );
                    m
                }
                Err(e) => {
                    eprintln!(
                        "search: failed to parse JSON from {:?}: {}",
                        path.as_ref(),
                        e
                    );
                    eprintln!("search: file content: {}", s);
                    SearchConfig::new()
                }
            }
        }
        Err(e) => {
            eprintln!("search: failed to read {:?}: {}", path.as_ref(), e);
            SearchConfig::new()
        }
    }
}

pub fn create_search_file() {
    if !file_exists(&SEARCH_JSON.to_string()) {
        // 确保 core 目录存在
        if let Some(parent) = std::path::Path::new(SEARCH_JSON).parent() {
            fs::create_dir_all(parent).expect(&format!("Failed to create directory: {:?}", parent));
        }
        let mut fd = fs::File::create(SEARCH_JSON.to_string()).expect(&format!(
            "Failed to create file: {}",
            SEARCH_JSON.to_string()
        ));
        fd.write_all(SEARCH_CONFIG_JSON_CONTENT.to_string().as_bytes())
            .expect(&format!(
                "Failed to write file: {}",
                SEARCH_JSON.to_string()
            ));
        fd.flush().expect(&format!(
            "Failed to flush file: {}",
            SEARCH_JSON.to_string()
        ));
    }
}

/// 更新配置（通过闭包修改）
// pub fn update_config<F>(f: F) -> Result<(), Error>
// where
//     F: FnOnce(&mut SearchConfig),
// {
//     let mut config = get_config();
//     f(&mut config);
//     task::file_config::update_config(config)?;
//     task::file_config::save_config()
// }
// 
// /// 从文件重新加载配置
// pub fn init_data_from_file() -> Result<(), Error> {
//     let config = task::file_config::parse_core_json(
//         &crate::r#const::constant::TASK_JSON,
//     )?;
//     task::file_config::update_config(config)
// }

/// 更新搜索配置
pub fn update_search_config(config: SearchConfig) -> Result<(), String> {
    let mut map = SEARCH_MAP.write().unwrap();
    *map = config;
    drop(map);
    save_search_to_file()
}

/// 保存搜索配置到文件
pub fn save_search_to_file() -> Result<(), String> {
    let map = SEARCH_MAP.read().unwrap();
    let json = serde_json::to_string_pretty(&*map)
        .map_err(|e| format!("Failed to serialize search config: {}", e))?;
    fs::write(SEARCH_JSON, json)
        .map_err(|e| format!("Failed to write search config: {}", e))?;
    Ok(())
}

/// 添加搜索源
pub fn add_search_source(source: SearchSource) -> Result<(), String> {
    let mut map = SEARCH_MAP.write().unwrap();
    map.source.push(source);
    drop(map);
    save_search_to_file()
}

/// 删除搜索源（根据索引）
pub fn remove_search_source(index: usize) -> Result<(), String> {
    let mut map = SEARCH_MAP.write().unwrap();
    if index < map.source.len() {
        map.source.remove(index);
        drop(map);
        save_search_to_file()
    } else {
        Err(format!("Index {} out of bounds", index))
    }
}
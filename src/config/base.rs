use crate::r#const::constant::{BASE_CONFIG_JSON_CONTENT, BASE_JSON};
use crate::utils::file_exists;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::RwLock;

/// Base 配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseConfig {
    pub host: String,
    pub replace_string: bool,
    pub remote_url2local_images: bool,
}

impl BaseConfig {
    fn new() -> Self {
        BaseConfig {
            host: String::default(),
            replace_string: false,
            remote_url2local_images: false,
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

/// 部分更新 Base 配置（host、replace_string、remote_url2local_images）
pub fn partial_update_base_config(
    host: String,
    replace_string: bool,
    remote_url2local_images: bool,
) -> Result<(), String> {
    let mut config = get_base_config();
    config.host = host;
    config.replace_string = replace_string;
    config.remote_url2local_images = remote_url2local_images;
    update_base_config(config)
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
                eprintln!("base: file {:?} is empty", path.as_ref());
                return BaseConfig::new();
            }
            match serde_json::from_str::<BaseConfig>(&s) {
                Ok(m) => {
                    eprintln!(
                        "base: successfully loaded from {:?}",
                        path.as_ref()
                    );
                    m
                }
                Err(e) => {
                    eprintln!(
                        "base: failed to parse JSON from {:?}: {}",
                        path.as_ref(),
                        e
                    );
                    eprintln!("base: file content: {}", s);
                    BaseConfig::new()
                }
            }
        }
        Err(e) => {
            eprintln!("base: failed to read {:?}: {}", path.as_ref(), e);
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
    ) {
        eprintln!("sync_host_from_logos: failed to sync host to base.json: {}", e);
    } else {
        eprintln!("sync_host_from_logos: synced host from logos.json to base.json");
    }
}

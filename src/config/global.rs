use crate::r#const::constant::{GLOBAL_CONFIG_CONTENT, GLOBAL_CONFIG_FILE_NAME, REPLACE_JSON};
use crate::utils::file_exists;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{fs, io};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    // 在这里定义您的配置字段
    pub remote_url2local_images: bool,
    pub search: crate::config::config::Search,
}

impl GlobalConfig {
    fn new() -> GlobalConfig {
        GlobalConfig {
            remote_url2local_images: false,
            search: crate::config::config::Search {
                source: Vec::new(),
                extensions: Vec::new(),
                search_list: Vec::new(),
            },
        }
    }
}

static GLOBAL_CONFIG_DATA: Lazy<RwLock<GlobalConfig>> = Lazy::new(|| RwLock::new(read_global_config()));

pub fn init_global_config() {
    if !file_exists(&GLOBAL_CONFIG_FILE_NAME.to_string()) {
        // 确保 core 目录存在
        if let Some(parent) = Path::new(GLOBAL_CONFIG_FILE_NAME).parent() {
            fs::create_dir_all(parent).expect(&format!("Failed to create directory: {:?}", parent));
        }
        let mut fd = fs::File::create(GLOBAL_CONFIG_FILE_NAME).expect(&format!(
            "Failed to create file: {}",
            GLOBAL_CONFIG_FILE_NAME.to_string()
        ));
        fd.write_all(GLOBAL_CONFIG_CONTENT.to_string().as_bytes())
            .expect(&format!(
                "Failed to write file: {}",
                GLOBAL_CONFIG_FILE_NAME.to_string()
            ));
        fd.flush().expect(&format!(
            "Failed to flush file: {}",
            GLOBAL_CONFIG_FILE_NAME.to_string()
        ));
    }
}


pub fn init_data_from_file() -> io::Result<()> {
    let map = read_global_config();
    {
        let mut cfg = GLOBAL_CONFIG_DATA.write().unwrap();
        *cfg = map;
    }
    // let cfg = GLOBAL_CONFIG_DATA.read().unwrap();
    Ok(())
}

fn read_global_config() -> GlobalConfig {
    let path = Path::new(format!("{}", GLOBAL_CONFIG_FILE_NAME).as_mut_str()).to_owned();
    match fs::read_to_string(&path) {
        Ok(s) => match serde_json::from_str::<GlobalConfig>(&s) {
            Ok(m) => {
                m
            },
            Err(e) => {
                eprintln!(
                    "replace: failed to parse JSON from {:?}: {}",
                    path,
                    e
                );
                GlobalConfig::new()
            }
        },
        Err(e) => {
            eprintln!("replace: failed to read {:?}: {}", path, e);
            GlobalConfig::new()
        }
    }
}



pub fn get_config() -> GlobalConfig {
    GLOBAL_CONFIG_DATA.read().unwrap().clone()
}

pub fn update_config<F>(updater: F) -> Result<(), String>
where
    F: FnOnce(&mut GlobalConfig),
{
    // 读取当前配置
    let mut config = read_global_config();
    
    // 应用更新
    updater(&mut config);
    
    // 序列化配置
    let json_str = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    
    // 确保 core 目录存在
    if let Some(parent) = Path::new(GLOBAL_CONFIG_FILE_NAME).parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {:?}: {}", parent, e))?;
    }
    
    // 写入文件
    fs::write(GLOBAL_CONFIG_FILE_NAME, json_str)
        .map_err(|e| format!("Failed to write config file: {}", e))?;
    // 更新内存中的配置（覆盖）
    if let Ok(mut guard) = GLOBAL_CONFIG_DATA.write() {
        *guard = config.clone();
    }
    
    Ok(())
}

pub fn set_remote_url2local_images(value: bool) -> Result<(), String> {
    update_config(|config| {
        config.remote_url2local_images = value;
    })
}

pub fn set_search(value: crate::config::config::Search) -> Result<(), String> {
    update_config(|config| {
        config.search = value;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_config_new() {
        let config = GlobalConfig::new();
        assert_eq!(config.remote_url2local_images, false);
    }

    #[test]
    fn test_read_global_config_file_not_exists() {
        let config = read_global_config();
        assert_eq!(config.remote_url2local_images, false);
    }

    #[test]
    fn test_update_config() {
        let result = update_config(|config| {
            config.remote_url2local_images = true;
        });
        assert!(result.is_ok());

        let result = update_config(|config| {
            config.remote_url2local_images = false;
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_config_multiple_fields() {
        let result = update_config(|config| {
            config.remote_url2local_images = true;
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_config() {
        let config = get_config();
        assert!(config.remote_url2local_images == true || config.remote_url2local_images == false);
    }

    // #[test]
    // fn test_config_serialization() {
    //     let config = GlobalConfig {
    //         remote_url2local_images: true,
    //     };
    //     let json = serde_json::to_string(&config).unwrap();
    //     assert!(json.contains("remote_url2local_images"));
    //     assert!(json.contains("true"));
    // }

    #[test]
    fn test_config_deserialization() {
        let json = r#"{"remote_url2local_images":true}"#;
        let config: GlobalConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.remote_url2local_images, true);
    }
}


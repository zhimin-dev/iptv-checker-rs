use crate::r#const::constant::{GROUP_MAPPING_CONFIG_JSON_CONTENT, GROUP_MAPPING_JSON};
use crate::utils::file_exists;
use log::error;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMappingConfig {
    #[serde(default)]
    pub groups: Vec<String>,
    /// tvg-name → group-title
    #[serde(default)]
    pub mapping: HashMap<String, String>,
}

impl GroupMappingConfig {
    fn new() -> Self {
        GroupMappingConfig {
            groups: Vec::new(),
            mapping: HashMap::new(),
        }
    }
}

static GROUP_MAP: Lazy<RwLock<GroupMappingConfig>> = Lazy::new(|| {
    let p = Path::new(get_group_mapping_file_path().as_str()).to_owned();
    RwLock::new(read_group_mapping_json(&p))
});

pub fn get_group_mapping_config() -> GroupMappingConfig {
    GROUP_MAP.read().unwrap().clone()
}

/// Get the group-title for a given tvg-name, if mapped
pub fn get_group_for_channel(tv_name: &str) -> Option<String> {
    let config = GROUP_MAP.read().unwrap();
    config.mapping.get(tv_name).cloned()
}

/// Get all mappings as a HashMap
pub fn get_group_mapping_map() -> HashMap<String, String> {
    GROUP_MAP.read().unwrap().mapping.clone()
}

/// Get all groups
pub fn get_groups() -> Vec<String> {
    GROUP_MAP.read().unwrap().groups.clone()
}

pub fn get_group_mapping_json() -> Result<String, String> {
    let config = GROUP_MAP.read().unwrap();
    serde_json::to_string_pretty(&*config)
        .map_err(|e| format!("Failed to serialize group mapping: {}", e))
}

pub fn update_group_mapping(mapping: HashMap<String, String>) -> Result<(), String> {
    let mut config = GROUP_MAP.write().unwrap();
    config.mapping = mapping;
    drop(config);
    save_group_mapping_to_file()
}

pub fn save_full_config(groups: Vec<String>, mapping: HashMap<String, String>) -> Result<(), String> {
    let mut config = GROUP_MAP.write().unwrap();
    config.groups = groups;
    config.mapping = mapping;
    drop(config);
    save_group_mapping_to_file()
}

pub fn set_group_mapping(tv_name: String, group_title: String) -> Result<(), String> {
    let mut config = GROUP_MAP.write().unwrap();
    // Ensure group exists in groups list
    if !config.groups.contains(&group_title) {
        config.groups.push(group_title.clone());
    }
    config.mapping.insert(tv_name, group_title);
    drop(config);
    save_group_mapping_to_file()
}

pub fn remove_group_mapping(tv_name: &str) -> Result<(), String> {
    let mut config = GROUP_MAP.write().unwrap();
    config.mapping.remove(tv_name);
    drop(config);
    save_group_mapping_to_file()
}

pub fn add_group(group_title: String) -> Result<(), String> {
    let mut config = GROUP_MAP.write().unwrap();
    if !config.groups.contains(&group_title) {
        config.groups.push(group_title);
    }
    drop(config);
    save_group_mapping_to_file()
}

pub fn delete_group(group_title: &str) -> Result<(), String> {
    let mut config = GROUP_MAP.write().unwrap();
    config.groups.retain(|g| g != group_title);
    config.mapping.retain(|_, v| v != group_title);
    drop(config);
    save_group_mapping_to_file()
}

pub fn reload_group_mapping() -> Result<(), String> {
    let p = Path::new(get_group_mapping_file_path().as_str()).to_owned();
    let new_map = read_group_mapping_json(&p);
    let mut map = GROUP_MAP.write().unwrap();
    *map = new_map;
    Ok(())
}

fn read_group_mapping_json<P: AsRef<Path>>(path: P) -> GroupMappingConfig {
    match fs::read_to_string(&path) {
        Ok(s) => {
            if s.trim().is_empty() {
                return GroupMappingConfig::new();
            }
            match serde_json::from_str::<GroupMappingConfig>(&s) {
                Ok(m) => m,
                Err(e) => {
                    error!("group_mapping: failed to parse JSON from {:?}: {}", path.as_ref(), e);
                    GroupMappingConfig::new()
                }
            }
        }
        Err(_) => GroupMappingConfig::new(),
    }
}

pub fn save_group_mapping_to_file() -> Result<(), String> {
    let map = GROUP_MAP.read().unwrap();
    let json = serde_json::to_string_pretty(&*map)
        .map_err(|e| format!("Failed to serialize group mapping: {}", e))?;
    fs::write(get_group_mapping_file_path(), json)
        .map_err(|e| format!("Failed to write group mapping: {}", e))?;
    Ok(())
}

pub fn get_group_mapping_file_path() -> String {
    format!("./{}", GROUP_MAPPING_JSON)
}

pub fn create_group_mapping_file() {
    if !file_exists(&get_group_mapping_file_path()) {
        if let Some(parent) = std::path::Path::new(get_group_mapping_file_path().as_str()).parent() {
            fs::create_dir_all(parent)
                .expect(&format!("Failed to create directory: {:?}", parent));
        }
        let mut fd = fs::File::create(get_group_mapping_file_path())
            .expect(&format!("Failed to create file: {}", get_group_mapping_file_path()));
        fd.write_all(GROUP_MAPPING_CONFIG_JSON_CONTENT.to_string().as_bytes())
            .expect(&format!("Failed to write file: {}", get_group_mapping_file_path()));
        fd.flush()
            .expect(&format!("Failed to flush file: {}", get_group_mapping_file_path()));
    }
}

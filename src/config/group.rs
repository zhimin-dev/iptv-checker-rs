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

/// 单个分组类型的数据：分组列表 + tvg-name -> 分组 映射
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GroupTypeConfig {
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub mapping: HashMap<String, String>,
}

/// 支持多分组类型的分组映射配置：
/// - active: 当前生效的分组类型（prefix = 前缀/地域分组，category = 电视分类分组）
/// - types: 各类型独立的分组列表与映射（互不干扰，可随时切换）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMappingConfig {
    /// 兼容旧数据：仅一级分组列表（迁移到 types["prefix"]）
    #[serde(default)]
    pub groups: Vec<String>,
    /// 兼容旧数据：tvg-name -> group-title（迁移到 types["prefix"]）
    #[serde(default)]
    pub mapping: HashMap<String, String>,
    #[serde(default = "default_active_group_type")]
    pub active: String,
    #[serde(default)]
    pub types: HashMap<String, GroupTypeConfig>,
}

fn default_active_group_type() -> String {
    "prefix".to_string()
}

pub const GROUP_TYPE_PREFIX: &str = "prefix";
pub const GROUP_TYPE_CATEGORY: &str = "category";

impl GroupMappingConfig {
    fn new() -> Self {
        let mut cfg = GroupMappingConfig {
            groups: Vec::new(),
            mapping: HashMap::new(),
            active: default_active_group_type(),
            types: HashMap::new(),
        };
        cfg.types.insert(GROUP_TYPE_PREFIX.to_string(), GroupTypeConfig::default());
        cfg.types.insert(GROUP_TYPE_CATEGORY.to_string(), GroupTypeConfig::default());
        cfg
    }

    /// 兼容旧数据：老文件只有 groups/mapping，迁移到 prefix 类型；确保两种类型都存在
    fn migrate(&mut self) {
        if self.types.is_empty()
            && (!self.groups.is_empty() || !self.mapping.is_empty())
        {
            self.types.insert(
                GROUP_TYPE_PREFIX.to_string(),
                GroupTypeConfig {
                    groups: std::mem::take(&mut self.groups),
                    mapping: std::mem::take(&mut self.mapping),
                },
            );
        }
        self.types
            .entry(GROUP_TYPE_PREFIX.to_string())
            .or_default();
        self.types
            .entry(GROUP_TYPE_CATEGORY.to_string())
            .or_default();
        if self.active != GROUP_TYPE_CATEGORY {
            self.active = GROUP_TYPE_PREFIX.to_string();
        }
    }

    fn active_config(&mut self) -> &mut GroupTypeConfig {
        self.types.entry(self.active.clone()).or_default()
    }
}

static GROUP_MAP: Lazy<RwLock<GroupMappingConfig>> = Lazy::new(|| {
    let p = Path::new(get_group_mapping_file_path().as_str()).to_owned();
    RwLock::new(read_group_mapping_json(&p))
});

pub fn get_group_mapping_config() -> GroupMappingConfig {
    GROUP_MAP.read().unwrap().clone()
}

/// 当前生效的分组类型：prefix | category
pub fn get_active_group_type() -> String {
    GROUP_MAP.read().unwrap().active.clone()
}

/// 设置当前生效的分组类型
pub fn set_active_group_type(active: &str) -> Result<(), String> {
    let t = if active.trim() == GROUP_TYPE_CATEGORY {
        GROUP_TYPE_CATEGORY.to_string()
    } else {
        GROUP_TYPE_PREFIX.to_string()
    };
    {
        let mut config = GROUP_MAP.write().unwrap();
        config.active = t;
    }
    save_group_mapping_to_file()
}

/// Get the group-title for a given tvg-name, if mapped（当前类型）
pub fn get_group_for_channel(tv_name: &str) -> Option<String> {
    let config = GROUP_MAP.read().unwrap();
    config.types.get(&config.active)?.mapping.get(tv_name).cloned()
}

/// Get all mappings as a HashMap（当前类型）
pub fn get_group_mapping_map() -> HashMap<String, String> {
    let config = GROUP_MAP.read().unwrap();
    config
        .types
        .get(&config.active)
        .map(|t| t.mapping.clone())
        .unwrap_or_default()
}

/// Get all groups（当前类型）
pub fn get_groups() -> Vec<String> {
    let config = GROUP_MAP.read().unwrap();
    config
        .types
        .get(&config.active)
        .map(|t| t.groups.clone())
        .unwrap_or_default()
}

pub fn get_group_mapping_json() -> Result<String, String> {
    let config = GROUP_MAP.read().unwrap();
    serde_json::to_string_pretty(&*config)
        .map_err(|e| format!("Failed to serialize group mapping: {}", e))
}

pub fn update_group_mapping(mapping: HashMap<String, String>) -> Result<(), String> {
    {
        let mut config = GROUP_MAP.write().unwrap();
        config.active_config().mapping = mapping;
    }
    save_group_mapping_to_file()
}

pub fn save_full_config(groups: Vec<String>, mapping: HashMap<String, String>) -> Result<(), String> {
    {
        let mut config = GROUP_MAP.write().unwrap();
        let active = config.active_config();
        active.groups = groups;
        active.mapping = mapping;
    }
    save_group_mapping_to_file()
}

pub fn set_group_mapping(tv_name: String, group_title: String) -> Result<(), String> {
    {
        let mut config = GROUP_MAP.write().unwrap();
        let active = config.active_config();
        // Ensure group exists in groups list
        if !active.groups.contains(&group_title) {
            active.groups.push(group_title.clone());
        }
        active.mapping.insert(tv_name, group_title);
    }
    save_group_mapping_to_file()
}

pub fn remove_group_mapping(tv_name: &str) -> Result<(), String> {
    {
        let mut config = GROUP_MAP.write().unwrap();
        config.active_config().mapping.remove(tv_name);
    }
    save_group_mapping_to_file()
}

pub fn add_group(group_title: String) -> Result<(), String> {
    {
        let mut config = GROUP_MAP.write().unwrap();
        let active = config.active_config();
        if !active.groups.contains(&group_title) {
            active.groups.push(group_title);
        }
    }
    save_group_mapping_to_file()
}

pub fn delete_group(group_title: &str) -> Result<(), String> {
    {
        let mut config = GROUP_MAP.write().unwrap();
        let active = config.active_config();
        active.groups.retain(|g| g != group_title);
        active.mapping.retain(|_, v| v != group_title);
    }
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
                Ok(mut m) => {
                    m.migrate();
                    m
                }
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

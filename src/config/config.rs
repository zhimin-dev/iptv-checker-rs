use std::collections::HashMap;
use std::fs;
use std::io::{Error, ErrorKind};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::path::Path;
use log::{debug, error, info};
use uuid::Uuid;
use crate::common::do_check;
use crate::common::task::Task;
use crate::config::{parse_core_json, update_config};

/// 核心配置结构体
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Core {
    pub check: Check,
    pub ob: Ob,
    pub search: Search,
}

/// 检查相关配置
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Check {
    pub now: Option<String>,
    pub task: HashMap<String, Task>,
}

/// 转播相关配置
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Ob {
    pub list: Vec<ObItem>,
}

/// 转播项
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ObItem {
    pub id: String,
    pub pid: i32,
    pub name: String,
    pub create_time: u64,
    pub url: String,
}

/// 搜索相关配置
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Search {
    pub source: Vec<SearchSource>,
    pub extensions: Vec<String>,
    pub search_list: Vec<SearchListItem>,
}

/// 搜索源配置
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SearchSource {
    pub urls: Vec<String>,
    pub include_files: Vec<String>,
    pub parse_type: String,
}

/// 搜索列表项
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SearchListItem {
    pub id: String,
    pub config: Vec<SearchConfig>,
    pub result: String,
}

/// 搜索配置项
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SearchConfig {
    pub search_name: Vec<String>,
    pub save_name: String,
    pub full_match: bool,
    pub exclude_url: Vec<String>,
    pub exclude_host: Vec<String>,
}

/// 全局配置
lazy_static::lazy_static! {
    static ref GLOBAL_CONFIG: Mutex<Core> = Mutex::new(Core::default());
}

impl Default for Core {
    fn default() -> Self {
        Core {
            check: Check {
                now: None,
                task: HashMap::new(),
            },
            ob: Ob {
                list: Vec::new(),
            },
            search: Search {
                source: Vec::new(),
                extensions: Vec::new(),
                search_list: Vec::new(),
            },
        }
    }
}

pub fn init_config() {
    // Initialize config
    let config_data = match parse_core_json("core.json") {
        Ok(cfg) => {
            info!("Successfully loaded core.json");
            cfg
        }
        Err(e) => {
            error!("Failed to load core.json: {}", e);
            // 使用默认配置
            Core::default()
        }
    };
    // Update global config
    if let Err(e) = update_config(config_data) {
        error!("Failed to update global config: {}", e);
    }
}

/// 配置管理模块
pub mod file_config {
    use super::*;

    /// 从文件解析配置
    pub fn parse_core_json(file_path: &str) -> Result<Core, Error> {
        let content = fs::read_to_string(file_path)?;
        let config: Core = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// 从字符串解析配置
    pub fn read_config(content: String) -> Result<Core, Error> {
        Ok(serde_json::from_str(&content)?)
    }

    /// 保存配置到文件
    pub fn save_config(file_path: &str) -> Result<(), Error> {
        let config = GLOBAL_CONFIG.lock().unwrap();
        let content = serde_json::to_string_pretty(&*config)?;
        fs::write(file_path, content)?;
        Ok(())
    }

    /// 更新整个配置
    pub fn update_config(new_config: Core) -> Result<(), Error> {
        let mut config = GLOBAL_CONFIG.lock().unwrap();
        *config = new_config;
        Ok(())
    }

    /// 更新检查配置
    pub fn update_check(check: Check) -> Result<(), Error> {
        let mut config = GLOBAL_CONFIG.lock().unwrap();
        config.check = check;
        Ok(())
    }

    /// 更新转播配置
    pub fn update_ob(ob: Ob) -> Result<(), Error> {
        let mut config = GLOBAL_CONFIG.lock().unwrap();
        config.ob = ob;
        Ok(())
    }

    /// 更新搜索配置
    pub fn update_search(search: Search) -> Result<(), Error> {
        let mut config = GLOBAL_CONFIG.lock().unwrap();
        config.search = search;
        Ok(())
    }

    /// 添加或更新任务
    pub fn save_task(id: String, task: Task) -> Result<(), Error> {
        let mut config = GLOBAL_CONFIG.lock().unwrap();
        config.check.task.insert(id, task);
        Ok(())
    }

    /// 删除任务
    pub fn delete_task(id: &str) -> Result<(), Error> {
        let mut config = GLOBAL_CONFIG.lock().unwrap();
        config.check.task.remove(id);
        Ok(())
    }

    /// 获取整个配置
    pub fn get_config() -> Result<Core, Error> {
        let config = GLOBAL_CONFIG.lock().unwrap();
        Ok(config.clone())
    }

    /// 获取检查配置
    pub fn get_check() -> Result<Check, Error> {
        let config = GLOBAL_CONFIG.lock().unwrap();
        Ok(config.check.clone())
    }

    /// 获取转播配置
    pub fn get_ob() -> Result<Ob, Error> {
        let config = GLOBAL_CONFIG.lock().unwrap();
        Ok(config.ob.clone())
    }

    /// 获取搜索配置
    pub fn get_search() -> Result<Search, Error> {
        let config = GLOBAL_CONFIG.lock().unwrap();
        Ok(config.search.clone())
    }

    /// 获取特定任务
    pub fn get_task(id: &str) -> Result<Option<Task>, Error> {
        let config = GLOBAL_CONFIG.lock().unwrap();
        Ok(config.check.task.get(id).cloned())
    }

    /// 获取所有任务
    pub fn get_all_tasks() -> Result<HashMap<String, Task>, Error> {
        let config = GLOBAL_CONFIG.lock().unwrap();
        Ok(config.check.task.clone())
    }

    /// 获取当前运行的任务ID
    pub fn get_now_task() -> Result<Option<String>, Error> {
        let config = GLOBAL_CONFIG.lock().unwrap();
        Ok(config.check.now.clone())
    }

    /// 设置当前运行的任务ID
    pub fn set_now_task(task_id: Option<String>) -> Result<(), Error> {
        let mut config = GLOBAL_CONFIG.lock().unwrap();
        config.check.now = task_id;
        Ok(())
    }
}

/*
// 从文件加载配置
let config = config::parse_core_json("core.json")?;

// 更新配置
config::update_config(config)?;

// 管理任务
config::save_task("task_id".to_string(), task)?;
config::delete_task("task_id")?;
let task = config::get_task("task_id")?;
let all_tasks = config::get_all_tasks()?;

// 管理当前任务
config::set_now_task(Some("task_id".to_string()))?;
let current_task = config::get_now_task()?;

// 保存配置到文件
config::save_config("core.json")?;
 */
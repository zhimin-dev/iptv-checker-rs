use crate::common::task::Task;
use crate::config::{parse_core_json, update_config};
use crate::r#const::constant::{CORE_DATA, CORE_JSON};
use crate::utils::file_exists;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Error, Write};
use std::sync::Mutex;

/// 核心配置结构体，包含所有配置项
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Core {
    pub check: Check,   // 检查相关配置
    pub ob: Ob,         // 转播相关配置
    pub search: Search, // 搜索相关配置
    pub others: Others, // 其他配置
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Others {
    pub translate_dic: String,
    pub replace_dic: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ReplaceChar {
    pub name: String,
    pub replace: String,
}

/// 检查相关配置结构体
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Check {
    pub now: Option<String>,         // 当前运行的任务ID
    pub task: HashMap<String, Task>, // 任务列表
}

/// 转播相关配置结构体
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Ob {
    pub list: Vec<ObItem>, // 转播列表
}

/// 转播项结构体
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ObItem {
    pub id: String,       // 转播ID
    pub pid: i32,         // 进程ID
    pub name: String,     // 转播名称
    pub create_time: u64, // 创建时间
    pub url: String,      // 转播URL
}

/// 搜索相关配置结构体
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Search {
    pub source: Vec<SearchSource>,        // 搜索源列表
    pub extensions: Vec<String>,          // 文件扩展名列表
    pub search_list: Vec<SearchListItem>, // 搜索列表
}

/// 搜索源配置结构体
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SearchSource {
    pub urls: Vec<String>,          // 源URL列表
    pub include_files: Vec<String>, // 包含的文件列表
    pub parse_type: String,         // 解析类型
}

/// 搜索列表项结构体
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SearchListItem {
    pub id: String,                // 列表项ID
    pub config: Vec<SearchConfig>, // 搜索配置列表
    pub result: String,            // 搜索结果
}

/// 搜索配置项结构体
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SearchConfig {
    pub search_name: Vec<String>,  // 搜索名称列表
    pub save_name: String,         // 保存名称
    pub full_match: bool,          // 是否完全匹配
    pub exclude_url: Vec<String>,  // 排除的URL列表
    pub exclude_host: Vec<String>, // 排除的主机列表
}

lazy_static::lazy_static! {
    static ref GLOBAL_CONFIG: Mutex<Core> = Mutex::new(Core::default());
}

/// 为Core实现Default trait
impl Default for Core {
    fn default() -> Self {
        Core {
            check: Check {
                now: None,
                task: HashMap::new(),
            },
            others: Others {
                translate_dic: String::default(),
                replace_dic: String::default(),
            },
            ob: Ob { list: Vec::new() },
            search: Search {
                source: Vec::new(),
                extensions: Vec::new(),
                search_list: Vec::new(),
            },
        }
    }
}

pub fn create_config_file() {
    if !file_exists(&CORE_JSON.to_string()) {
        // 确保 core 目录存在
        if let Some(parent) = std::path::Path::new(CORE_JSON).parent() {
            fs::create_dir_all(parent).expect(&format!("Failed to create directory: {:?}", parent));
        }
        let mut fd = fs::File::create(CORE_JSON)
            .expect(&format!("Failed to create file: {}", CORE_JSON.to_string()));
        fd.write(CORE_DATA.to_string().as_bytes())
            .expect(&format!("Failed to write file: {}", CORE_JSON.to_string()));
        fd.flush()
            .expect(&format!("Failed to flush file: {}", CORE_JSON.to_string()));
    }
}

/// 初始化配置
pub fn init_config() {
    create_config_file();
    // 尝试从文件加载配置
    let config_data = match parse_core_json(CORE_JSON) {
        Ok(cfg) => {
            info!("Successfully loaded {}", CORE_JSON);
            cfg
        }
        Err(e) => {
            error!("Failed to load {}: {}", CORE_JSON, e);
            // 加载失败时使用默认配置
            Core::default()
        }
    };
    // 更新全局配置
    if let Err(e) = update_config(config_data) {
        error!("Failed to update global config: {}", e);
    }
}

/// 配置管理模块
pub mod file_config {
    use super::*;
    use crate::r#const::constant::CORE_JSON;

    /// 从文件解析配置
    pub fn parse_core_json(file_path: &str) -> Result<Core, Error> {
        let content = fs::read_to_string(file_path)?;
        let config: Core = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// 从字符串解析配置
    // pub fn read_config(content: String) -> Result<Core, Error> {
    //     Ok(serde_json::from_str(&content)?)
    // }

    /// 保存配置到文件
    pub fn save_config() -> Result<(), Error> {
        let config = GLOBAL_CONFIG.lock().unwrap();
        let content = serde_json::to_string_pretty(&*config)?;
        fs::write(CORE_JSON, content)?;
        Ok(())
    }

    /// 更新整个配置
    pub fn update_config(new_config: Core) -> Result<(), Error> {
        let mut config = GLOBAL_CONFIG.lock().unwrap();
        *config = new_config;
        Ok(())
    }

    /// 更新检查配置
    // pub fn update_check(check: Check) -> Result<(), Error> {
    //     let mut config = GLOBAL_CONFIG.lock().unwrap();
    //     config.check = check;
    //     Ok(())
    // }

    /// 更新转播配置
    // pub fn update_ob(ob: Ob) -> Result<(), Error> {
    //     let mut config = GLOBAL_CONFIG.lock().unwrap();
    //     config.ob = ob;
    //     Ok(())
    // }

    /// 更新搜索配置
    // pub fn update_search(search: Search) -> Result<(), Error> {
    //     let mut config = GLOBAL_CONFIG.lock().unwrap();
    //     config.search = search;
    //     Ok(())
    // }

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
    // pub fn get_config() -> Result<Core, Error> {
    //     let config = GLOBAL_CONFIG.lock().unwrap();
    //     Ok(config.clone())
    // }

    pub fn get_now_check_task_id() -> Option<String> {
        let config = GLOBAL_CONFIG.lock().unwrap();
        config.check.now.clone()
    }

    pub fn set_now_check_id(now: Option<String>) {
        let mut config = GLOBAL_CONFIG.lock().unwrap();
        config.check.now = now;
    }

    /// 获取检查配置
    pub fn get_check() -> Result<Check, Error> {
        let config = GLOBAL_CONFIG.lock().unwrap();
        Ok(config.check.clone())
    }

    pub fn get_others() -> Result<Others, Error> {
        let config = GLOBAL_CONFIG.lock().unwrap();

        Ok(config.others.clone())
    }

    /// 获取转播配置
    // pub fn get_ob() -> Result<Ob, Error> {
    //     let config = GLOBAL_CONFIG.lock().unwrap();
    //     Ok(config.ob.clone())
    // }

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

    // pub fn get_now_task() -> Result<Option<String>, Error> {
    //     let config = GLOBAL_CONFIG.lock().unwrap();
    //     Ok(config.check.now.clone())
    // }

    // pub fn set_now_task(task_id: Option<String>) -> Result<(), Error> {
    //     let mut config = GLOBAL_CONFIG.lock().unwrap();
    //     config.check.now = task_id;
    //     Ok(())
    // }
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

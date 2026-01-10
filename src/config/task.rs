use crate::common::task::Task;
use crate::r#const::constant::{TASK_DATA, TASK_JSON};
use crate::utils::file_exists;
use log::{error, info};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::RwLock;

/// 检查相关配置结构体
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskConfig {
    pub now: Option<String>,         // 当前运行的任务ID
    pub task: HashMap<String, Task>, // 任务列表
}

impl TaskConfig {
    pub fn new() -> Self {
        Self {
            now: None,
            task: HashMap::new(),
        }
    }
}

static TASK_MAP: Lazy<RwLock<TaskConfig>> = Lazy::new(|| {
    let p = Path::new(TASK_JSON);
    RwLock::new(read_task_json(p))
});

/// 读取任务配置文件
fn read_task_json<P: AsRef<Path>>(path: P) -> TaskConfig {
    match fs::read_to_string(&path) {
        Ok(s) => {
            if s.trim().is_empty() {
                eprintln!("task: file {:?} is empty", path.as_ref());
                return TaskConfig::new();
            }
            match serde_json::from_str::<TaskConfig>(&s) {
                Ok(m) => {
                    eprintln!(
                        "task: successfully loaded {} tasks from {:?}",
                        m.task.len(),
                        path.as_ref()
                    );
                    m
                }
                Err(e) => {
                    eprintln!(
                        "task: failed to parse JSON from {:?}: {}",
                        path.as_ref(),
                        e
                    );
                    eprintln!("task: file content: {}", s);
                    TaskConfig::new()
                }
            }
        }
        Err(e) => {
            eprintln!("task: failed to read {:?}: {}", path.as_ref(), e);
            TaskConfig::new()
        }
    }
}

/// 获取任务配置
pub fn get_task_config() -> TaskConfig {
    TASK_MAP.read().unwrap().clone()
}

/// 重新加载任务配置
pub fn reload_task_config() -> Result<(), String> {
    let p = Path::new(TASK_JSON);
    let new_config = read_task_json(p);
    let mut config = TASK_MAP.write().unwrap();
    *config = new_config;
    Ok(())
}

pub fn create_task_file() {
    if !file_exists(&TASK_JSON.to_string()) {
        // 确保 core 目录存在
        if let Some(parent) = std::path::Path::new(TASK_JSON).parent() {
            fs::create_dir_all(parent).expect(&format!("Failed to create directory: {:?}", parent));
        }
        let mut fd = fs::File::create(TASK_JSON)
            .expect(&format!("Failed to create file: {}", TASK_JSON.to_string()));
        fd.write(TASK_DATA.to_string().as_bytes())
            .expect(&format!("Failed to write file: {}", TASK_JSON.to_string()));
        fd.flush()
            .expect(&format!("Failed to flush file: {}", TASK_JSON.to_string()));
    }
}

/// 初始化配置
pub fn init_task_config() {
    create_task_file();
    // 重新加载配置（Lazy 会在首次访问时自动加载）
    if let Err(e) = reload_task_config() {
        error!("Failed to reload task config: {}", e);
    } else {
        info!("Successfully initialized task config from {}", TASK_JSON);
    }
}

/// 配置管理模块
pub mod file_config {
    use std::collections::HashMap;
    use std::fs;
    use crate::config::task::{TaskConfig, TASK_MAP};
    use crate::r#const::constant::TASK_JSON;
    use std::io::Error;
    use crate::common::task::Task;

    /// 从文件解析配置
    pub fn parse_task_json(file_path: &str) -> Result<TaskConfig, Error> {
        let content = fs::read_to_string(file_path)?;
        let config: TaskConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// 保存配置到文件
    pub fn save_task_config() -> Result<(), Error> {
        let config = TASK_MAP.read().unwrap();
        let content = serde_json::to_string_pretty(&*config)?;
        fs::write(TASK_JSON, content)?;
        Ok(())
    }

    /// 更新整个配置
    pub fn update_config(new_config: TaskConfig) -> Result<(), Error> {
        let mut config = TASK_MAP.write().unwrap();
        *config = new_config;
        Ok(())
    }

    /// 添加或更新任务
    pub fn save_task(id: String, task: Task) -> Result<(), Error> {
        let mut config = TASK_MAP.write().unwrap();
        config.task.insert(id, task);
        Ok(())
    }

    /// 删除任务
    pub fn delete_task(id: &str) -> Result<(), Error> {
        let mut config = TASK_MAP.write().unwrap();
        config.task.remove(id);
        Ok(())
    }

    pub fn get_now_check_task_id() -> Option<String> {
        let config = TASK_MAP.read().unwrap();
        config.now.clone()
    }

    pub fn set_now_check_id(now: Option<String>) {
        let mut config = TASK_MAP.write().unwrap();
        config.now = now;
    }

    /// 获取特定任务
    pub fn get_task(id: &str) -> Result<Option<Task>, Error> {
        let config = TASK_MAP.read().unwrap();
        Ok(config.task.get(id).cloned())
    }

    /// 获取所有任务
    pub fn get_all_tasks() -> Result<HashMap<String, Task>, Error> {
        let config = TASK_MAP.read().unwrap();
        Ok(config.task.clone())
    }
}

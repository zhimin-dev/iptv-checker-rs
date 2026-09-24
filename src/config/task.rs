use crate::common::task::Task;
use crate::r#const::constant::{TASK_DATA, TASK_JSON};
use crate::utils::file_exists;
use log::{error, info, warn};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, RwLock};

/// task.json 文件写锁：多个检查任务并发运行时，串行化整个配置文件的写入，
/// 防止两个线程同时写文件导致内容交错损坏。
static TASK_FILE_WRITE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

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
    /// 原子领取当前配置，旧调度快照不能覆盖用户的新配置或重新创建已删除任务。
    fn begin_task(&mut self, id: &str, started_at: i32) -> Option<Task> {
        let task = self.task.get_mut(id)?;
        if task.task_info.is_running {
            return None;
        }
        task.task_info.is_running = true;
        task.task_info.task_status = crate::common::task::TaskStatus::InProgress;
        task.task_info.last_run_time = started_at;
        self.now = Some(id.to_string());
        Some(task.clone())
    }

    fn reset_stale_task(&mut self, id: &str, observed_start: i32) {
        if let Some(task) = self.task.get_mut(id) {
            if task.task_info.is_running && task.task_info.last_run_time == observed_start {
                task.task_info.is_running = false;
                task.task_info.task_status = crate::common::task::TaskStatus::Pending;
                if self.now.as_deref() == Some(id) {
                    self.now = None;
                }
            }
        }
    }

    fn finish_task(&mut self, id: &str, finished_at: i32) {
        if self.now.as_deref() == Some(id) {
            self.now = None;
        }
        if let Some(task) = self.task.get_mut(id) {
            task.task_info.complete(finished_at);
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
                warn!("task: file {:?} is empty", path.as_ref());
                return TaskConfig::new();
            }
            match serde_json::from_str::<TaskConfig>(&s) {
                Ok(m) => {
                    error!(
                        "task: successfully loaded {} tasks from {:?}",
                        m.task.len(),
                        path.as_ref()
                    );
                    m
                }
                Err(e) => {
                    error!(
                        "task: failed to parse JSON from {:?}: {}",
                        path.as_ref(),
                        e
                    );
                    error!("task: file content: {}", s);
                    TaskConfig::new()
                }
            }
        }
        Err(e) => {
            error!("task: failed to read {:?}: {}", path.as_ref(), e);
            TaskConfig::new()
        }
    }
}

/// 重新加载任务配置
pub fn reload_task_config() -> Result<(), String> {
    let p = Path::new(TASK_JSON);
    let new_config = read_task_json(p);
    let mut config = TASK_MAP.write().unwrap();
    *config = new_config;
    Ok(())
}

pub fn save_task_to_file() -> Result<(), String> {
    let _guard = TASK_FILE_WRITE_LOCK
        .lock()
        .map_err(|e| format!("Failed to lock task file write: {}", e))?;
    let map = TASK_MAP.read().unwrap();
    let json = serde_json::to_string_pretty(&*map)
        .map_err(|e| format!("Failed to serialize search config: {}", e))?;
    fs::write(TASK_JSON, json)
        .map_err(|e| format!("Failed to write search config: {}", e))?;
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

/// 进程启动时调用：清理上一次进程退出时遗留的「运行中」状态。
/// 进程重启后不可能有任务仍在运行，若不清理，任务会一直显示「正在检查」且不再被调度。
/// 复位所有 is_running 任务为 Pending，并清空「当前运行任务」标记。
pub fn reset_running_tasks_on_startup() {
    let mut config = TASK_MAP.write().unwrap();
    let mut reset_count = 0;
    for (_id, task) in config.task.iter_mut() {
        if task.task_info.is_running {
            task.task_info.is_running = false;
            task.task_info.task_status = crate::common::task::TaskStatus::Pending;
            reset_count += 1;
        }
    }
    if config.now.is_some() {
        config.now = None;
    }
    drop(config);
    if reset_count > 0 {
        info!(
            "reset {} stale running task(s) on startup (previous process left them in progress)",
            reset_count
        );
        let _ = save_task_to_file();
    }
}

/// 配置管理模块
pub mod file_config {
    use std::collections::HashMap;
    use std::fs;
    use crate::config::task::TASK_MAP;
    use crate::r#const::constant::TASK_JSON;
    use std::io::Error;
    use crate::common::task::Task;

    /// 保存配置到文件
    pub fn save_task_config() -> Result<(), Error> {
        let _guard = super::TASK_FILE_WRITE_LOCK
            .lock()
            .map_err(|e| Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        let config = TASK_MAP.read().unwrap();
        let content = serde_json::to_string_pretty(&*config)?;
        fs::write(TASK_JSON, content)?;
        Ok(())
    }

    /// 添加或更新任务
    pub fn save_task(id: String, task: Task) -> Result<(), Error> {
        let mut config = TASK_MAP.write().unwrap();
        config.task.insert(id, task);
        Ok(())
    }

    pub fn begin_task(id: &str, started_at: i32) -> Option<Task> {
        TASK_MAP.write().unwrap().begin_task(id, started_at)
    }

    pub fn reset_stale_task(id: &str, observed_start: i32) {
        TASK_MAP
            .write()
            .unwrap()
            .reset_stale_task(id, observed_start);
    }

    pub fn finish_task(id: &str, finished_at: i32) {
        TASK_MAP.write().unwrap().finish_task(id, finished_at);
    }

    pub fn update_original(id: &str, original: crate::common::task::TaskContent) -> bool {
        let mut config = TASK_MAP.write().unwrap();
        if let Some(task) = config.task.get_mut(id) {
            task.set_original(original);
            true
        } else {
            false
        }
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

#[cfg(test)]
mod regression_tests {
    use super::*;
    use crate::common::task::{RunTime, TaskContent, TaskStatus};

    #[test]
    fn test_running_edit_preserves_config_and_latest_schedule() {
        let mut config = TaskConfig::new();
        let task = Task::new();
        let id = task.get_uuid();
        config.task.insert(id.clone(), task);
        assert!(config.begin_task(&id, 100).is_some());
        assert!(config.begin_task(&id, 101).is_none());
        let mut edited = TaskContent::new();
        edited.set_urls(vec!["new-source.m3u".into()]);
        edited.set_result_file_name("new-result".into());
        edited.set_run_type(RunTime::EveryHour);
        config.task.get_mut(&id).unwrap().set_original(edited);
        config.finish_task(&id, 200);
        let latest = config.task.get(&id).unwrap();
        assert_eq!(latest.original.get_result_name(), "new-result");
        assert_eq!(latest.original.get_urls(), vec!["new-source.m3u"]);
        assert!(!latest.task_info.is_running);
        assert_eq!(latest.task_info.task_status, TaskStatus::Pending);
        assert_eq!(latest.task_info.next_run_time, 3800);
    }

    #[test]
    fn test_stale_reset_preserves_edit_and_does_not_reset_new_run() {
        let mut config = TaskConfig::new();
        let task = Task::new();
        let id = task.get_uuid();
        config.task.insert(id.clone(), task);
        config.begin_task(&id, 100).unwrap();
        config
            .task
            .get_mut(&id)
            .unwrap()
            .original
            .set_result_file_name("edited".into());
        config.reset_stale_task(&id, 100);
        assert_eq!(config.task[&id].original.get_result_name(), "edited");
        config.begin_task(&id, 200).unwrap();
        config.reset_stale_task(&id, 100);
        assert!(config.task[&id].task_info.is_running);
    }

    #[test]
    fn test_deleted_task_is_not_resurrected_or_claimed() {
        let mut config = TaskConfig::new();
        let task = Task::new();
        let id = task.get_uuid();
        config.task.insert(id.clone(), task);
        config.begin_task(&id, 100).unwrap();
        config.task.remove(&id);
        config.now = Some("other-running-task".into());
        config.finish_task(&id, 200);
        assert!(config.task.is_empty());
        assert!(config.begin_task(&id, 300).is_none());
        assert_eq!(config.now.as_deref(), Some("other-running-task"));
    }
}

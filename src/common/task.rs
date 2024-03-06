use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write, Result};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use md5;
use std::time::{SystemTime, UNIX_EPOCH};
use clokwerk::{Scheduler, TimeUnits};

const FILE_PATH: &str = "tasks.json";

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskInfo {
    // 运行时间
    run_time: RunTime,

    // 最后一次运行时间
    last_run_time: i32,

    // 任务状态
    task_status: TaskStatus,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
enum RunTime {
    EveryDay,
    EveryHour,
}

impl TaskInfo {
    pub fn new() -> TaskInfo {
        return TaskInfo {
            run_time: RunTime::EveryDay,
            task_status: TaskStatus::Pending,
            last_run_time: 0,
        };
    }

    pub fn set_status(&mut self, stats: TaskStatus) {
        self.task_status = stats
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskContent {
    // 订阅源
    urls: Vec<String>,
    // 订阅内容
    contents: String,
    // 结果文件名，最后可以通过这个文件来获取结果
    result_name: String,
    // 最终的md5
    md5: String,
}

fn md5_str(input: String) -> String {
    let digest = md5::compute(input);

    format!("{:x}", digest)
}

impl TaskContent {
    pub fn new() -> TaskContent {
        TaskContent {
            urls: vec![],
            contents: "".to_string(),
            result_name: "".to_string(),
            md5: "".to_string(),
        }
    }

    pub fn gen_md5(&mut self) {
        self.md5 = String::from("");
        let json_string = serde_json::to_string(&self).unwrap();
        self.md5 = md5_str(json_string);
    }

    pub fn set_urls(&mut self, urls: Vec<String>) {
        self.urls = urls;
    }

    pub fn set_contents(&mut self, contents: String) {
        self.contents = contents
    }

    pub fn set_result_file_name(&mut self, name: String) {
        self.result_name = name
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
enum TaskStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Task {
    //任务来源
    original: TaskContent,

    //任务id
    id: String,

    //任务创建时间
    create_time: u64,

    //任务详情
    task_info: TaskInfo,
}

fn now() -> u64 {
    let now = SystemTime::now();
    return now.duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();
}

impl Task {
    pub fn new() -> Task {
        let id = Uuid::new_v4();
        Task {
            original: TaskContent::new(),
            id: id.to_string(),
            create_time: now(),
            task_info: TaskInfo::new(),
        }
    }

    pub fn set_original(&mut self, original: TaskContent) {
        self.original = original
    }

    pub fn get_uuid(&self) -> String {
        self.id.clone()
    }

    pub fn set_task_info(&mut self, task_info: TaskInfo) {
        self.task_info = task_info
    }
}

pub struct TaskManager {
    pub tasks: Mutex<HashMap<String, Task>>,
}

impl TaskManager {
    pub fn add_task(&self, task: TaskContent) -> Result<String> {
        let mut ori = TaskContent::new();
        if task.urls.len() > 0 {
            ori.set_urls(task.urls);
        } else if !task.contents.is_empty() {
            ori.set_contents(task.contents);
        }
        if !task.result_name.is_empty() {
            ori.set_result_file_name(task.result_name)
        }
        ori.gen_md5();
        let mut task = Task::new();
        task.set_original(ori);
        let mut tasks = self.tasks.lock().unwrap();
        tasks.insert(task.get_uuid(), task.clone());
        drop(tasks); // 显式释放锁以防止死锁
        self.save_tasks()?;
        Ok(task.get_uuid())
    }

    pub fn delete_task(&self, id: String) -> Result<bool> {
        let mut tasks = self.tasks.lock().unwrap();
        let removed = tasks.remove(&id).is_some();
        drop(tasks);
        if removed {
            self.save_tasks()?;
        }
        Ok(removed)
    }

    pub fn load_tasks(&self) -> Result<()> {
        match load_tasks_from_file() {
            Ok(loaded_tasks) => {
                let mut tasks = self.tasks.lock().unwrap();
                *tasks = loaded_tasks;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn save_tasks(&self) -> Result<()> {
        let tasks = self.tasks.lock().unwrap();
        save_tasks_to_file(&*tasks)
    }

    fn update_task_status(&self, id: String, status: TaskStatus) -> Result<bool> {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(task) = tasks.get_mut(&id) {
            task.task_info.set_status(status);
            drop(tasks);
            self.save_tasks()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn list_task(&self) -> Result<Vec<Task>> {
        return match load_tasks_from_file() {
            Ok(data) => {
                let mut list = vec![];
                for (key, value) in data.into_iter() {
                    list.push(value);
                }
                return Ok(list);
            }
            Err(e) => {
                Err(e)
            }
        };
    }

    pub fn run_job(&self) {
        println!("run job")
    }
}

pub async fn add_task(task_manager: web::Data<Arc<TaskManager>>, scheduler: web::Data<Arc<Mutex<Scheduler>>>, task_json: web::Json<TaskContent>) -> impl Responder {
    {
        let mut scheduler = scheduler.lock().unwrap();
        scheduler.every(10.seconds()).run(move || {
            println!("add task!");
        });
    }
    let mut resp = HashMap::new();
    let task = task_json.into_inner();
    match task_manager.add_task(task) {
        Ok(id) => {
            resp.insert("code", String::from("200"));
            resp.insert("data", id.to_string());
            HttpResponse::Ok().json(resp)
        }
        Err(_) => {
            resp.insert("code", String::from("500"));
            resp.insert("msg", String::from("internal error"));
            HttpResponse::Ok().json(resp)
        }
    }
}

pub async fn delete_task(task_manager: web::Data<Arc<TaskManager>>, path: web::Path<String>) -> impl Responder {
    let mut resp = HashMap::new();
    match task_manager.delete_task(path.into_inner().to_string()) {
        Ok(true) => {
            resp.insert("code", String::from("200"));
            resp.insert("msg", String::from("success"));
            HttpResponse::Ok().json(resp)
        }
        Ok(false) => {
            resp.insert("code", String::from("400"));
            resp.insert("msg", String::from("Task not found"));
            HttpResponse::Ok().json(resp)
        }
        Err(_) => {
            resp.insert("code", String::from("500"));
            resp.insert("msg", String::from("internal error"));
            HttpResponse::Ok().json(resp)
        }
    }
}

pub async fn list_task(task_manager: web::Data<Arc<TaskManager>>) -> impl Responder {
    let mut resp = HashMap::new();
    match task_manager.list_task() {
        Ok(data) => {
            resp.insert("list", data);
        }
        Err(_) => {}
    }
    HttpResponse::Ok().json(resp)
}

// 任务存储到文件的相关函数
fn save_tasks_to_file(tasks: &HashMap<String, Task>) -> Result<()> {
    let data = serde_json::to_vec(tasks)?;
    Ok(std::fs::write(FILE_PATH, &data)?)
}

fn load_tasks_from_file() -> Result<HashMap<String, Task>> {
    match std::fs::read(FILE_PATH) {
        Err(e) => {
            let mut data = File::create(FILE_PATH).unwrap();
            data.write_all(b"{}").unwrap()
        }
        _ => {}
    }
    let data = std::fs::read(FILE_PATH)?;
    Ok(serde_json::from_slice(&data)?)
}
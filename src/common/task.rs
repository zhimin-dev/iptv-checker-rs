use crate::common::do_check;
use actix_web::{web, HttpResponse, Responder};
use md5;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Result, Write};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const FILE_PATH: &str = "tasks.json";

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskInfo {
    // 定时时间
    run_type: RunTime,

    // 最后一次运行时间, (s)
    last_run_time: i32,

    // next run time, (s)
    next_run_time: i32,

    is_running: bool,

    // 任务状态
    task_status: TaskStatus,
}

#[warn(private_interfaces)]
#[derive(Debug, Deserialize, Serialize, Clone)]
enum RunTime {
    EveryDay,
    EveryHour,
}

impl TaskInfo {
    pub fn new() -> TaskInfo {
        return TaskInfo {
            run_type: RunTime::EveryDay,
            task_status: TaskStatus::Pending,
            last_run_time: 0,
            next_run_time: 0,
            is_running: false,
        };
    }

    pub fn set_run_type(&mut self, run_type: RunTime) {
        self.run_type = run_type
    }

    // pub fn set_status(&mut self, stats: TaskStatus) {
    //     self.task_status = stats
    // }

    pub fn set_next_run_time(&mut self, time: i32) {
        self.next_run_time = time
    }

    pub fn set_last_run_time(&mut self, time: i32) {
        self.next_run_time = time
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskContent {
    // 订阅源
    urls: Vec<String>,
    // 结果文件名，最后可以通过这个文件来获取结果
    result_name: String,
    // 最终的md5
    md5: String,
    // 运行类型
    run_type: RunTime,
    // 喜欢关键词
    keyword_like: Vec<String>,
    // 不喜欢关键词
    keyword_dislike: Vec<String>,
    // 下载远端文件
    http_timeout: i32,
    // 检查时的超时配置
    check_timeout: i32,
    // 并发数
    concurrent: i32,
    // 是否支持排序
    #[serde(default)]
    sort: bool,
    // 是否不检查
    #[serde(default)]
    no_check: bool,
    #[serde(default)]
    rename: bool,

    #[serde(default)]
    ffmpeg_check: bool,
}

const DEFAULT_TIMEOUT: i32 = 30000;
const DEFAULT_CONCURRENT: i32 = 30;

pub fn md5_str(input: String) -> String {
    let digest = md5::compute(input);

    format!("{:x}", digest)
}

impl TaskContent {
    pub fn new() -> TaskContent {
        TaskContent {
            urls: vec![],
            result_name: "".to_string(),
            md5: "".to_string(),
            run_type: RunTime::EveryDay,
            keyword_like: vec![],
            keyword_dislike: vec![],
            http_timeout: 0,
            check_timeout: 0,
            sort: false,
            concurrent: 1,
            no_check: false,
            rename: false,
            ffmpeg_check: false,
        }
    }

    pub fn valid(&self) -> Result<TaskContent> {
        let mut ori = TaskContent::new();
        if self.urls.is_empty() {
            return Err(Error::new(ErrorKind::Other, "参数错误"));
        }
        ori.set_urls(self.urls.clone());
        if self.result_name.is_empty() {
            return Err(Error::new(ErrorKind::Other, "参数错误"));
        }
        if !self.result_name.is_empty() {
            ori.set_result_file_name(self.result_name.clone())
        }
        if self.http_timeout > 0 {
            ori.set_http_timeout(self.http_timeout);
        }
        if self.check_timeout > 0 {
            ori.set_check_timeout(self.check_timeout);
        }
        if self.keyword_like.len() > 0 {
            ori.set_keyword_like(self.keyword_like.clone())
        }
        if self.keyword_dislike.len() > 0 {
            ori.set_keyword_dislike(self.keyword_dislike.clone())
        }
        if self.sort {
            ori.set_sort(self.sort);
        }
        if self.no_check {
            ori.set_no_check(self.no_check);
        }
        if self.concurrent > 0 {
            ori.set_concurrent(self.concurrent);
        }
        if self.ffmpeg_check {
            ori.set_ffmpeg_check(self.ffmpeg_check);
        }
        if self.rename {
            ori.set_rename(self.rename);
        }
        ori.set_run_type(self.run_type.clone());
        ori.gen_md5();

        Ok(ori)
    }

    pub fn get_urls(self) -> Vec<String> {
        self.urls
    }

    pub fn gen_md5(&mut self) {
        self.md5 = String::from("");
        let json_string = serde_json::to_string(&self).unwrap();
        self.md5 = md5_str(json_string);
    }

    pub fn set_urls(&mut self, urls: Vec<String>) {
        self.urls = urls;
    }

    pub fn set_result_file_name(&mut self, name: String) {
        self.result_name = name
    }

    pub fn set_keyword_like(&mut self, like: Vec<String>) {
        self.keyword_like = like;
    }

    pub fn set_keyword_dislike(&mut self, dislike: Vec<String>) {
        self.keyword_dislike = dislike;
    }

    pub fn set_sort(&mut self, sort: bool) {
        self.sort = sort
    }

    pub fn set_no_check(&mut self, no_check: bool) {
        self.no_check = no_check
    }

    pub fn set_concurrent(&mut self, concurrent: i32) {
        self.concurrent = concurrent
    }

    pub fn set_ffmpeg_check(&mut self, ffmpeg_check: bool) {
        self.ffmpeg_check = ffmpeg_check
    }

    pub fn set_rename(&mut self, rename: bool) {
        self.rename = rename
    }

    pub fn set_http_timeout(&mut self, timeout: i32) {
        self.http_timeout = timeout
    }

    pub fn get_current(self) -> i32 {
        let default_val = DEFAULT_CONCURRENT;
        if self.concurrent == 0 {
            default_val
        } else {
            self.concurrent
        }
    }

    pub fn get_http_timeout(self) -> i32 {
        let default_val = DEFAULT_TIMEOUT;
        if self.http_timeout > 0 {
            self.http_timeout
        } else {
            default_val
        }
    }

    pub fn get_check_timeout(self) -> i32 {
        let default_val = DEFAULT_TIMEOUT;
        if self.check_timeout > 0 {
            self.check_timeout
        } else {
            default_val
        }
    }

    pub fn set_check_timeout(&mut self, timeout: i32) {
        self.check_timeout = timeout
    }

    pub fn set_run_type(&mut self, run_type: RunTime) {
        self.run_type = run_type
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
enum TaskStatus {
    Pending,
    InProgress,
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
    return now
        .duration_since(UNIX_EPOCH)
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
        self.original = original.clone();
        self.task_info.set_run_type(original.run_type.clone());
    }

    pub fn get_uuid(&self) -> String {
        self.id.clone()
    }

    pub fn set_id(&mut self, id: String) {
        self.id = id.clone()
    }

    pub fn set_task_info(&mut self, task_info: TaskInfo) {
        self.task_info = task_info
    }

    pub fn get_task_info(self) -> TaskInfo {
        self.task_info
    }

    pub fn run(&mut self) {
        if self.task_info.is_running {
            return;
        }
        if self.task_info.next_run_time != 0 && self.task_info.next_run_time > now() as i32 {
            return;
        }
        self.task_info.is_running = true;
        self.task_info.task_status = TaskStatus::InProgress;
        let urls = self.clone().original.get_urls();
        let out_out_file = self.clone().original.result_name;
        let mut keyword_like = vec![];
        if self.clone().original.keyword_like.len() > 0 {
            keyword_like = self.clone().original.keyword_like
        }
        let mut keyword_dislike = vec![];
        if self.clone().original.keyword_dislike.len() > 0 {
            keyword_dislike = self.clone().original.keyword_dislike
        }
        let mut sort = false;
        if self.clone().original.sort {
            sort = self.clone().original.sort;
        }
        let task_id = self.clone().id.clone();
        let http_timeout = self.clone().original.get_http_timeout();
        let concurrent = self.clone().original.get_current();
        let no_check = self.clone().original.no_check;
        let check_timeout = self.clone().original.get_check_timeout();
        let rename = self.clone().original.rename;
        let ffmpeg_check = self.clone().original.ffmpeg_check;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            println!("start taskId: {}", task_id);
            let _ = do_check(
                urls,
                out_out_file.clone(),
                http_timeout,
                true,
                check_timeout,
                concurrent,
                keyword_like.clone(),
                keyword_dislike.clone(),
                sort,
                no_check,
                rename,
                ffmpeg_check,
            )
                .await;
            println!("end taskId: {}", task_id);
        });
        self.task_info.task_status = TaskStatus::Pending;
        self.task_info.is_running = false;
        self.task_info.last_run_time = now() as i32;
        let now_time = now() as i32;
        match self.task_info.run_type {
            RunTime::EveryDay => {
                self.task_info.next_run_time = now_time + 86400;
            }
            RunTime::EveryHour => {
                self.task_info.next_run_time = now_time + 3600;
            }
        }
    }
}

pub struct TaskManager {
    pub tasks: Mutex<HashMap<String, Task>>,
}

impl TaskManager {
    pub fn add_task(&self, task: TaskContent) -> Result<String> {
        let ori = task.valid().unwrap();
        let mut task = Task::new();
        task.set_original(ori);
        let mut tasks = self.tasks.lock().unwrap();
        tasks.insert(task.get_uuid(), task.clone());
        drop(tasks); // 显式释放锁以防止死锁
        self.save_tasks()?;
        Ok(task.get_uuid())
    }

    pub fn import_task_from_data(&self, data_map: HashMap<String, Task>) -> bool {
        let mut tasks = self.tasks.lock().unwrap();
        for (k, v) in data_map {
            let id = v.id.clone();
            if let None = tasks.get_mut(&id) {
                tasks.insert(k, v.clone());
            }
        }
        drop(tasks);
        if let Ok(_) = self.save_tasks() {
            return true;
        }
        false
    }

    pub fn run_task(&self, id: String) -> Result<bool> {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(task) = tasks.get_mut(&id) {
            task.task_info.set_next_run_time(now() as i32);
            drop(tasks);
            self.save_tasks()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn get_task(&self, id: String) -> Option<Task> {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(data) = tasks.get_mut(&id) {
            return Some(data.clone());
        }
        return None;
    }

    pub fn update_task(&self, id: String, pass_task: TaskContent) -> Result<bool> {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(task) = tasks.get_mut(&id) {
            let mut task_info = task.clone().get_task_info();
            let ori = pass_task.valid().unwrap();
            let mut task = Task::new();
            task.set_original(ori);
            task.set_id(id);
            task_info.set_run_type(pass_task.run_type);
            task.set_task_info(task_info);
            tasks.insert(task.get_uuid(), task.clone());
            drop(tasks);
            self.save_tasks()?;
            Ok(true)
        } else {
            Ok(false)
        }
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

    pub fn save_tasks(&self) -> Result<()> {
        let tasks = self.tasks.lock().unwrap();
        save_tasks_to_file(&*tasks)
    }

    // pub fn update_task_status(&self, id: String, status: TaskStatus) -> Result<bool> {
    //     let mut tasks = self.tasks.lock().unwrap();
    //     if let Some(task) = tasks.get_mut(&id) {
    //         task.task_info.set_status(status);
    //         drop(tasks);
    //         self.save_tasks()?;
    //         Ok(true)
    //     } else {
    //         Ok(false)
    //     }
    // }

    pub fn update_task_info(&self, id: String, task_info: TaskInfo) -> Result<bool> {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(task) = tasks.get_mut(&id) {
            task.set_task_info(task_info);
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
                for (_key, value) in data.into_iter() {
                    list.push(value);
                }
                list.sort_by(|a, b| a.create_time.cmp(&b.create_time));
                return Ok(list);
            }
            Err(e) => Err(e),
        };
    }
}

#[derive(Serialize, Deserialize)]
pub struct UpdateTaskQuery {
    task_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct RunTaskQuery {
    task_id: String,
}

pub async fn run_task(
    task_manager: web::Data<Arc<TaskManager>>,
    req: web::Query<RunTaskQuery>,
) -> impl Responder {
    println!("{}", req.task_id.clone());
    let mut resp = HashMap::new();
    match task_manager.run_task(req.task_id.clone()) {
        Ok(_) => {
            resp.insert("code", String::from("200"));
            resp.insert("data", req.task_id.to_string());
            HttpResponse::Ok()
                .content_type("application/json")
                .json(resp)
        }
        Err(err) => {
            resp.insert("code", String::from("500"));
            resp.insert("msg", String::from(err.to_string()));
            HttpResponse::Ok()
                .content_type("application/json")
                .json(resp)
        }
    }
}

pub async fn update_task(
    task_manager: web::Data<Arc<TaskManager>>,
    task_json: web::Json<TaskContent>,
    req: web::Query<UpdateTaskQuery>,
) -> impl Responder {
    println!("{}", req.task_id.clone());
    let mut resp = HashMap::new();
    let task = task_json.into_inner();
    match task_manager.update_task(req.task_id.clone(), task) {
        Ok(_) => {
            resp.insert("code", String::from("200"));
            resp.insert("data", req.task_id.to_string());
            HttpResponse::Ok().json(resp)
        }
        Err(err) => {
            resp.insert("code", String::from("500"));
            resp.insert("msg", String::from(err.to_string()));
            HttpResponse::Ok().json(resp)
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetDownloadBodyReq {
    task_id: String,
}

pub async fn system_tasks_export(_: web::Data<Arc<TaskManager>>) -> impl Responder {
    let data = get_task_from_file();
    if let Ok(inner) = data {
        HttpResponse::Ok().json(inner)
    } else {
        let mut resp = HashMap::new();
        resp.insert("code", String::from("500"));
        resp.insert("msg", String::from("导出失败"));
        HttpResponse::InternalServerError().json(resp)
    }
}

pub async fn system_tasks_import(
    task_manager: web::Data<Arc<TaskManager>>,
    req: web::Json<HashMap<String, Task>>,
) -> impl Responder {
    let mut resp = HashMap::new();
    if task_manager.import_task_from_data(req.into_inner()) {
        resp.insert("code", String::from("200"));
        resp.insert("msg", String::from("成功"));
        HttpResponse::Ok().json(resp)
    } else {
        resp.insert("code", String::from("500"));
        resp.insert("msg", String::from("导入失败"));
        HttpResponse::InternalServerError().json(resp)
    }
}

pub async fn get_download_body(
    task_manager: web::Data<Arc<TaskManager>>,
    req: web::Query<GetDownloadBodyReq>,
) -> impl Responder {
    let mut resp = HashMap::new();
    resp.insert("content", String::default());
    resp.insert("url", String::default());
    let task = task_manager.get_task(req.task_id.clone());
    if let Some(info) = task {
        let data = info.clone();
        resp.insert("url", data.original.result_name.clone());
        if let Some(contents) = get_file_contents(data.original.result_name) {
            resp.insert("content", contents.clone());
        }
    }
    return HttpResponse::Ok().json(resp);
}

fn get_file_contents(file_name: String) -> Option<String> {
    if let Ok(mut f) = File::open(file_name.clone()) {
        let mut contents = String::default();
        if let Ok(_) = f.read_to_string(&mut contents) {
            return Some(contents);
        }
    }
    Some(String::default())
}

pub async fn add_task(
    task_manager: web::Data<Arc<TaskManager>>,
    task_json: web::Json<TaskContent>,
) -> impl Responder {
    let mut resp = HashMap::new();
    let task = task_json.into_inner();
    match task_manager.add_task(task) {
        Ok(id) => {
            resp.insert("code", String::from("200"));
            resp.insert("data", id.to_string());
            HttpResponse::Ok().json(resp)
        }
        Err(err) => {
            resp.insert("code", String::from("500"));
            resp.insert("msg", String::from(err.to_string()));
            HttpResponse::Ok().json(resp)
        }
    }
}

pub async fn delete_task(
    task_manager: web::Data<Arc<TaskManager>>,
    path: web::Path<String>,
) -> impl Responder {
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
        Err(_) => {
            let mut data = File::create(FILE_PATH)?;
            data.write_all(b"{}")?
        }
        _ => {}
    }
    let data = std::fs::read(FILE_PATH)?;
    Ok(serde_json::from_slice(&data)?)
}

pub fn get_task_from_file() -> Result<HashMap<String, Task>> {
    return load_tasks_from_file();
}

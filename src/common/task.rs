use crate::common::do_check;
use crate::common::task::TaskStatus::InProgress;
use crate::config::config::file_config;
use crate::config::{get_now_check_task_id, save_task, set_now_check_id};
use actix_web::{web, HttpResponse, Responder};
use log::{debug, error, info};
use md5;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Result};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskInfo {
    // 定时时间
    run_type: RunTime,

    // 最后一次运行时间, (s)
    pub last_run_time: i32,

    // next run time, (s)
    pub next_run_time: i32,

    pub is_running: bool,

    // 任务状态
    pub task_status: TaskStatus,
}

#[warn(private_interfaces)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum RunTime {
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
    #[serde(default)]
    same_save_num: i32,

    #[serde(default)]
    not_http_skip: bool,

    // 视频质量
    #[serde(default)]
    video_quality: Vec<String>,
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
            same_save_num: 0,
            not_http_skip: false,
            video_quality: vec![],
        }
    }

    pub fn get_result_name(&self) -> String {
        self.result_name.clone()
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
        if self.not_http_skip {
            ori.set_not_http_skip(self.not_http_skip);
        }
        ori.set_video_quality(self.video_quality.clone());
        ori.set_same_save_num(self.same_save_num);
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

    pub fn set_video_quality(&mut self, qualities: Vec<String>) {
        self.video_quality = qualities
    }

    pub fn set_rename(&mut self, rename: bool) {
        self.rename = rename
    }

    pub fn set_not_http_skip(&mut self, not_http_skip: bool) {
        self.not_http_skip = not_http_skip
    }

    pub fn set_same_save_num(&mut self, same_save_num: i32) {
        self.same_save_num = same_save_num
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
pub enum TaskStatus {
    Pending,
    InProgress,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Task {
    //任务来源
    pub original: TaskContent,

    //任务id
    id: String,

    //任务创建时间
    create_time: u64,

    //任务详情
    pub task_info: TaskInfo,
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

    pub fn get_task(self) -> Task {
        return self.clone();
    }

    pub fn run(&mut self) {
        // 如果当前时间大于最后运行时间，那么就运行
        if self.task_info.next_run_time != 0 && self.task_info.next_run_time - now() as i32 > 0 {
            return;
        }
        self.task_info.is_running = true;
        let _ = save_task(self.id.clone(), self.clone().get_task());
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
        // 设置当前任务id
        set_now_check_id(Some(self.clone().id.clone()));
        let http_timeout = self.clone().original.get_http_timeout();
        let concurrent = self.clone().original.get_current();
        let no_check = self.clone().original.no_check;
        let check_timeout = self.clone().original.get_check_timeout();
        let rename = self.clone().original.rename;
        let ffmpeg_check = self.clone().original.ffmpeg_check;
        let same_save_num = self.clone().original.same_save_num;
        let not_http_skip = self.clone().original.not_http_skip;
        let video_quality = self.clone().original.video_quality;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            debug!("start taskId: {}", task_id);
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
                same_save_num,
                not_http_skip,
                video_quality,
            )
            .await;
            debug!("end taskId: {}", task_id);
        });
        self.task_info.task_status = TaskStatus::Pending;
        self.task_info.is_running = false;
        let now_time = now() as i32;
        // 设置当前为空
        set_now_check_id(None);
        match self.task_info.run_type {
            RunTime::EveryDay => {
                self.task_info.next_run_time = now_time + 86400;
            }
            RunTime::EveryHour => {
                self.task_info.next_run_time = now_time + 3600;
            }
        }
        self.task_info.last_run_time = now_time;
        // 更新任务信息
        if let Err(e) = save_task(self.id.clone(), self.clone().get_task()) {
            error!("Failed to update task {}: {}", self.id.clone(), e);
        }
    }
}

pub struct TaskManager {}

impl TaskManager {
    pub fn add_task(&self, task: TaskContent) -> Result<String> {
        let ori = task.valid().unwrap();
        let mut task = Task::new();
        task.set_original(ori);
        let id = task.get_uuid();
        if let Err(e) = file_config::save_task(id.clone(), task) {
            return Err(Error::new(ErrorKind::Other, e.to_string()));
        }
        if let Err(e) = file_config::save_config() {
            return Err(Error::new(ErrorKind::Other, e.to_string()));
        }
        Ok(id)
    }

    pub fn import_task_from_data(&self, data_map: HashMap<String, Task>) -> bool {
        for (k, v) in data_map {
            if let Err(_) = file_config::save_task(k, v) {
                return false;
            }
        }
        if let Err(_) = file_config::save_config() {
            return false;
        }
        true
    }

    pub fn run_task(&self, id: String) -> Result<bool> {
        if let Ok(Some(mut task)) = file_config::get_task(&id) {
            task.task_info.set_next_run_time(now() as i32 + 60);
            if let Err(_) = file_config::save_task(id, task) {
                return Ok(false);
            }
            if let Err(_) = file_config::save_config() {
                return Ok(false);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn get_task(&self, id: String) -> Option<Task> {
        if let Ok(Some(task)) = file_config::get_task(&id) {
            Some(task)
        } else {
            None
        }
    }

    pub fn update_task(&self, id: String, pass_task: TaskContent) -> Result<bool> {
        if let Ok(Some(task)) = file_config::get_task(&id) {
            let mut task_info = task.clone().get_task_info();
            let ori = pass_task.valid()?;
            let mut new_task = Task::new();
            new_task.set_original(ori);
            new_task.set_id(id);
            task_info.set_run_type(pass_task.run_type);
            new_task.set_task_info(task_info);
            if let Err(_) = file_config::save_task(new_task.get_uuid(), new_task) {
                return Ok(false);
            }
            if let Err(_) = file_config::save_config() {
                return Ok(false);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn delete_task(&self, id: String) -> Result<bool> {
        if let Err(_) = file_config::delete_task(&id) {
            Ok(false)
        } else {
            if let Err(_) = file_config::save_config() {
                Ok(false)
            } else {
                Ok(true)
            }
        }
    }

    pub fn list_task(&self) -> Result<Vec<Task>> {
        if let Ok(tasks) = file_config::get_all_tasks() {
            let mut list: Vec<Task> = tasks.into_values().collect();
            list.sort_by(|a, b| a.create_time.cmp(&b.create_time));
            Ok(list)
        } else {
            Err(Error::new(ErrorKind::Other, "Failed to get tasks"))
        }
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
    debug!("{}", req.task_id.clone());
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
    info!("{}", req.task_id.clone());
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
    if let Ok(tasks) = file_config::get_all_tasks() {
        HttpResponse::Ok().json(tasks)
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

pub fn get_file_contents(file_name: String) -> Option<String> {
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskListResponse {
    pub list: Vec<Task>,
    pub now_id: Option<String>,
}

impl TaskListResponse {
    pub fn new() -> TaskListResponse {
        TaskListResponse {
            list: Vec::new(),
            now_id: None,
        }
    }
    pub fn set_list(&mut self, list: Vec<Task>) {
        self.list = list;
    }

    pub fn set_now_id(&mut self, now_id: Option<String>) {
        self.now_id = now_id;
    }
}

pub async fn list_task(task_manager: web::Data<Arc<TaskManager>>) -> impl Responder {
    let mut resp = TaskListResponse::new();

    if let Ok(data) = task_manager.list_task() {
        resp.set_list(data);
        resp.set_now_id(get_now_check_task_id());
    }

    HttpResponse::Ok().json(resp)
}

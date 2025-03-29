use crate::common::check;
use crate::common::task::{
    add_task, delete_task, get_download_body, list_task, run_task, system_tasks_export,
    system_tasks_import, update_task, TaskManager,
};
use actix_files as fs;
use actix_files::NamedFile;
use actix_multipart::form::{tempfile::TempFile, MultipartForm};
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use clokwerk::{Scheduler, TimeUnits};
use serde::{Deserialize, Serialize};
use simplelog::{CombinedLogger, Config, WriteLogger};
use std::collections::HashMap;
use std::fmt::format;
use std::fs::File;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time;
use std::time::Duration;

#[derive(Debug, Deserialize, Serialize)]
struct TaskDel {
    task_id: String, //任务id
}

#[derive(Debug, Deserialize, Serialize)]
struct TaskDelResp {
    result: bool, //是否成功
}

pub async fn check_ipv6() -> bool {
    let result = reqwest::get("http://[2606:2800:220:1:248:1893:25c8:1946]").await;

    match result {
        Ok(_) => true,
        Err(_) => {
            // 处理错误，根据错误类型返回更探针对性的信息也可以
            // HttpResponse::Ok().body(format!("IPv6 might not be supported: {}", e))
            false
        }
    }
}

#[derive(Serialize, Deserialize)]
struct CheckUrlIsAvailableRequest {
    url: String,
    timeout: Option<i32>,
}

#[get("/check/url-is-available")]
async fn check_url_is_available(req: web::Query<CheckUrlIsAvailableRequest>) -> impl Responder {
    let mut timeout = 0;
    if let Some(i) = req.timeout {
        timeout = i;
    }
    let res = check::check::check_link_is_valid(req.url.to_owned(), timeout as u64,
                                                true, false, true);
    match res.await {
        Ok(data) => {
            let obj = serde_json::to_string(&data).unwrap();
            return HttpResponse::Ok().body(obj);
        }
        Err(e) => {
            error!("check_url_is_available error {}", e);
            return HttpResponse::InternalServerError().body("{\"msg\":\"internal error\"}");
        }
    };
}

#[derive(Serialize, Deserialize)]
struct FetchM3uBodyRequest {
    url: String,
    timeout: Option<i32>,
}

#[get("/fetch/m3u-body")]
async fn fetch_m3u_body(req: web::Query<FetchM3uBodyRequest>) -> impl Responder {
    let mut timeout = 0;
    if let Some(i) = req.timeout {
        timeout = i;
    }
    let client = reqwest::Client::builder()
        .timeout(time::Duration::from_millis(timeout as u64))
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let resp = client.get(req.url.to_owned()).send().await;
    match resp {
        Ok(res) => {
            if res.status().is_success() {
                let body = res.text().await;
                match body {
                    Ok(text) => {
                        return HttpResponse::Ok().body(text);
                    }
                    Err(e) => {
                        error!("resp status error : {}", e);
                        return HttpResponse::InternalServerError()
                            .body("{\"msg\":\"internal error, fetch body error\"}");
                    }
                }
            }
            return HttpResponse::InternalServerError()
                .body("{\"msg\":\"internal error, status is not 200\"}");
        }
        Err(e) => {
            error!("fetch error : {}", e);
            return HttpResponse::InternalServerError()
                .body("{\"msg\":\"internal error, fetch error\"}");
        }
    };
}

pub static VIEW_BASE_DIR: &str = "./static/";

#[derive(Serialize, Deserialize)]
struct SystemStatusResp {
    can_ipv6: bool,
    version: String,
    output: String,
}

#[get("/system/info")]
async fn system_status() -> impl Responder {
    let check_ipv6 = check_ipv6().await;
    let system_status = SystemStatusResp {
        can_ipv6: check_ipv6,
        version: env!("CARGO_PKG_VERSION").to_string(),
        output: format!("{}{}", VIEW_BASE_DIR, "upload".to_string()),
    };
    let obj = serde_json::to_string(&system_status).unwrap();
    return HttpResponse::Ok()
        .append_header(("Content-Type", "application/json"))
        .body(obj);
}

#[get("/")]
async fn index() -> impl Responder {
    let path: std::path::PathBuf = "./web/index.html".into(); // 替换为实际的 index.html 路径
    NamedFile::open(path)
}

#[derive(Debug, MultipartForm)]
struct UploadFormReq {
    #[multipart(rename = "file")]
    file: TempFile,
}

#[derive(Serialize, Deserialize)]
struct UploadResponse {
    msg: String,
    url: String,
}

#[post("/media/upload")]
async fn upload(MultipartForm(form): MultipartForm<UploadFormReq>) -> impl Responder {
    let path = format!("static/input/{}", form.file.file_name.unwrap());
    log::info!("saving to {path}");
    form.file.file.persist(path.clone()).unwrap();
    let resp = UploadResponse {
        msg: "success".to_string(),
        url: path.clone().to_string(),
    };
    let obj = serde_json::to_string(&resp).unwrap();
    return HttpResponse::Ok()
        .append_header(("Content-Type", "application/json"))
        .body(obj);
}

pub async fn start_web(port: u16) {
    let log_file = File::create(format!("./static/logs/app-{}.log", Local::now().format("%Y%m%d%H:%M").to_string())).unwrap();
    let cb_logger = CombinedLogger::init(
        vec![
            WriteLogger::new(
                LevelFilter::Debug,
                Config::default(),
                log_file,
            ),
            WriteLogger::new(
                LevelFilter::Debug,
                Config::default(),
                std::io::stdout()),
        ]
    );
    match cb_logger {
        Ok(cb_data) => {}
        Err(e) => {
            error!("cb_logger: {}",e)
        }
    }

    let data = Arc::new(TaskManager {
        tasks: Mutex::new(HashMap::new()),
    });

    // 尝试从文件加载任务
    if let Err(e) = data.load_tasks() {
        error!("Failed to load tasks: {}", e);
    }

    // 使用 Arc<Mutex<Scheduler>> 来共享 scheduler
    let scheduler: Arc<Mutex<Scheduler>> = Arc::new(Mutex::new(Scheduler::with_tz(chrono::Local)));

    // 创建一个新线程来运行定时任务
    let scheduler_thread = {
        let scheduler = Arc::clone(&scheduler);
        thread::spawn(move || loop {
            {
                let mut scheduler = scheduler.lock().unwrap();
                scheduler.run_pending();
            }
            thread::sleep(Duration::from_secs(30));
        })
    };
    let data_clone = Arc::clone(&data);
    {
        let mut scheduler = scheduler.lock().unwrap();
        scheduler.every(30.seconds()).run(move || {
            let data_clone = Arc::clone(&data_clone);
            let tasks = data_clone.list_task().unwrap();
            for mut task in tasks {
                task.run();
                data_clone
                    .update_task_info(task.get_uuid(), task.get_task_info())
                    .unwrap();
            }
        });
    }
    let data_clone_for_http = Arc::clone(&data);
    let _ = HttpServer::new(move || {
        let data_clone_for_http_server = Arc::clone(&data_clone_for_http);
        App::new()
            .service(check_url_is_available)
            .service(fetch_m3u_body)
            .service(system_status)
            .service(index)
            .service(upload)
            .service(fs::Files::new("/static", VIEW_BASE_DIR.to_owned()).show_files_listing())
            .app_data(web::Data::new(data_clone_for_http_server))
            .app_data(web::Data::new(scheduler.clone()))
            .route("/tasks/list", web::get().to(list_task))
            .route("/tasks/run", web::get().to(run_task))
            .route("/tasks/update", web::post().to(update_task))
            .route("/tasks/add", web::post().to(add_task))
            .route("/tasks/get-download-body", web::get().to(get_download_body))
            .route("/system/tasks/export", web::get().to(system_tasks_export))
            .route("/system/tasks/import", web::post().to(system_tasks_import))
            .route("/tasks/delete/{id}", web::delete().to(delete_task))
            .service(fs::Files::new("/", "./web/"))
            .wrap(Logger::default())
    })
        .bind(("0.0.0.0", port))
        .expect("Failed to bind address")
        .run()
        .await
        .expect("failed to run server");

    scheduler_thread.join().unwrap();
}

use crate::middleware::Logging;
use actix_web::middleware::Logger;
use chrono::Local;
use clap::ColorChoice;
use env_logger::fmt::style::{Color, RgbColor};
use env_logger::Env;
use log::Level::Trace;
use log::{error, info, LevelFilter};
use std::io::Write;

pub fn init_logger() {
    let env = Env::default().filter_or("MY_LOG_LEVEL", "debug");
    // 设置日志打印格式
    env_logger::Builder::from_env(env).format(|buf, record| {
        Ok({
            // let level_color = match record.level() {
            //     log::Level::Error => Color::Rgb(RgbColor(231,28,31)),
            //     log::Level::Warn => Color::Rgb(RgbColor(209,223,17)),
            //     log::Level::Info => Color::Rgb(RgbColor(39,165,0)),
            //     log::Level::Debug | log::Level::Trace => Color::Rgb(RgbColor(117,90,179)),
            // };

            // let mut level_style = buf.default_level_style(Trace);
            // level_style.set_color(level_color).set_bold(true);
            //
            // let mut style = buf.style();
            // style.set_color(Color::Rgb(RgbColor(255,255,255))).set_dimmed(true);

            write!(buf, "{} {} [ {} ] {}\n",
                   Local::now().format("%Y-%m-%d %H:%M:%S"),
                   record.level(),
                   record.module_path().unwrap_or("<unnamed>"),
                   record.args()).unwrap();
        })
    }).filter(None, LevelFilter::Debug).init();
    info!("env_logger initialized.");
}
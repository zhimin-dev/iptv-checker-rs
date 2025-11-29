use crate::common::check;
use crate::common::task::{
    add_task, delete_task, get_download_body, list_task, run_task, system_tasks_export,
    system_tasks_import, update_task, TaskManager,
};
use crate::common::translate::init_from_default_file;
use crate::common::favourite::get_favourite_map;
use crate::config::config::{init_config, Search};
use crate::config::global::{get_config, init_data_from_file, update_config};
use crate::config::{get_check, get_task};
use crate::r#const::constant::{INPUT_FOLDER, REPLACE_JSON, STATIC_FOLDER};
use crate::search::init_search_data;
use std::path::Path;
use actix_files as fs;
use actix_files::NamedFile;
use actix_multipart::form::{tempfile::TempFile, MultipartForm};
use actix_web::middleware::Logger;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use clokwerk::{Scheduler, TimeUnits};
use chrono::Local;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time;
use std::time::Duration;
use tokio::signal;

/// 更新全局配置请求结构体
#[derive(Serialize, Deserialize)]
struct UpdateGlobalConfigRequest {
    remote_url2local_images: bool,
    search: Search,
}

/// 更新全局配置
#[post("/system/global-config")]
async fn update_global_config(req: web::Json<UpdateGlobalConfigRequest>) -> impl Responder {
    let result = update_config(|config| {
        config.remote_url2local_images = req.remote_url2local_images;
        config.search = req.search.clone();
    });
    if result.is_ok() {
        let _ = init_data_from_file();
        return HttpResponse::Ok()
            .append_header(("Content-Type", "application/json"))
            .body("{\"msg\":\"success\"}");
    }
    return HttpResponse::InternalServerError().body("{\"msg\":\"Failed to save configuration\"}");
}

/// 删除任务请求结构体
#[derive(Debug, Deserialize, Serialize)]
struct TaskDel {
    task_id: String, // 任务ID
}

/// 删除任务响应结构体
#[derive(Debug, Deserialize, Serialize)]
struct TaskDelResp {
    result: bool, // 操作是否成功
}

// /// 检查系统是否支持IPv6
// pub async fn check_ipv6() -> bool {
//     let result = reqwest::get("http://[2606:2800:220:1:248:1893:25c8:1946]").await;
//
//     match result {
//         Ok(_) => true,
//         Err(_) => false,
//     }
// }

/// URL可用性检查请求结构体
#[derive(Serialize, Deserialize)]
struct CheckUrlIsAvailableRequest {
    url: String,
    timeout: Option<i32>,
}

/// 检查URL是否可用的API端点
#[get("/check/url-is-available")]
async fn check_url_is_available(req: web::Query<CheckUrlIsAvailableRequest>) -> impl Responder {
    let mut timeout = 0;
    if let Some(i) = req.timeout {
        timeout = i;
    }
    let res =
        check::check::check_link_is_valid(req.url.to_owned(), timeout as u64, true, false).await;
    match res {
        Ok(mut data) => {
            if data.ffmpeg_info.is_some() {
                let ff = data.clone().ffmpeg_info.unwrap();
                data.audio = ff.audio;
                if ff.video.len() > 0 {
                    data.video = Some(ff.video[0].clone());
                }
            }
            let obj = serde_json::to_string(&data.clone()).unwrap();
            return HttpResponse::Ok().body(obj);
        }
        Err(e) => {
            error!("check_url_is_available error {}", e);
            return HttpResponse::InternalServerError().body("{\"msg\":\"internal error\"}");
        }
    };
}

/// 获取replace.json配置
#[get("/system/replace")]
async fn get_replace_config() -> impl Responder {
    let replace_path = format!("{}", REPLACE_JSON);
    match std::fs::read_to_string(&replace_path) {
        Ok(content) => HttpResponse::Ok()
            .append_header(("Content-Type", "application/json"))
            .body(content),
        Err(_) => {
            // 如果文件不存在，返回空数组
            HttpResponse::Ok()
                .append_header(("Content-Type", "application/json"))
                .body("[]")
        }
    }
}

/// 更新replace.json配置请求结构体
#[derive(Serialize, Deserialize)]
struct UpdateReplaceRequest {
    content: String,
}

/// 更新replace.json配置
#[post("/system/replace")]
async fn update_replace_config(req: web::Json<UpdateReplaceRequest>) -> impl Responder {
    let replace_path = format!("{}", REPLACE_JSON);

    // 验证JSON格式
    if let Err(_) = serde_json::from_str::<serde_json::Value>(&req.content) {
        return HttpResponse::BadRequest().body("{\"msg\":\"Invalid JSON format\"}");
    }

    match std::fs::write(&replace_path, &req.content) {
        Ok(_) => {
            let _ = init_from_default_file();
            HttpResponse::Ok()
                .append_header(("Content-Type", "application/json"))
                .body("{\"msg\":\"success\"}")
        }
        Err(e) => {
            error!("Failed to write replace.json: {}", e);
            HttpResponse::InternalServerError().body("{\"msg\":\"Failed to save configuration\"}")
        }
    }
}

/// 获取M3U文件内容请求结构体
#[derive(Serialize, Deserialize)]
struct FetchM3uBodyRequest {
    url: String,
    timeout: Option<i32>,
}

/// 获取M3U文件内容的API端点
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

/// 系统状态响应结构体
#[derive(Serialize, Deserialize)]
struct SystemStatusResp {
    remote_url2local_images: bool, //是否需要转换远程图片
    search: Search,
    today_fetch: bool,// 是否处理爬取
}

/// 文件列表和内容响应体
#[derive(Serialize, Deserialize)]
struct FileContent {
    label: String,
    content: String,
}

/// 清空今日搜索文件夹的API端点
#[get("/system/clear-search-folder")]
async fn system_clear_search_folder() -> impl Responder {
    use crate::search;
    match search::clear_search_folder() {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"msg": "clear search folder success"})),
        Err(e) => {
            log::error!("clear search folder failed: {}", e);
            HttpResponse::InternalServerError()
                .json(serde_json::json!({"msg": "internal error, clear search folder failed"}))
        }
    }
}

/// 初始化今日搜索数据的API端点
#[get("/system/init-search-data")]
async fn system_init_search_data() -> impl Responder {
    use crate::search::init_search_data;

    match init_search_data().await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"msg": "init search data success"})),
        Err(e) => {
            log::error!("init search data failed: {}", e);
            HttpResponse::InternalServerError()
                .json(serde_json::json!({"msg": format!("internal error, init search data failed: {}", e)}))
        }
    }
}


/// 获取今日搜索文件夹下所有文件及其内容的API端点
#[get("/system/list-today-files")]
async fn system_list_today_files() -> impl Responder {
    use std::fs;
    use std::io::Read;
    use chrono::Local;

    // 拼出今日的路径
    let today = Local::now().format("%Y%m%d").to_string();
    let folder_path = format!("{}/input/search/{}", STATIC_FOLDER, today);

    let dir = std::path::Path::new(&folder_path);
    let mut result: Vec<FileContent> = vec![];
    if dir.exists() && dir.is_dir() {
        match fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            let mut file = match fs::File::open(&path) {
                                Ok(f) => f,
                                Err(e) => {
                                    log::error!("读取文件失败 {}: {}", name, e);
                                    continue;
                                }
                            };
                            let mut content = String::new();
                            if let Err(e) = file.read_to_string(&mut content) {
                                log::error!("读取文件内容失败 {}: {}", name, e);
                                continue;
                            }
                            result.push(FileContent {
                                label: name.to_string(),
                                content,
                            });
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("目录读取失败: {}", e);
                return HttpResponse::InternalServerError()
                    .body("{\"msg\":\"internal error, cannot read dir\"}");
            }
        }
    } else {
        return HttpResponse::Ok()
            .append_header(("Content-Type", "application/json"))
            .body("[]");
    }
    let resp = match serde_json::to_string(&result) {
        Ok(json) => json,
        Err(e) => {
            log::error!("序列化文件内容失败: {}", e);
            return HttpResponse::InternalServerError()
                .body("{\"msg\":\"internal error, cannot serialize\"}");
        }
    };
    HttpResponse::Ok()
        .append_header(("Content-Type", "application/json"))
        .body(resp)
}

/// 打开URL请求结构体
#[derive(Serialize, Deserialize)]
struct OpenUrlRequest {
    url: String,
}

/// 获取URL内容的API端点
#[get("/system/open-url")]
async fn system_open_url(req: web::Query<OpenUrlRequest>) -> impl Responder {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    
    match client.get(&req.url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.text().await {
                    Ok(text) => HttpResponse::Ok().body(text),
                    Err(e) => {
                        error!("Failed to read response text: {}", e);
                        HttpResponse::InternalServerError()
                            .json(serde_json::json!({"msg": format!("Failed to read response: {}", e)}))
                    }
                }
            } else {
                 HttpResponse::InternalServerError()
                    .json(serde_json::json!({"msg": format!("Request failed with status: {}", resp.status())}))
            }
        }
        Err(e) => {
            error!("Failed to fetch URL: {}", e);
            HttpResponse::InternalServerError()
                .json(serde_json::json!({"msg": format!("Failed to fetch URL: {}", e)}))
        }
    }
}

/// 打开URL请求结构体
#[derive(Serialize, Deserialize)]
struct GetFavouriteChannelRequest {
    channel_type: String,
}

/// 获取URL内容的API端点
#[get("/system/get-favourite-channel")]
async fn system_get_favourite_channel(req: web::Query<GetFavouriteChannelRequest>) -> impl Responder {
    let channel_type = req.channel_type.to_owned();
    if channel_type != "all" && channel_type != "like"  {
        return HttpResponse::BadRequest().body("{\"msg\":\"invalid channel type\"}");
    }
    let data = match check::get_favourite_channel(channel_type).await {
        Ok(data) => data,
        Err(_e) => {
            return HttpResponse::InternalServerError().body("{\"msg\":\"internal error, get favourite channel failed\"}");
        }
    };
    return HttpResponse::Ok()
        .append_header(("Content-Type", "text/plain; charset=utf-8"))
        .body(data);
}

#[derive(Serialize, Deserialize)]
struct FavouriteListResponse {
    like: Vec<String>,
    equal: Vec<String>,
    all_channel_url: String,
    liked_channel_url: String,
    checked_liked_channel_url: String,
}

#[derive(Serialize, Deserialize)]
struct SaveFavouriteRequest {
    like: Vec<String>,
    equal: Vec<String>,
}

#[post("/system/save-favourite")]
async fn system_save_favourite(req: web::Json<SaveFavouriteRequest>) -> impl Responder {
    let mut map = std::collections::HashMap::new();
    map.insert("like", req.like.clone());
    map.insert("equal", req.equal.clone());

    match serde_json::to_string_pretty(&map) {
        Ok(json_str) => {
             let file_path = crate::r#const::constant::FAVOURITE_FILE_NAME;
             match std::fs::write(file_path, json_str) {
                 Ok(_) => HttpResponse::Ok().json(serde_json::json!({"msg": "success"})),
                 Err(e) => {
                     log::error!("Failed to write favourite file: {}", e);
                     HttpResponse::InternalServerError().json(serde_json::json!({"msg": "save failed"}))
                 }
             }
        },
        Err(e) => {
            log::error!("Failed to serialize favourite list: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"msg": "serialize failed"}))
        }
    }
}

#[get("/system/get-favourite")]
async fn system_get_favourite() -> impl Responder {
    let fav_map = get_favourite_map();
    let like = fav_map.get("like").cloned().unwrap_or_default();
    let equal = fav_map.get("equal").cloned().unwrap_or_default();

    let resp = FavouriteListResponse {
        like,
        equal,
        all_channel_url: "/system/get-favourite-channel?channel_type=all".to_string(),
        liked_channel_url: "/system/get-favourite-channel?channel_type=like".to_string(),
        checked_liked_channel_url: "".to_string(),
    };

    HttpResponse::Ok().json(resp)
}

/// 获取系统信息的API端点
#[get("/system/info")]
async fn system_status() -> impl Responder {
    let today = Local::now().format("%Y%m%d").to_string();
    let search_path = format!("{}/input/search/{}", STATIC_FOLDER, today);
    let today_fetch = Path::new(&search_path).exists();
    
    let system_status = SystemStatusResp {
        remote_url2local_images: get_config().remote_url2local_images,
        search: get_config().search,
        today_fetch,
    };
    let obj = serde_json::to_string(&system_status).unwrap();
    return HttpResponse::Ok()
        .append_header(("Content-Type", "application/json"))
        .body(obj);
}

/// 首页API端点
#[get("/")]
async fn index() -> impl Responder {
    let path: std::path::PathBuf = "./web/index.html".into();
    NamedFile::open(path)
}

/// 文件上传请求结构体
#[derive(Debug, MultipartForm)]
struct UploadFormReq {
    #[multipart(rename = "file")]
    file: TempFile,
}

/// 文件上传响应结构体
#[derive(Serialize, Deserialize)]
struct UploadResponse {
    msg: String, // 响应消息
    url: String, // 文件URL
}

/// 文件上传API端点
#[post("/media/upload")]
async fn upload(MultipartForm(form): MultipartForm<UploadFormReq>) -> impl Responder {
    let path = format!("{}{}", INPUT_FOLDER, form.file.file_name.unwrap());
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

/// 启动Web服务器
pub async fn start_web(port: u16) {
    // 初始化配置
    init_config();

    // 初始化任务管理器
    let task_manager = Arc::new(TaskManager {});

    // 创建定时任务调度器
    let scheduler: Arc<Mutex<Scheduler>> = Arc::new(Mutex::new(Scheduler::with_tz(chrono::Local)));

    // 创建定时任务执行线程
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

    // Use atomic bool for thread-safe locking
    let lock = Arc::new(Mutex::new(false));

    // 设置定时任务
    {
        let mut scheduler = scheduler.lock().unwrap();
        let lock_clone = Arc::clone(&lock);
        // 每10分钟运行一次，检查
        scheduler.every(2.minutes()).run(move || {
            info!("start search task");
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                let data = init_search_data().await;
                if data.is_err() {
                    error!("search data failed");
                }
                info!("search task finished");
            });
        });
        // 检查任务
        scheduler.every(30.seconds()).run(move || {
            // 判断当前是否有任务在并行运行，如果有，再判断任务是否已经运行了超过10分钟，如果超过了，可以再次运行
            let mut locked_flag = lock_clone.lock().unwrap();
            if *locked_flag {
                debug!("scheduler thread lock");
                return;
            }
            *locked_flag = true;
            // 获取所有任务
            if let Ok(tasks) = get_check() {
                for (id, _) in tasks.task {
                    // 运行任务
                    if let Ok(task) = get_task(&id) {
                        if let Some(mut task) = task {
                            // 运行任务
                            task.run();
                        }
                    }
                }
            }
            *locked_flag = false;
        });
    }

    let server = HttpServer::new(move || {
        App::new()
            .service(check_url_is_available)
            .service(fetch_m3u_body)
            .service(system_status)
            .service(system_list_today_files)
            .service(system_clear_search_folder)
            .service(system_init_search_data)
            .service(system_open_url)
            .service(system_get_favourite_channel)
            .service(system_save_favourite)
            .service(system_get_favourite)
            .service(update_replace_config)
            .service(get_replace_config)
            .service(update_global_config)
            .service(index)
            .service(upload)
            .service(fs::Files::new("/static", STATIC_FOLDER.to_owned()).show_files_listing())
            .app_data(web::Data::new(scheduler.clone()))
            .app_data(web::Data::new(Arc::clone(&task_manager)))
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
    .shutdown_timeout(60)
    .run();

    let server_handle = server.handle();
    let server_task = tokio::spawn(server);

    // Wait for Ctrl+C
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Shutting down server...");
            server_handle.stop(true).await;
        }
        _ = server_task => {}
    }

    // Wait for scheduler thread to finish
    scheduler_thread.join().unwrap();
}

use crate::common::check;
use crate::config::favourite::{get_favourite_map, reload_favourite_map};
use crate::common::task::{
    add_task, delete_task, get_download_body, get_file_contents, list_task, run_task,
    system_tasks_export, system_tasks_import, update_task, TaskManager,
};
use crate::common::translate::init_from_default_file;
use crate::config::search::SearchConfig;
use crate::config::{get_task, get_all_tasks};
use crate::r#const::constant::{
    INPUT_FOLDER, INPUT_SEARCH_FOLDER, LOGOS_FOLDER, STATIC_FOLDER,
};
use crate::search::init_search_data;
use actix_files as actix_fs;
use actix_files::NamedFile;
use actix_multipart::form::{tempfile::TempFile, MultipartForm};
use actix_web::middleware::Logger;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use chrono::Local;
use clokwerk::{Scheduler, TimeUnits};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time;
use std::time::Duration;
use tokio::signal;
use zip::{ZipArchive, ZipWriter};
use zip::write::FileOptions;

/// 更新全局配置请求结构体
#[derive(Serialize, Deserialize)]
struct UpdateGlobalConfigRequest {
    search: SearchConfig,
}

/// 更新全局配置
#[post("/system/global-config")]
async fn update_global_config(req: web::Json<UpdateGlobalConfigRequest>) -> impl Responder {
    let result = crate::config::search::update_search_config(req.search.clone());
    if result.is_ok() {
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
    match crate::config::replace::get_replace_config_json() {
        Ok(json) => HttpResponse::Ok()
            .append_header(("Content-Type", "application/json"))
            .body(json),
        Err(e) => {
            error!("Failed to get replace config: {}", e);
            HttpResponse::InternalServerError()
                .body("{\"msg\":\"Failed to get configuration\"}")
        }
    }
}

/// Replace配置请求结构体
#[derive(Debug, Serialize, Deserialize)]
struct UpdateReplaceConfigRequest {
    replace_string: bool,
    replace_map: HashMap<String, String>,
}

/// 更新replace.json配置
#[post("/system/replace")]
async fn update_replace_config(req: web::Json<UpdateReplaceConfigRequest>) -> impl Responder {
    // 使用 config 模块保存
    match crate::config::replace::partial_update_replace_config(
        req.replace_string,
        req.replace_map.clone(),
    ) {
        Ok(_) => {
            let _ = init_from_default_file();
            HttpResponse::Ok()
                .append_header(("Content-Type", "application/json"))
                .body("{\"msg\":\"success\"}")
        }
        Err(e) => {
            error!("Failed to update replace config: {}", e);
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
    search: SearchConfig,
    today_fetch: bool, // 是否处理爬取
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
    use chrono::Local;
    use std::fs;
    use std::io::Read;

    // 拼出今日的路径
    let today = Local::now().format("%Y%m%d").to_string();
    let folder_path = format!("{}/{}", INPUT_SEARCH_FOLDER, today);

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
                        HttpResponse::InternalServerError().json(
                            serde_json::json!({"msg": format!("Failed to read response: {}", e)}),
                        )
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
async fn system_get_favourite_channel(
    req: web::Query<GetFavouriteChannelRequest>,
) -> impl Responder {
    let channel_type = req.channel_type.to_owned();
    if channel_type != "all" && channel_type != "like" {
        return HttpResponse::BadRequest().body("{\"msg\":\"invalid channel type\"}");
    }
    let data = match check::get_favourite_channel(channel_type).await {
        Ok(data) => data,
        Err(_e) => {
            return HttpResponse::InternalServerError()
                .body("{\"msg\":\"internal error, get favourite channel failed\"}");
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
    use crate::config::favourite::FavouriteConfig;
    
    let config = FavouriteConfig {
        like: req.like.clone(),
        equal: req.equal.clone(),
    };

    // 使用 config 模块保存
    match crate::config::favourite::update_favourite_config(config) {
        Ok(_) => {
            // 重新加载 favourite map
            if let Err(e) = reload_favourite_map() {
                log::error!("Failed to reload favourite map: {}", e);
            }
            HttpResponse::Ok().json(serde_json::json!({"msg": "success"}))
        }
        Err(e) => {
            log::error!("Failed to update favourite config: {}", e);
            HttpResponse::InternalServerError()
                .json(serde_json::json!({"msg": "Failed to save configuration"}))
        }
    }
}

#[get("/system/get-favourite")]
async fn system_get_favourite() -> impl Responder {
    let fav_map = get_favourite_map();
    let like = fav_map.like.clone();
    let equal = fav_map.equal.clone();

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
    let search_path = format!("{}/{}", INPUT_SEARCH_FOLDER, today);
    let today_fetch = Path::new(&search_path).exists();

    let config = crate::config::search::get_search_config();
    let system_status = SystemStatusResp {
        search: config,
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
    let file_name = match form.file.file_name {
        Some(name) => name,
        None => {
            return HttpResponse::BadRequest()
                .json(serde_json::json!({"msg": "Missing file name", "url": ""}))
        }
    };
    let path = format!("{}{}", INPUT_FOLDER, file_name);
    log::info!("saving to {path}");
    if let Err(e) = form.file.file.persist(path.clone()) {
        log::error!("Failed to save file: {}", e);
        return HttpResponse::InternalServerError()
            .json(serde_json::json!({"msg": format!("Failed to save file: {}", e), "url": ""}));
    }
    let resp = UploadResponse {
        msg: "success".to_string(),
        url: path.clone(),
    };
    match serde_json::to_string(&resp) {
        Ok(obj) => HttpResponse::Ok()
            .append_header(("Content-Type", "application/json"))
            .body(obj),
        Err(e) => {
            log::error!("Serialization error: {}", e);
            HttpResponse::InternalServerError()
                .json(serde_json::json!({"msg": "Internal Error", "url": ""}))
        }
    }
}

/// 多文件上传请求结构体
#[derive(Debug, MultipartForm)]
struct UploadLogosReq {
    #[multipart(rename = "files")]
    files: Vec<TempFile>,
}

/// Logo配置项结构体
#[derive(Debug, Serialize, Deserialize, Clone)]
struct LogoConfig {
    url: String,
    name: Vec<String>,
}

/// Logos.json 完整配置结构体
#[derive(Debug, Serialize, Deserialize, Clone)]
struct LogosJsonConfig {
    host: String,
    remote_url2local_images: bool,
    logos: Vec<LogoConfig>,
}

/// 更新 logos.json 文件
fn update_logos_json_file() -> std::io::Result<()> {
    use std::collections::{HashMap, HashSet};
    use std::fs;

    let logos_file_path = format!(".{}", LOGOS_FOLDER);

    let folder = std::path::Path::new(logos_file_path.as_str());

    // 读取现有的配置以保留别名和其他字段
    let mut existing_data: HashMap<String, HashSet<String>> = HashMap::new();

    // 使用 config 模块读取现有配置
    let config: crate::config::logos::LogosConfig = crate::config::logos::get_logos_config();
    let host = config.host;
    let remote_url2local_images = config.remote_url2local_images;
    for item in config.logos {
        existing_data.insert(item.url, item.name.into_iter().collect());
    }
    
    // 尝试解析旧格式进行迁移（如果配置为空）
    if existing_data.is_empty() {
        if let Ok(content) = fs::read_to_string(crate::config::logos::get_logos_file_path()) {
        // 尝试解析旧格式进行迁移
        if let Ok(list) = serde_json::from_str::<Vec<LogoConfig>>(&content) {
            for item in list {
                existing_data.insert(item.url, item.name.into_iter().collect());
            }
        }
        // 尝试解析更旧的 Map 格式 (迁移)
        else if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
            for (name, url) in map {
                existing_data.entry(url).or_default().insert(name);
            }
        }
        }
    }

    let mut final_list: Vec<LogoConfig> = Vec::new();
    let mut processed_urls = HashSet::new();

    if folder.exists() && folder.is_dir() {
        for entry in fs::read_dir(folder)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name == "logos.json" || name.starts_with(".") {
                        continue;
                    }

                    let url = format!("./{}/{}", LOGOS_FOLDER, name)
                        .replace(format!("./{}/", LOGOS_FOLDER).as_str(), LOGOS_FOLDER);
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(name)
                        .to_string();

                    // 获取该URL已有的别名集合，如果不存在则创建
                    let names = existing_data.entry(url.clone()).or_default();
                    // 确保文件名本身作为别名存在
                    names.insert(stem);

                    processed_urls.insert(url.clone());
                }
            }
        }
    }

    // 将处理后的数据转换为 Vec<LogoConfig>
    for url in processed_urls {
        if let Some(names) = existing_data.get(&url) {
            let mut name_list: Vec<String> = names.iter().cloned().collect();
            name_list.sort(); // 排序以便输出稳定
            final_list.push(LogoConfig {
                url,
                name: name_list,
            });
        }
    }

    // 按照 URL 排序
    final_list.sort_by(|a, b| a.url.cmp(&b.url));

    // 使用 config 模块保存
    crate::config::logos::migrate_and_save_logos(existing_data, host, remote_url2local_images)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

/// Logo多文件上传API端点
#[post("/media/upload-logos")]
async fn upload_logos(MultipartForm(form): MultipartForm<UploadLogosReq>) -> impl Responder {
    let mut uploaded_files = Vec::new();

    for file in form.files {
        let file_name = match file.file_name {
            Some(name) => name,
            None => continue,
        };

        let path = format!(".{}{}", LOGOS_FOLDER, file_name);

        if let Err(e) = file.file.persist(path.clone()) {
            log::error!("Failed to save logo {}: {}", file_name, e);
            continue;
        }
        uploaded_files.push(file_name);
    }

    // Update JSON index
    if let Err(e) = update_logos_json_file() {
        log::error!("Failed to update logos json: {}", e);
    }

    HttpResponse::Ok().json(serde_json::json!({"msg": "success", "uploaded": uploaded_files}))
}

/// 获取Logo列表API端点
#[get("/media/logos")]
async fn get_logos_list() -> impl Responder {
    match crate::config::logos::read_logos_json_string() {
        Ok(content) => HttpResponse::Ok()
            .append_header(("Content-Type", "application/json"))
            .body(content),
        Err(_) => {
            let _ = update_logos_json_file();
            match crate::config::logos::read_logos_json_string() {
                Ok(c) => HttpResponse::Ok()
                    .append_header(("Content-Type", "application/json"))
                    .body(c),
                Err(_) => HttpResponse::Ok().json(serde_json::json!([])),
            }
        }
    }
}

/// 更新Logos.json完整配置的请求结构体
#[derive(Debug, Serialize, Deserialize)]
struct UpdateLogosConfigRequest {
    host: String,
    remote_url2local_images: bool,
}

/// 更新Logos.json完整配置API端点
#[post("/media/logos/config")]
async fn update_logos_config(req: web::Json<UpdateLogosConfigRequest>) -> impl Responder {
    // 使用 config 模块更新
    match crate::config::logos::partial_update_logos_config(
        req.host.trim_end_matches('/').to_string(),
        req.remote_url2local_images,
    ) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"msg": "success"})),
        Err(e) => {
            log::error!("Failed to update logos config: {}", e);
            HttpResponse::InternalServerError()
                .json(serde_json::json!({"msg": format!("Failed to save configuration: {}", e)}))
        }
    }
}

/// 更新Logo配置API端点
#[post("/media/logos/update")]
async fn update_logo_config(req: web::Json<LogoConfig>) -> impl Responder {
    // 使用 config 模块更新
    match crate::config::logos::update_logo_names(&req.url, req.name.clone()) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"msg": "success"})),
        Err(e) => {
            if e.contains("not found") {
                HttpResponse::NotFound().json(serde_json::json!({"msg": "Logo not found"}))
            } else {
                log::error!("Failed to update logo names: {}", e);
                HttpResponse::InternalServerError()
                    .json(serde_json::json!({"msg": format!("Failed to save configuration: {}", e)}))
            }
        }
    }
}

/// M3U解析和Logo替换请求结构体
#[derive(Serialize, Deserialize)]
struct QRequest {
    url: String,
    host: String,
}

/// 获取任务内容的请求结构体
#[derive(Serialize, Deserialize)]
struct GetTaskContentRequest {
    task_id: String,
}

/// 任务内容响应项结构体
#[derive(Serialize, Deserialize)]
struct TaskContentItem {
    #[serde(rename = "type")]
    content_type: String,
    content: String,
}

/// 获取任务内容API端点（返回sub和logo两种类型的内容）
#[get("/tasks/get-task-content")]
pub async fn get_task_content(
    task_manager: web::Data<Arc<TaskManager>>,
    req: web::Query<GetTaskContentRequest>,
) -> impl Responder {
    use crate::common::M3uObjectList;
    use std::fs;

    // 1. 获取任务信息
    let task = task_manager.get_task(req.task_id.clone());
    let result_name = match task {
        Some(info) => info.original.get_result_name().to_string(),
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "msg": "Task not found"
            }));
        }
    };

    // 2. 处理路径：如果 result_name 以 / 开头，则添加 . 前缀；否则直接使用
    let file_path = if result_name.starts_with('/') {
        format!(".{}", result_name)
    } else {
        result_name.clone()
    };

    // 3. 获取原始M3U内容（type = "sub"）
    let sub_content = get_file_contents(file_path.clone())
        .unwrap_or_else(|| String::default());

    // 4. 获取处理后的M3U内容（type = "logo"）
    // 使用 config 模块获取 logos 映射
    let logos_map = crate::config::logos::get_logos_map();

    // 读取 M3U 文件
    let m3u_content = match fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "msg": format!("Failed to read M3U file: {}", e)
            }));
        }
    };

    let mut logo_content = String::new();

    let host = crate::config::logos::get_logos_config().host;
    if !host.is_empty() {
        // 解析 M3U 并替换 Logo
        let mut m3u_list = M3uObjectList::from(m3u_content);
        m3u_list.replace_logos(host.clone(), &logos_map);
        logo_content = m3u_list.get_m3u_content();
    }

    let mut response = Vec::new();
    response.push(TaskContentItem {
        content_type: "sub".to_string(),
        content: sub_content,
    });
    if !logo_content.is_empty() {
        response.push(TaskContentItem {
            content_type: "logo".to_string(),
            content: logo_content,
        });
    }

    HttpResponse::Ok().json(response)
}

/// 配置导出请求结构体
#[derive(MultipartForm)]
struct ConfigImportForm {
    file: TempFile,
}

/// 导出系统配置（打包 core 文件夹）
#[get("/system/export")]
async fn system_export_config() -> impl Responder {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let export_dir = format!("{}system", STATIC_FOLDER);
    let export_filename = format!("config_export_{}.zip", timestamp);
    let export_path = format!("{}/{}", export_dir, export_filename);
    
    // 确保导出目录存在
    if let Err(e) = fs::create_dir_all(&export_dir) {
        error!("Failed to create export directory: {}", e);
        return HttpResponse::InternalServerError()
            .json(serde_json::json!({"msg": format!("Failed to create export directory: {}", e)}));
    }
    
    // 创建 ZIP 文件
    let file = match fs::File::create(&export_path) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to create zip file: {}", e);
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({"msg": format!("Failed to create zip file: {}", e)}));
        }
    };
    
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);
    
    // 需要导出的配置文件列表
    let config_files = vec![
        "core/task.json",
        "core/search.json",
        "core/replace.json",
        "core/favourite.json",
        "core/logos.json",
    ];
    
    for file_path in config_files {
        if let Ok(mut file_content) = fs::File::open(file_path) {
            if let Err(e) = zip.start_file(file_path, options) {
                error!("Failed to add {} to zip: {}", file_path, e);
                continue;
            }
            
            let mut buffer = Vec::new();
            if let Err(e) = file_content.read_to_end(&mut buffer) {
                error!("Failed to read {}: {}", file_path, e);
                continue;
            }
            
            if let Err(e) = zip.write_all(&buffer) {
                error!("Failed to write {} to zip: {}", file_path, e);
                continue;
            }
            
            info!("Added {} to export", file_path);
        } else {
            info!("Skipping {} (file not found)", file_path);
        }
    }
    
    if let Err(e) = zip.finish() {
        error!("Failed to finalize zip: {}", e);
        return HttpResponse::InternalServerError()
            .json(serde_json::json!({"msg": format!("Failed to finalize zip: {}", e)}));
    }
    
    info!("Config exported to: {}", export_path);
    
    HttpResponse::Ok().json(serde_json::json!({
        "msg": "success",
        "file": format!("/static/system/{}", export_filename),
        "filename": export_filename
    }))
}

/// 导入系统配置（从 ZIP 文件恢复 core 文件夹）
#[post("/system/import")]
async fn system_import_config(MultipartForm(form): MultipartForm<ConfigImportForm>) -> impl Responder {
    let temp_file_path = form.file.file.path();
    
    // 打开 ZIP 文件
    let file = match fs::File::open(temp_file_path) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to open uploaded file: {}", e);
            return HttpResponse::BadRequest()
                .json(serde_json::json!({"msg": format!("Failed to open uploaded file: {}", e)}));
        }
    };
    
    let mut archive = match ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            error!("Failed to read zip archive: {}", e);
            return HttpResponse::BadRequest()
                .json(serde_json::json!({"msg": "Invalid zip file format"}));
        }
    };
    
    // 验证 ZIP 文件内容
    let expected_files = vec![
        "core/task.json",
        "core/search.json",
        "core/replace.json",
        "core/favourite.json",
        "core/logos.json",
    ];
    
    let mut found_files: HashMap<String, bool> = HashMap::new();
    for name in expected_files.iter() {
        found_files.insert(name.to_string(), false);
    }
    
    // 检查文件是否存在并验证 JSON 格式
    for i in 0..archive.len() {
        let mut file = match archive.by_index(i) {
            Ok(f) => f,
            Err(_) => continue,
        };
        
        let file_name = file.name().to_string();
        
        if expected_files.contains(&file_name.as_str()) {
            found_files.insert(file_name.clone(), true);
            
            // 读取文件内容并验证 JSON 格式
            let mut contents = String::new();
            if let Err(e) = file.read_to_string(&mut contents) {
                error!("Failed to read {} from zip: {}", file_name, e);
                return HttpResponse::BadRequest()
                    .json(serde_json::json!({"msg": format!("Failed to read {} from zip", file_name)}));
            }
            
            // 验证 JSON 格式
            if let Err(e) = serde_json::from_str::<serde_json::Value>(&contents) {
                error!("Invalid JSON format in {}: {}", file_name, e);
                return HttpResponse::BadRequest()
                    .json(serde_json::json!({"msg": format!("Invalid JSON format in {}", file_name)}));
            }
            
            info!("Validated {}", file_name);
        }
    }
    
    // 检查是否至少有一个配置文件
    let has_any_file = found_files.values().any(|&v| v);
    if !has_any_file {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({"msg": "No valid configuration files found in zip"}));
    }
    
    // 创建备份
    let backup_dir = format!("{}system/backup", STATIC_FOLDER);
    let backup_timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_path = format!("{}/config_backup_{}.zip", backup_dir, backup_timestamp);
    
    if let Err(e) = fs::create_dir_all(&backup_dir) {
        error!("Failed to create backup directory: {}", e);
    } else {
        // 备份当前配置
        if let Ok(backup_file) = fs::File::create(&backup_path) {
            let mut backup_zip = ZipWriter::new(backup_file);
            let options = FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            
            for file_path in expected_files.iter() {
                if let Ok(mut file_content) = fs::File::open(*file_path) {
                    let _ = backup_zip.start_file(*file_path, options);
                    let mut buffer = Vec::new();
                    let _ = file_content.read_to_end(&mut buffer);
                    let _ = backup_zip.write_all(&buffer);
                }
            }
            
            let _ = backup_zip.finish();
            info!("Backup created at: {}", backup_path);
        }
    }
    
    // 解压并覆盖配置文件
    let mut imported_files = Vec::new();
    for i in 0..archive.len() {
        let mut file = match archive.by_index(i) {
            Ok(f) => f,
            Err(_) => continue,
        };
        
        let file_name = file.name().to_string();
        
        if expected_files.contains(&file_name.as_str()) {
            let mut contents = String::new();
            if let Err(e) = file.read_to_string(&mut contents) {
                error!("Failed to read {}: {}", file_name, e);
                continue;
            }
            
            // 确保目录存在
            if let Some(parent) = PathBuf::from(&file_name).parent() {
                let _ = fs::create_dir_all(parent);
            }
            
            // 写入文件
            if let Err(e) = fs::write(&file_name, contents) {
                error!("Failed to write {}: {}", file_name, e);
                continue;
            }
            
            imported_files.push(file_name.clone());
            info!("Imported {}", file_name);
        }
    }
    
    // 重新加载配置
    let _ = crate::config::task::reload_task_config();
    let _ = crate::config::search::reload_search_map();
    let _ = crate::config::replace::reload_replace_config();
    let _ = crate::config::favourite::reload_favourite_map();
    let _ = crate::config::logos::reload_logos_map();
    
    info!("Configuration imported successfully");
    
    HttpResponse::Ok().json(serde_json::json!({
        "msg": "success",
        "imported_files": imported_files,
        "backup": format!("/static/system/backup/config_backup_{}.zip", backup_timestamp)
    }))
}

/// M3U解析和Logo替换API端点
#[get("/q")]
async fn q_m3u(req: web::Query<QRequest>) -> impl Responder {
    use crate::common::M3uObjectList;
    use std::fs;

    // 1. 使用 config 模块获取 logos 映射
    let logos_map = crate::config::logos::get_logos_map();

    // 2. 读取 M3U 文件
    let file_path = format!(".{}", &req.url);
    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => return HttpResponse::BadRequest().body(format!("Failed to read file: {}", e)),
    };

    // 3. 解析 M3U
    let mut m3u_list = M3uObjectList::from(content);

    // 4. 替换 Logo
    m3u_list.replace_logos(req.host.to_string(), &logos_map);

    // 5. 生成结果
    let result = m3u_list.get_m3u_content();

    HttpResponse::Ok()
        .append_header((
            "Content-Type",
            "application/vnd.apple.mpegurl; charset=utf-8",
        ))
        .body(result)
}

/// 启动Web服务器
pub async fn start_web(port: u16) {

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
        // 每1小时运行一次，检查
        scheduler.every(1.hour()).run(move || {
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
            let now_time = Local::now().format("%Y%m%d-%H:%M:%s").to_string();
            info!("{}", now_time.clone() + "check task started");
            *locked_flag = true;
            // 获取所有任务
            if let Ok(tasks) = get_all_tasks() {
                for (id, _) in tasks {
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
            info!("{}", now_time.clone() + "check task ended");
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
            .service(system_export_config)
            .service(system_import_config)
            .service(index)
            .service(upload)
            .service(upload_logos)
            .service(get_logos_list)
            .service(update_logos_config)
            .service(update_logo_config)
            .service(q_m3u)
            .service(get_task_content)
            .service(actix_fs::Files::new("/static", STATIC_FOLDER.to_owned()).show_files_listing())
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
            .service(actix_fs::Files::new("/", "./web/"))
            .wrap(Logger::default())
    })
    .workers(16) // 增加工作线程数到 16，避免本地请求死锁
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

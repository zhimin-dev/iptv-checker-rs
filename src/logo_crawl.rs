//! 频道 logo 自动爬取：搜索配置开启后，爬取过程中自动下载频道台标到本地，
//! 并在「频道封面配置」页提供整理区：批量选择封面、绑定频道名后进入正式封面配置列表。

use actix_web::{delete, get, post, web, HttpResponse, Responder};
use futures::StreamExt;
use lazy_static::lazy_static;
use log::info;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub static LOGO_CRAWL_FOLDER: &str = "./static/core/logos_crawled/";
pub static LOGO_CRAWL_URL_BASE: &str = "/static/core/logos_crawled/";
pub static LOGO_CRAWL_FILE: &str = "./static/core/logos_crawled.json";

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CrawledLogo {
    pub id: String,
    pub file: String,
    pub url: String,
    pub source_url: String,
    pub names: Vec<String>,
    pub created_at: u64,
}

lazy_static! {
    static ref CRAWLED_LOGOS: Mutex<Vec<CrawledLogo>> = Mutex::new(load());
}

fn load() -> Vec<CrawledLogo> {
    match std::fs::read_to_string(LOGO_CRAWL_FILE) {
        Ok(s) => serde_json::from_str::<Vec<CrawledLogo>>(&s).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn save() {
    let list = CRAWLED_LOGOS.lock().unwrap().clone();
    if let Ok(json) = serde_json::to_string_pretty(&list) {
        let _ = std::fs::write(LOGO_CRAWL_FILE, json);
    }
}

fn ext_from_content_type(ct: &str) -> &'static str {
    if ct.contains("png") {
        "png"
    } else if ct.contains("jpeg") || ct.contains("jpg") {
        "jpg"
    } else if ct.contains("webp") {
        "webp"
    } else if ct.contains("gif") {
        "gif"
    } else {
        "png"
    }
}

async fn download_logo(url: &str) -> Option<String> {
    let client = crate::common::util::get_http_client();
    let resp = client
        .get(url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !ct.starts_with("image/") {
        return None;
    }
    let bytes = resp.bytes().await.ok()?;
    if bytes.len() < 100 || bytes.len() > 2 * 1024 * 1024 {
        return None;
    }
    let ext = ext_from_content_type(&ct).to_string();
    let file_name = format!("{:x}.{}", md5::compute(url.as_bytes()), ext);
    let path = format!("{}{}", LOGO_CRAWL_FOLDER, file_name);
    std::fs::write(&path, bytes).ok()?;
    Some(file_name)
}

/// 从频道列表爬取 logo（url 去重；每轮最多 200 个；8 并发下载）
pub async fn crawl_logos(m3u: &crate::common::M3uObjectList) {
    let _ = std::fs::create_dir_all(LOGO_CRAWL_FOLDER);
    let mut candidates: Vec<(String, String, String)> = Vec::new();
    {
        let mut seen: HashSet<String> = HashSet::new();
        for obj in m3u.get_list_ref() {
            let Some(ext) = obj.get_extend() else { continue };
            let logo_url = ext.tv_logo.trim().to_string();
            if logo_url.is_empty() || !logo_url.starts_with("http") {
                continue;
            }
            if !seen.insert(logo_url.clone()) {
                continue;
            }
            let id = format!("{:x}", md5::compute(logo_url.as_bytes()));
            let exists = CRAWLED_LOGOS.lock().unwrap().iter().any(|c| c.id == id);
            if exists {
                continue;
            }
            // 文件已存在（如已绑定到正式封面的）也不再重复下载
            let file_exists = ["png", "jpg", "webp", "gif"]
                .iter()
                .any(|ext| std::path::Path::new(&format!("{}{}.{}", LOGO_CRAWL_FOLDER, id, ext)).exists());
            if file_exists {
                continue;
            }
            let name = obj.get_display_name().to_string();
            candidates.push((id, logo_url, name));
            if candidates.len() >= 200 {
                break;
            }
        }
    }
    if candidates.is_empty() {
        return;
    }
    // 8 并发下载
    let results = futures::stream::iter(candidates.into_iter())
        .map(|(id, url, name)| async move {
            match download_logo(&url).await {
                Some(file_name) => Some((id, url, name, file_name)),
                None => None,
            }
        })
        .buffer_unordered(8)
        .collect::<Vec<_>>()
        .await;
    let mut new_items = Vec::new();
    for item in results.into_iter().flatten() {
        let (id, source_url, name, file_name) = item;
        new_items.push(CrawledLogo {
            id,
            file: format!("static/core/logos_crawled/{}", file_name),
            url: format!("{}{}", LOGO_CRAWL_URL_BASE, file_name),
            source_url,
            names: vec![name],
            created_at: now_secs(),
        });
    }
    if !new_items.is_empty() {
        let count = new_items.len();
        let mut list = CRAWLED_LOGOS.lock().unwrap();
        list.extend(new_items);
        if list.len() > 500 {
            let drain = list.len() - 500;
            list.drain(0..drain);
        }
        save();
        info!("crawled {} channel logos", count);
    }
}

// ============================== API ==============================

#[get("/media/logos-crawled")]
async fn get_crawled_logos_api() -> impl Responder {
    let list = CRAWLED_LOGOS.lock().unwrap().clone();
    let total = list.len();
    HttpResponse::Ok().json(serde_json::json!({ "list": list, "total": total }))
}

#[derive(Deserialize)]
pub struct BindLogoReq {
    pub ids: Vec<String>,
    pub names: Vec<String>,
}

/// 批量绑定：选中若干爬取封面，绑定到频道名，进入正式封面配置列表（logos.json）
#[post("/media/logos-crawled/bind")]
async fn bind_crawled_logos_api(req: web::Json<BindLogoReq>) -> impl Responder {
    let names: Vec<String> = req
        .names
        .iter()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect();
    if names.is_empty() || req.ids.is_empty() {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({ "msg": "ids and names are required" }));
    }
    let ids: HashSet<String> = req.ids.iter().cloned().collect();
    let mut bound = 0;
    {
        let mut crawled = CRAWLED_LOGOS.lock().unwrap();
        let mut cfg = crate::config::logos::get_logos_config();
        for item in crawled.iter().filter(|c| ids.contains(&c.id)) {
            if let Some(existing) = cfg.logos.iter_mut().find(|l| l.url == item.url) {
                for n in &names {
                    if !existing.name.contains(n) {
                        existing.name.push(n.clone());
                    }
                }
            } else {
                cfg.logos.push(crate::config::logos::LogoItem {
                    url: item.url.clone(),
                    name: names.clone(),
                });
            }
            bound += 1;
        }
        if bound > 0 {
            let _ = crate::config::logos::update_logos_config(cfg);
        }
        crawled.retain(|c| !ids.contains(&c.id));
        save();
    }
    info!("bound {} crawled logos to channels", bound);
    HttpResponse::Ok().json(serde_json::json!({ "msg": "bound", "count": bound }))
}

#[delete("/media/logos-crawled/{id}")]
async fn delete_crawled_logo_api(path: web::Path<String>) -> impl Responder {
    let id = path.into_inner();
    let removed = {
        let mut list = CRAWLED_LOGOS.lock().unwrap();
        let idx = list.iter().position(|c| c.id == id);
        idx.map(|i| list.remove(i))
    };
    match removed {
        Some(item) => {
            let _ = std::fs::remove_file(format!("./{}", item.file));
            save();
            HttpResponse::Ok().json(serde_json::json!({ "msg": "deleted", "id": id }))
        }
        None => HttpResponse::NotFound().json(serde_json::json!({ "msg": "not found" })),
    }
}

#[delete("/media/logos-crawled")]
async fn clear_crawled_logos_api() -> impl Responder {
    let count = {
        let mut list = CRAWLED_LOGOS.lock().unwrap();
        let count = list.len();
        list.clear();
        count
    };
    save();
    // 删除爬取文件（已绑定的文件在 logos.json 中仍被引用，仅清理未绑定的索引文件）
    if let Ok(entries) = std::fs::read_dir(LOGO_CRAWL_FOLDER) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    info!("cleared {} crawled logos", count);
    HttpResponse::Ok().json(serde_json::json!({ "msg": "cleared", "count": count }))
}

/// 注册路由
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(get_crawled_logos_api)
        .service(bind_crawled_logos_api)
        .service(delete_crawled_logo_api)
        .service(clear_crawled_logos_api);
}

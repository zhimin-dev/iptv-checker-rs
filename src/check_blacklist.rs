//! 检查源黑名单：同一源连续失败 N 次后加入黑名单，下次检查直接过滤，提升检查速度。
//! 支持一键清理与定时自动清理（过期失效，防止误伤后来恢复的源）。

use actix_web::{delete, get, post, web, HttpResponse, Responder};
use lazy_static::lazy_static;
use log::info;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub static CHECK_BLACKLIST_FILE: &str = "./static/core/check_blacklist.json";

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BlacklistEntry {
    pub fail_count: u32,
    pub first_fail_at: u64,
    pub last_fail_at: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct BlacklistFile {
    #[serde(default)]
    items: HashMap<String, BlacklistEntry>,
}

lazy_static! {
    static ref BLACKLIST: Mutex<HashMap<String, BlacklistEntry>> = Mutex::new(load());
}

fn load() -> HashMap<String, BlacklistEntry> {
    match std::fs::read_to_string(CHECK_BLACKLIST_FILE) {
        Ok(s) => serde_json::from_str::<BlacklistFile>(&s)
            .map(|f| f.items)
            .unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn save() {
    let items = BLACKLIST.lock().unwrap().clone();
    let file = BlacklistFile { items };
    if let Ok(json) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(CHECK_BLACKLIST_FILE, json);
    }
}

/// 失败阈值（连续失败 N 次加入黑名单），从 base.json 读取
pub fn get_fail_times() -> u32 {
    crate::config::base::get_base_config()
        .check_blacklist_fail_times
        .max(1)
}

/// 自动清理天数（超过该天数未失败自动移出黑名单）
pub fn get_auto_clean_days() -> u64 {
    crate::config::base::get_base_config()
        .check_blacklist_auto_clean_days
        .max(1) as u64
}

/// 该 url 是否已被拉黑（连续失败次数达到阈值）
pub fn is_blacklisted(url: &str) -> bool {
    let map = BLACKLIST.lock().unwrap();
    map.get(url.trim())
        .map(|e| e.fail_count >= get_fail_times())
        .unwrap_or(false)
}

/// 当前生效的黑名单 url 集合（用于检查前过滤）
pub fn get_blacklisted_urls() -> HashSet<String> {
    let threshold = get_fail_times();
    let map = BLACKLIST.lock().unwrap();
    map.iter()
        .filter(|(_, e)| e.fail_count >= threshold)
        .map(|(u, _)| u.clone())
        .collect()
}

/// 检查成功：移出黑名单（源恢复了）
pub fn mark_success(url: &str) {
    let u = url.trim().to_string();
    if u.is_empty() {
        return;
    }
    let removed = BLACKLIST.lock().unwrap().remove(&u).is_some();
    if removed {
        save();
    }
}

/// 检查失败：累计失败次数，达到阈值即拉黑
pub fn mark_failed(url: &str) {
    let u = url.trim().to_string();
    if u.is_empty() {
        return;
    }
    let now = now_secs();
    {
        let mut map = BLACKLIST.lock().unwrap();
        let entry = map.entry(u.clone()).or_insert(BlacklistEntry {
            fail_count: 0,
            first_fail_at: now,
            last_fail_at: now,
        });
        entry.fail_count += 1;
        entry.last_fail_at = now;
        // 防止条目无限膨胀
        if map.len() > 5000 {
            let oldest = map
                .iter()
                .min_by_key(|(_, e)| e.last_fail_at)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                map.remove(&k);
            }
        }
    }
    save();
}

/// 定时自动清理：超过 auto_clean_days 天没有失败的条目移出（防止误伤恢复的源）
pub fn cleanup_expired() -> usize {
    let days = get_auto_clean_days();
    let expire_before = now_secs().saturating_sub(days * 24 * 3600);
    let removed: Vec<String> = {
        let map = BLACKLIST.lock().unwrap();
        map.iter()
            .filter(|(_, e)| e.last_fail_at < expire_before)
            .map(|(u, _)| u.clone())
            .collect()
    };
    if !removed.is_empty() {
        let mut map = BLACKLIST.lock().unwrap();
        for u in &removed {
            map.remove(u);
        }
        save();
        info!("check blacklist auto-clean removed {} entries", removed.len());
    }
    removed.len()
}

/// 一键清理
pub fn clear_all() -> usize {
    let count = BLACKLIST.lock().unwrap().len();
    BLACKLIST.lock().unwrap().clear();
    save();
    info!("check blacklist cleared {} entries", count);
    count
}

/// 黑名单列表（含是否已生效）
pub fn list() -> Vec<serde_json::Value> {
    let threshold = get_fail_times();
    let map = BLACKLIST.lock().unwrap();
    let mut items: Vec<(&String, &BlacklistEntry)> = map.iter().collect();
    items.sort_by(|a, b| b.1.last_fail_at.cmp(&a.1.last_fail_at));
    items
        .into_iter()
        .map(|(u, e)| {
            serde_json::json!({
                "url": u,
                "fail_count": e.fail_count,
                "blacklisted": e.fail_count >= threshold,
                "first_fail_at": e.first_fail_at,
                "last_fail_at": e.last_fail_at,
            })
        })
        .collect()
}

// ============================== API ==============================

#[derive(Deserialize)]
pub struct BlacklistQuery {
    #[serde(default)]
    pub page: usize,
    #[serde(default)]
    pub page_size: usize,
}

#[get("/api/check/blacklist")]
async fn get_blacklist_api(q: web::Query<BlacklistQuery>) -> impl Responder {
    let list = list();
    let total = list.len();
    let size = if q.page_size > 0 { q.page_size } else { 50 };
    let paged: Vec<serde_json::Value> = list
        .into_iter()
        .skip(q.page.saturating_mul(size))
        .take(size)
        .collect();
    HttpResponse::Ok().json(serde_json::json!({
        "list": paged,
        "total": total,
        "page": q.page,
        "page_size": size,
        "fail_times": get_fail_times(),
        "auto_clean_days": get_auto_clean_days(),
    }))
}

#[delete("/api/check/blacklist")]
async fn clear_blacklist_api() -> impl Responder {
    let count = clear_all();
    HttpResponse::Ok().json(serde_json::json!({ "msg": "cleared", "count": count }))
}

#[derive(Deserialize)]
pub struct BlacklistConfigReq {
    pub fail_times: u32,
    pub auto_clean_days: u32,
}

#[get("/api/check/blacklist/config")]
async fn get_blacklist_config_api() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "fail_times": get_fail_times(),
        "auto_clean_days": get_auto_clean_days(),
    }))
}

#[post("/api/check/blacklist/config")]
async fn set_blacklist_config_api(req: web::Json<BlacklistConfigReq>) -> impl Responder {
    let fail_times = req.fail_times.clamp(1, 100);
    let auto_clean_days = req.auto_clean_days.clamp(1, 365);
    let mut cfg = crate::config::base::get_base_config();
    cfg.check_blacklist_fail_times = fail_times;
    cfg.check_blacklist_auto_clean_days = auto_clean_days;
    match crate::config::base::update_base_config(cfg) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "fail_times": fail_times,
            "auto_clean_days": auto_clean_days,
            "msg": "success",
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({ "msg": e })),
    }
}

/// 注册路由
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(get_blacklist_api)
        .service(clear_blacklist_api)
        .service(get_blacklist_config_api)
        .service(set_blacklist_config_api);
}


//! 统一频道图标配置：把「频道图标 + 分组 + tvg-id」合并为一组配置。
//! 每一项对应一个频道：主名称、别名、tvg-id（EPG 匹配）、分组、图标地址。
//! 检查链路中：图标匹配（logo）、分组映射、tvg-id 都从这份配置读取。

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::RwLock;

pub static CHANNEL_ICONS_FILE: &str = "./static/core/channel_icons.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelIconItem {
    /// 主频道名（tvg-name / 展示名）
    pub name: String,
    /// 别名列表
    #[serde(default)]
    pub aliases: Vec<String>,
    /// 映射到 tvg-id（EPG 节目单匹配），可为空
    #[serde(default)]
    pub tvg_id: String,
    /// 分组（group-title）
    #[serde(default)]
    pub group: String,
    /// 图标地址
    #[serde(default)]
    pub logo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelIconsConfig {
    #[serde(default)]
    pub items: Vec<ChannelIconItem>,
}

static CHANNEL_ICONS: Lazy<RwLock<ChannelIconsConfig>> = Lazy::new(|| {
    RwLock::new(read_channel_icons())
});

fn read_channel_icons() -> ChannelIconsConfig {
    match fs::read_to_string(CHANNEL_ICONS_FILE) {
        Ok(s) => serde_json::from_str::<ChannelIconsConfig>(&s).unwrap_or_else(|_| migrate_from_logos()),
        Err(_) => migrate_from_logos(),
    }
}

/// 首次使用时从旧 logos.json 迁移：每个 LogoItem（url + 别名列表）生成一组条目
fn migrate_from_logos() -> ChannelIconsConfig {
    let cfg = crate::config::logos::get_logos_config();
    let mut items: Vec<ChannelIconItem> = Vec::new();
    for l in &cfg.logos {
        if l.name.is_empty() {
            continue;
        }
        items.push(ChannelIconItem {
            name: l.name[0].clone(),
            aliases: l.name.iter().skip(1).cloned().collect(),
            tvg_id: String::new(),
            group: String::new(),
            logo: l.url.clone(),
        });
    }
    let config = ChannelIconsConfig { items };
    if let Ok(json) = serde_json::to_string_pretty(&config) {
        let _ = fs::write(CHANNEL_ICONS_FILE, json);
    }
    config
}

pub fn get_channel_icons() -> ChannelIconsConfig {
    CHANNEL_ICONS.read().unwrap().clone()
}

/// 全量保存
pub fn save_channel_icons(items: Vec<ChannelIconItem>) -> Result<(), String> {
    let config = ChannelIconsConfig { items };
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("serialize failed: {}", e))?;
    fs::write(CHANNEL_ICONS_FILE, json).map_err(|e| format!("write failed: {}", e))?;
    *CHANNEL_ICONS.write().unwrap() = config;
    Ok(())
}

/// 添加或更新一个条目（按 name 匹配）。
/// 更新时保留已有条目的 tvg_id / 分组（仅当新值为空时不覆盖），避免绑定图标时冲掉人工配置。
pub fn upsert_item(item: ChannelIconItem) {
    let mut config = get_channel_icons();
    if let Some(existing) = config
        .items
        .iter_mut()
        .find(|i| i.name.eq_ignore_ascii_case(&item.name))
    {
        existing.aliases = item.aliases;
        existing.logo = item.logo;
        if !item.tvg_id.is_empty() {
            existing.tvg_id = item.tvg_id;
        }
        if !item.group.is_empty() {
            existing.group = item.group;
        }
    } else {
        config.items.push(item);
    }
    let _ = save_channel_icons(config.items);
}

pub fn remove_item(name: &str) -> bool {
    let mut config = get_channel_icons();
    let before = config.items.len();
    config.items.retain(|i| !i.name.eq_ignore_ascii_case(name));
    if config.items.len() != before {
        let _ = save_channel_icons(config.items);
        true
    } else {
        false
    }
}

/// 图标匹配表：主名称/别名/tvg-id（小写）→ 图标地址
pub fn get_logo_map() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for item in get_channel_icons().items {
        if item.logo.trim().is_empty() {
            continue;
        }
        let logo = item.logo.clone();
        for key in item_name_keys(&item) {
            map.entry(key).or_insert_with(|| logo.clone());
        }
    }
    map
}

/// 该项的所有匹配键（小写）：主名称、别名、tvg-id
fn item_name_keys(item: &ChannelIconItem) -> Vec<String> {
    let mut keys = Vec::new();
    let mut push = |s: &str| {
        let t = s.trim().to_lowercase();
        if !t.is_empty() {
            keys.push(t);
        }
    };
    push(&item.name);
    for a in &item.aliases {
        push(a);
    }
    push(&item.tvg_id);
    keys
}

/// 按频道名（tvg-name / 展示名）查找统一配置项
pub fn find_for_channel(channel_name: &str) -> Option<ChannelIconItem> {
    let target = channel_name.trim().to_lowercase();
    if target.is_empty() {
        return None;
    }
    get_channel_icons()
        .items
        .into_iter()
        .find(|i| item_name_keys(i).iter().any(|k| k == &target))
}

/// 统一配置中的分组映射（按频道名匹配）
pub fn get_group_for_channel(channel_name: &str) -> Option<String> {
    let item = find_for_channel(channel_name)?;
    if item.group.trim().is_empty() {
        None
    } else {
        Some(item.group.trim().to_string())
    }
}

/// 统一配置中的 tvg-id（按频道名匹配）
pub fn get_tvg_id_for_channel(channel_name: &str) -> Option<String> {
    let item = find_for_channel(channel_name)?;
    if item.tvg_id.trim().is_empty() {
        None
    } else {
        Some(item.tvg_id.trim().to_string())
    }
}

/// 图标地址是否已在统一配置中（爬取列表过滤用）
pub fn logo_exists(logo_url: &str) -> bool {
    get_channel_icons()
        .items
        .iter()
        .any(|i| i.logo == logo_url)
}

/// 重新加载（配置导入后调用）
pub fn reload() {
    *CHANNEL_ICONS.write().unwrap() = read_channel_icons();
}

/// 外部模块读取文件路径用
pub fn get_file_path() -> &'static str {
    CHANNEL_ICONS_FILE
}

#[allow(dead_code)]
fn _path_guard(_p: &Path) {}

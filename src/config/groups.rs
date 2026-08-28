//! 两级分组定义（分组编辑页专用）：显式维护「分组1-分组2」树。
//! 与频道图标统一配置（channel_icons.json）互通：
//! - get_groups 会把频道项上已有的分组并入结果，保证频道上存在的分组一定可见；
//! - delete_group 可选同步清除频道项上的对应分组字段。

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::sync::RwLock;

pub static GROUPS_FILE: &str = "./static/core/groups.json";

/// 两级分组定义：group1 必填，group2 可为空（空表示仅一级分组）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroupDef {
    #[serde(default)]
    pub group1: String,
    #[serde(default)]
    pub group2: String,
}

impl GroupDef {
    pub fn key(&self) -> (String, String) {
        (self.group1.trim().to_string(), self.group2.trim().to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GroupsConfig {
    #[serde(default)]
    pub groups: Vec<GroupDef>,
}

static GROUPS: Lazy<RwLock<GroupsConfig>> = Lazy::new(|| {
    RwLock::new(read_groups())
});

fn read_groups() -> GroupsConfig {
    match fs::read_to_string(GROUPS_FILE) {
        Ok(s) => serde_json::from_str::<GroupsConfig>(&s).unwrap_or_default(),
        Err(_) => GroupsConfig::default(),
    }
}

/// 分组列表 = 显式定义 ∪ 频道项上已有的分组（去重 + 排序）
pub fn get_groups() -> Vec<GroupDef> {
    let mut list: Vec<GroupDef> = GROUPS.read().unwrap().groups.clone();
    for item in crate::config::channel_icons::get_channel_icons().items {
        let g1 = item.group1.trim().to_string();
        if g1.is_empty() {
            continue;
        }
        let top = GroupDef { group1: g1.clone(), group2: String::new() };
        if !list.contains(&top) {
            list.push(top);
        }
        let g2 = item.group2.trim().to_string();
        if !g2.is_empty() {
            let def = GroupDef { group1: g1, group2: g2 };
            if !list.contains(&def) {
                list.push(def);
            }
        }
    }
    list.retain(|g| !g.group1.trim().is_empty());
    list.sort_by(|a, b| {
        (a.group1.trim().to_lowercase(), a.group2.trim().to_lowercase())
            .cmp(&(b.group1.trim().to_lowercase(), b.group2.trim().to_lowercase()))
    });
    let mut seen: HashSet<(String, String)> = HashSet::new();
    list.retain(|g| seen.insert(g.key()));
    list
}

/// 全量保存显式分组定义
pub fn save_groups(groups: Vec<GroupDef>) -> Result<(), String> {
    let mut list: Vec<GroupDef> = groups
        .into_iter()
        .filter(|g| !g.group1.trim().is_empty())
        .map(|g| GroupDef { group1: g.group1.trim().to_string(), group2: g.group2.trim().to_string() })
        .collect();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    list.retain(|g| seen.insert(g.key()));
    list.sort_by(|a, b| {
        (a.group1.trim().to_lowercase(), a.group2.trim().to_lowercase())
            .cmp(&(b.group1.trim().to_lowercase(), b.group2.trim().to_lowercase()))
    });
    let config = GroupsConfig { groups: list };
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("serialize failed: {}", e))?;
    fs::write(GROUPS_FILE, json).map_err(|e| format!("write failed: {}", e))?;
    *GROUPS.write().unwrap() = config;
    Ok(())
}

/// 删除分组定义。
/// - group2 为空：删除该 group1 下的所有定义；
/// - clear_channels：同步清除频道项上匹配的分组字段（group2 为空时清空 group1+group2）。
/// 返回是否删除了任何定义。
pub fn delete_group(group1: &str, group2: &str, clear_channels: bool) -> Result<bool, String> {
    let g1 = group1.trim().to_string();
    if g1.is_empty() {
        return Err("group1 is required".into());
    }
    let g2 = group2.trim().to_string();

    let mut cfg = GROUPS.read().unwrap().clone();
    let before = cfg.groups.len();
    cfg.groups.retain(|g| {
        if g.group1.trim() != g1 {
            return true;
        }
        if g2.is_empty() {
            return false; // 删除该 group1 下所有定义
        }
        g.group2.trim() != g2
    });
    let removed = cfg.groups.len() != before;
    let json = serde_json::to_string_pretty(&cfg)
        .map_err(|e| format!("serialize failed: {}", e))?;
    fs::write(GROUPS_FILE, json).map_err(|e| format!("write failed: {}", e))?;
    *GROUPS.write().unwrap() = cfg;

    if clear_channels {
        let mut icons = crate::config::channel_icons::get_channel_icons();
        let mut changed = false;
        for item in icons.items.iter_mut() {
            if item.group1.trim() == g1 {
                if g2.is_empty() {
                    if !item.group1.trim().is_empty() || !item.group2.trim().is_empty() {
                        changed = true;
                    }
                    item.group1 = String::new();
                    item.group2 = String::new();
                } else if item.group2.trim() == g2 {
                    item.group2 = String::new();
                    changed = true;
                }
            }
        }
        if changed {
            crate::config::channel_icons::save_channel_icons(icons.items)?;
        }
    }
    Ok(removed)
}

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EpgMapping {
    pub name: String,
    pub channel: String,
    pub source: String,
}

pub static EPG_MAPPINGS: Lazy<HashMap<String, Vec<EpgMapping>>> = Lazy::new(|| {
    let mut map: HashMap<String, Vec<EpgMapping>> = HashMap::new();
    
    // Load the JSON file at compile time and parse it
    let json_data = include_str!("assets/epg_mapping.json");
    
    match serde_json::from_str::<Vec<EpgMapping>>(json_data) {
        Ok(mappings) => {
            for mapping in mappings {
                map.entry(mapping.name.clone())
                    .or_default()
                    .push(mapping);
            }
            log::info!("Loaded {} unique channel names into EPG mapping", map.len());
        }
        Err(e) => {
            log::error!("Failed to parse epg_mapping.json: {}", e);
        }
    }
    
    map
});

static QUALITY_SUFFIX_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)[\s_]*([\[\(]?(?:4K|8K|2K|1080P|720P|480P|360P|240P|HD|FHD|UHD|SD|HEVC|HDR)[\]\)]?|_\d+M\d*)$",
    )
    .unwrap()
});

/// 匹配【】（）() [] 等括号包裹的块（如【台】、（HD）），供规范化时剥除
static BRACKET_BLOCK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[【\[\(（][^【\]\)（）]*[】\]\)）]").unwrap()
});

/// 规范化频道/EPG 名称用于模糊匹配：
/// 繁体转简体、转小写、全角转半角、去掉空白与 - _ · . 等分隔符、剥除【台】等括号后缀块。
/// 例：「CCTV-1 综合【台】」与「CCTV1综合」规范化后一致。
pub fn normalize_epg_name(raw: &str) -> String {
    let simp = crate::common::translate::trad_to_simp(raw);
    // 全角转半角（含全角空格）
    let half: String = simp
        .chars()
        .map(|c| {
            let code = c as u32;
            if code == 0x3000 {
                ' '
            } else if (0xFF01..=0xFF5E).contains(&code) {
                char::from_u32(code - 0xFEE0).unwrap_or(c)
            } else {
                c
            }
        })
        .collect();
    let mut s = half.to_lowercase();
    // 反复剥除括号包裹的块（处理嵌套）
    loop {
        let before = s.clone();
        s = BRACKET_BLOCK_RE.replace_all(&s, "").to_string();
        if s == before {
            break;
        }
    }
    // 去掉空白与常见分隔符
    s.chars()
        .filter(|c| {
            !c.is_whitespace()
                && !matches!(
                    c,
                    '-' | '_' | '·' | '•' | '.' | ',' | '，' | '。' | ':' | '：'
                )
        })
        .collect()
}

/// 规范化后的 EPG 映射索引：normalize(name) -> 原始映射列表
pub static EPG_MAPPINGS_NORM: Lazy<HashMap<String, Vec<EpgMapping>>> = Lazy::new(|| {
    let mut map: HashMap<String, Vec<EpgMapping>> = HashMap::new();
    for mappings in EPG_MAPPINGS.values() {
        for m in mappings {
            let key = normalize_epg_name(&m.name);
            if key.is_empty() {
                continue;
            }
            map.entry(key).or_default().push(m.clone());
        }
    }
    log::info!("Normalized EPG mapping index: {} entries", map.len());
    map
});

/// Match a channel name against the EPG mapping, handling quality suffixes.
/// Returns `Some((epg_name, epg_channel_id))` on match, `None` otherwise.
///
/// Three-tier matching:
/// 1. Exact match against EPG keys
/// 2. Strip known quality suffixes from end, then exact match
/// 3. Longest-prefix match (EPG key is prefix of channel name)
///
/// Within each tier, source priority is zh > cn > hk > tw.
pub fn match_epg_channel(raw_name: &str) -> Option<(String, String)> {
    if raw_name.is_empty() {
        return None;
    }

    let priorities = ["zh", "cn", "hk", "tw"];

    let lookup = |name: &str| -> Option<(String, String)> {
        if let Some(mappings) = EPG_MAPPINGS.get(name) {
            for priority in priorities.iter() {
                if let Some(m) = mappings.iter().find(|m| m.source == *priority) {
                    return Some((m.name.clone(), m.channel.clone()));
                }
            }
            if let Some(m) = mappings.first() {
                return Some((m.name.clone(), m.channel.clone()));
            }
        }
        None
    };

    // Tier 1: exact match
    if let Some(result) = lookup(raw_name) {
        return Some(result);
    }

    // Tier 2: strip quality suffix, exact match
    let stripped = QUALITY_SUFFIX_RE.replace(raw_name, "");
    if stripped != raw_name {
        if let Some(result) = lookup(&stripped) {
            return Some(result);
        }
    }

    // Tier 3: 规范化模糊匹配（去横线/空格/【台】等后缀后精确匹配）
    let lookup_norm = |name: &str| -> Option<(String, String)> {
        let key = normalize_epg_name(name);
        if key.is_empty() {
            return None;
        }
        if let Some(mappings) = EPG_MAPPINGS_NORM.get(&key) {
            for priority in priorities.iter() {
                if let Some(m) = mappings.iter().find(|m| m.source == *priority) {
                    return Some((m.name.clone(), m.channel.clone()));
                }
            }
            if let Some(m) = mappings.first() {
                return Some((m.name.clone(), m.channel.clone()));
            }
        }
        None
    };
    if let Some(result) = lookup_norm(raw_name) {
        return Some(result);
    }
    if stripped != raw_name {
        if let Some(result) = lookup_norm(&stripped) {
            return Some(result);
        }
    }

    // Tier 4: longest-prefix fallback
    let mut best: Option<(String, String)> = None;
    let mut best_len = 0usize;
    for (epg_name, mappings) in EPG_MAPPINGS.iter() {
        let epg_len = epg_name.chars().count();
        if epg_len > best_len && epg_len < raw_name.chars().count() && raw_name.starts_with(epg_name.as_str()) {
            for priority in priorities.iter() {
                if let Some(m) = mappings.iter().find(|m| m.source == *priority) {
                    best = Some((m.name.clone(), m.channel.clone()));
                    best_len = epg_len;
                    break;
                }
            }
            if best_len != epg_len {
                if let Some(m) = mappings.first() {
                    best = Some((m.name.clone(), m.channel.clone()));
                    best_len = epg_len;
                }
            }
        }
    }
    best
}

pub fn get_best_tvg_id(tv_name: Option<&str>, display_name: &str) -> String {
    // Priority order: zh/cn -> hk -> tw
    let priorities = ["zh", "cn", "hk", "tw"];
    
    let lookup_and_match = |name: &str| -> Option<String> {
        if let Some(mappings) = EPG_MAPPINGS.get(name) {
            // Try to find a match based on priority
            for priority in priorities.iter() {
                if let Some(mapping) = mappings.iter().find(|m| m.source == *priority) {
                    return Some(mapping.channel.clone());
                }
            }
            // If no priority match, return the first available
            if let Some(mapping) = mappings.first() {
                return Some(mapping.channel.clone());
            }
        }
        None
    };

    // 1. Try tv_name if provided
    if let Some(name) = tv_name {
        if let Some(id) = lookup_and_match(name) {
            return id;
        }
    }

    // 2. Fallback to display_name
    if let Some(id) = lookup_and_match(display_name) {
        return id;
    }

    // 3. 规范化模糊匹配（去横线/空格/【台】等后缀）
    let lookup_norm = |name: &str| -> Option<String> {
        let key = normalize_epg_name(name);
        if key.is_empty() {
            return None;
        }
        if let Some(mappings) = EPG_MAPPINGS_NORM.get(&key) {
            for priority in priorities.iter() {
                if let Some(mapping) = mappings.iter().find(|m| m.source == *priority) {
                    return Some(mapping.channel.clone());
                }
            }
            if let Some(mapping) = mappings.first() {
                return Some(mapping.channel.clone());
            }
        }
        None
    };
    if let Some(name) = tv_name {
        if let Some(id) = lookup_norm(name) {
            return id;
        }
    }
    if let Some(id) = lookup_norm(display_name) {
        return id;
    }

    // 4. Final fallback: use display_name as id
    display_name.to_string()
}

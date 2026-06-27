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

    // Tier 3: longest-prefix fallback
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

    // 3. Final fallback: use display_name as id
    display_name.to_string()
}

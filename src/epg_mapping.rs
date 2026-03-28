use once_cell::sync::Lazy;
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

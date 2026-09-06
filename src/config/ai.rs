//! DeepSeek AI 整理配置：用户填写的 API Key / Base URL / 模型，用于「AI 整理」功能。

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::RwLock;

pub static AI_CONFIG_FILE: &str = "./static/core/ai.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_model")]
    pub model: String,
}

fn default_base_url() -> String {
    "https://api.deepseek.com".to_string()
}

fn default_model() -> String {
    "deepseek-chat".to_string()
}

impl Default for AiConfig {
    fn default() -> Self {
        AiConfig {
            api_key: String::new(),
            base_url: default_base_url(),
            model: default_model(),
        }
    }
}

static AI_CONFIG: Lazy<RwLock<AiConfig>> = Lazy::new(|| RwLock::new(read_ai_config()));

fn read_ai_config() -> AiConfig {
    match fs::read_to_string(AI_CONFIG_FILE) {
        Ok(s) => serde_json::from_str::<AiConfig>(&s).unwrap_or_default(),
        Err(_) => AiConfig::default(),
    }
}

pub fn get_ai_config() -> AiConfig {
    AI_CONFIG.read().unwrap().clone()
}

pub fn save_ai_config(config: AiConfig) -> Result<(), String> {
    let mut c = config;
    if c.base_url.trim().is_empty() {
        c.base_url = default_base_url();
    }
    if c.model.trim().is_empty() {
        c.model = default_model();
    }
    c.base_url = c.base_url.trim().trim_end_matches('/').to_string();
    let json = serde_json::to_string_pretty(&c).map_err(|e| format!("serialize failed: {}", e))?;
    fs::write(AI_CONFIG_FILE, json).map_err(|e| format!("write failed: {}", e))?;
    *AI_CONFIG.write().unwrap() = c;
    Ok(())
}

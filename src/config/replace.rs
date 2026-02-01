use crate::r#const::constant::{REPLACE_JSON, REPLACE_TXT_CONTENT};
use crate::utils::file_exists;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::RwLock;

/// Replace配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceConfig {
    #[serde(rename = "replace_string")]
    pub replace_string: bool,
    #[serde(rename = "replace_map")]
    pub replace_map: HashMap<String, String>,
}

impl ReplaceConfig {
    fn new() -> Self {
        ReplaceConfig {
            replace_string: false,
            replace_map: HashMap::new(),
        }
    }
}

/// 全局替换配置
static REPLACE_MAP: Lazy<RwLock<ReplaceConfig>> = Lazy::new(|| {
    let p = Path::new(REPLACE_JSON);
    RwLock::new(read_replace_json(p))
});

/// 获取替换配置（用于读取）
pub fn get_replace_config_for_api() -> ReplaceConfig {
    REPLACE_MAP.read().unwrap().clone()
}

/// 保存替换配置到文件
fn save_replace_to_file() -> Result<(), String> {
    let config = REPLACE_MAP.read().unwrap();
    let json = serde_json::to_string_pretty(&*config)
        .map_err(|e| format!("Failed to serialize replace config: {}", e))?;
    fs::write(REPLACE_JSON, json)
        .map_err(|e| format!("Failed to write replace config: {}", e))?;
    Ok(())
}

/// 更新替换配置（立即生效，无需重启）
pub fn update_replace_config(config: ReplaceConfig) -> Result<(), String> {
    let mut map = REPLACE_MAP.write().unwrap();
    *map = config;
    drop(map);
    save_replace_to_file()
}

/// 重新加载配置文件
pub fn reload_replace_config() -> Result<(), String> {
    let p = Path::new(REPLACE_JSON);
    let new_config = read_replace_json(p);
    let mut map = REPLACE_MAP.write().unwrap();
    *map = new_config;
    Ok(())
}

/// 部分更新替换配置（用于 API 更新）
pub fn partial_update_replace_config(
    replace_string: bool,
    replace_map: HashMap<String, String>,
) -> Result<(), String> {
    let mut map = REPLACE_MAP.write().unwrap();
    
    
    map.replace_string = replace_string;
    
    map.replace_map = replace_map;
    drop(map);
    
    save_replace_to_file()
}

/// 添加替换规则
pub fn add_replace_rule(key: String, value: String) -> Result<(), String> {
    let mut map = REPLACE_MAP.write().unwrap();
    map.replace_map.insert(key, value);
    drop(map);
    save_replace_to_file()
}

/// 删除替换规则
pub fn remove_replace_rule(key: &str) -> Result<(), String> {
    let mut map = REPLACE_MAP.write().unwrap();
    map.replace_map.remove(key);
    drop(map);
    save_replace_to_file()
}

/// 启用/禁用字符串替换
pub fn set_replace_enabled(enabled: bool) -> Result<(), String> {
    let mut map = REPLACE_MAP.write().unwrap();
    map.replace_string = enabled;
    drop(map);
    save_replace_to_file()
}

pub fn create_replace_file() {
    if !file_exists(&REPLACE_JSON.to_string()) {
        // 确保 core 目录存在
        if let Some(parent) = std::path::Path::new(REPLACE_JSON).parent() {
            fs::create_dir_all(parent).expect(&format!("Failed to create directory: {:?}", parent));
        }
        let mut fd = fs::File::create(REPLACE_JSON).expect(&format!(
            "Failed to create file: {}",
            REPLACE_JSON.to_string()
        ));
        fd.write_all(REPLACE_TXT_CONTENT.to_string().as_bytes())
            .expect(&format!(
                "Failed to write file: {}",
                REPLACE_JSON.to_string()
            ));
        fd.flush().expect(&format!(
            "Failed to flush file: {}",
            REPLACE_JSON.to_string()
        ));
    }
}

/// 尝试从指定路径读取 JSON 并解析为 ReplaceConfig，若失败返回默认配置
fn read_replace_json<P: AsRef<Path>>(path: P) -> ReplaceConfig {
    match fs::read_to_string(&path) {
        Ok(s) => match serde_json::from_str::<ReplaceConfig>(&s) {
            Ok(config) => config,
            Err(e) => {
                eprintln!(
                    "replace: failed to parse JSON from {:?}: {}",
                    path.as_ref(),
                    e
                );
                ReplaceConfig::new()
            }
        },
        Err(e) => {
            eprintln!("replace: failed to read {:?}: {}", path.as_ref(), e);
            ReplaceConfig::new()
        }
    }
}

/// 获取全局替换配置（内部使用）
fn get_replace_config() -> ReplaceConfig {
    REPLACE_MAP.read().unwrap().clone()
}

/// 获取替换配置的克隆（用于API返回）
pub fn get_replace_config_clone() -> ReplaceConfig {
    REPLACE_MAP.read().unwrap().clone()
}

/// 获取替换配置的 JSON 字符串
pub fn get_replace_config_json() -> Result<String, String> {
    let config = REPLACE_MAP.read().unwrap();
    serde_json::to_string_pretty(&*config)
        .map_err(|e| format!("Failed to serialize replace config: {}", e))
}

/// 将输入字符串中出现的所有 JSON key 替换为对应的 value
///
/// 替换过程中优先替换较长的 key（避免部分匹配导致意外结果）
pub fn replace(input: &str) -> String {
    let config = get_replace_config();

    // 如果 replaceString 为 false，不进行替换
    if !config.replace_string {
        return input.to_string();
    }

    if config.replace_map.is_empty() {
        return input.to_string();
    }

    // 按 key 长度降序，确保长 key 先被替换
    let mut keys: Vec<&String> = config.replace_map.keys().collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.len()));

    let mut out = input.to_string();
    for k in keys {
        if out.contains(k) {
            if let Some(v) = config.replace_map.get(k) {
                out = out.replace(k, v);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::replace::replace;

    #[test]
    fn test_trad_to_simp_basic() {
        // 构造临时文件，第一行简体，第二行繁体
        // let mut f = NamedTempFile::new().unwrap();
        // writeln!(f, "汉字测试和其它测层蹭插").unwrap(); // 简体行
        // writeln!(f, "漢字測試和其它測層蹭插").unwrap(); // 繁体行
        // f.flush().unwrap();
        // let path = f.path().to_path_buf();

        // 初始化映射
        // init_from_default_file().unwrap();
        // init_from_file(default_translate_file_path()).unwrap();

        let input = "漢字測試和其它測層蹭插[not 24/7]";

        let out = replace(input);
        println!("Output: {}", out);
        // assert_eq!(out, "汉字測試和其它测层蹭插"); // 注意：第二个"測"同字形在简体/繁体中相同，此处只是示例
    }
}

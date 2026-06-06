use crate::r#const::constant::{REPLACE_JSON, REPLACE_TXT_CONTENT};
use crate::utils::file_exists;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::RwLock;

/// Matches empty parentheses or parentheses containing only a single known quality-tag
/// suffix letter (`p`, `k`, `i` and their uppercase equivalents), e.g. `()`, `( )`, `(p)`,
/// `(k)` — residuals left after partial substitution of resolution/quality tags such as
/// `(1080p)` → `(p)` or `(4K)` → `(K)`.
static RE_EMPTY_PARENS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\(\s*[pPkKiI]?\s*\)").unwrap());

/// Matches empty square brackets or brackets containing only whitespace,
/// e.g. `[]`, `[ ]`.
static RE_EMPTY_BRACKETS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[\s*\]").unwrap());

/// Matches two or more consecutive whitespace characters.
static RE_MULTI_SPACE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r" {2,}").unwrap());

/// Replace配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceConfig {
    #[serde(default)]
    pub replace_string: bool,
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
/// 替换过程中优先替换较长的 key（避免部分匹配导致意外结果）。
/// 替换完成后，自动清理残留的空括号、空方括号以及多余空格。
pub fn replace(input: &str) -> String {
    let config = get_replace_config();

    // 如果 replaceString 为 false，不进行替换
    if !config.replace_string {
        return input.to_string();
    }

    replace_with_config(input, &config)
}

/// 纯函数：使用给定的 ReplaceConfig 对 input 执行替换和清理。
/// 便于在不依赖全局状态的情况下进行单元测试。
pub fn replace_with_config(input: &str, config: &ReplaceConfig) -> String {
    if config.replace_map.is_empty() {
        return input.to_string();
    }

    // 按 key 长度降序，确保长 key 先被替换
    let mut keys: Vec<&String> = config.replace_map.keys().collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.len()));

    let mut out = input.to_string();
    for k in keys {
        // 跳过空 key，避免无意义替换
        if k.is_empty() {
            continue;
        }
        if out.contains(k.as_str()) {
            if let Some(v) = config.replace_map.get(k) {
                out = out.replace(k.as_str(), v.as_str());
            }
        }
    }

    // 清理替换后残留的空括号 / 单字符括号，例如 "(p)"、"(k)"、"()"
    out = RE_EMPTY_PARENS.replace_all(&out, "").to_string();
    // 清理替换后残留的空方括号，例如 "[]"、"[ ]"
    out = RE_EMPTY_BRACKETS.replace_all(&out, "").to_string();
    // 将连续多个空格压缩为一个空格，并去除首尾空格
    out = RE_MULTI_SPACE.replace_all(&out, " ").to_string();
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a ReplaceConfig with replace_string=true and the given key→value map.
    fn make_config(map: HashMap<String, String>) -> ReplaceConfig {
        ReplaceConfig {
            replace_string: true,
            replace_map: map,
        }
    }

    #[test]
    fn test_trad_to_simp_basic() {
        let input = "漢字測試和其它測層蹭插[not 24/7]";
        let config = make_config({
            let mut m = HashMap::new();
            m.insert("[not 24/7]".to_string(), "".to_string());
            m
        });
        let out = replace_with_config(input, &config);
        println!("Output: {}", out);
    }

    #[test]
    fn test_replace_leaves_no_empty_parens() {
        // Simulates: user adds rule "1080" -> "" to filter quality tags.
        // "CCTV-1 (1080p)" should become "CCTV-1" after cleanup, not "CCTV-1 (p)".
        let mut map = HashMap::new();
        map.insert("1080".to_string(), "".to_string());
        let result = replace_with_config("CCTV-1 (1080p)", &make_config(map));
        assert_eq!(result, "CCTV-1");
    }

    #[test]
    fn test_replace_removes_empty_brackets() {
        // After removing the content of square brackets, the empty "[]" should be removed too.
        let mut map = HashMap::new();
        map.insert("geo-blocked".to_string(), "".to_string());
        let result = replace_with_config("SomeChannel [geo-blocked]", &make_config(map));
        assert_eq!(result, "SomeChannel");
    }

    #[test]
    fn test_replace_collapses_multiple_spaces() {
        // After removals, consecutive spaces should be collapsed into one.
        let mut map = HashMap::new();
        map.insert(" (1080p)".to_string(), "".to_string());
        let result = replace_with_config("CCTV-1  (1080p)", &make_config(map));
        assert_eq!(result, "CCTV-1");
    }

    #[test]
    fn test_replace_skips_empty_key() {
        // An empty key in the replace map must not cause unexpected behaviour.
        let mut map = HashMap::new();
        map.insert("".to_string(), "REPLACED".to_string());
        map.insert("[HD]".to_string(), "".to_string());
        let result = replace_with_config("CCTV-1 [HD]", &make_config(map));
        assert_eq!(result, "CCTV-1");
    }

    #[test]
    fn test_replace_preserves_valid_parens() {
        // Parentheses containing more than one character (e.g. "(民視)")
        // must NOT be removed.
        let mut map = HashMap::new();
        // Replacement is case-sensitive: key must match the input exactly.
        map.insert("[Not 24/7]".to_string(), "".to_string());
        let result = replace_with_config("FTV (民視) [Not 24/7]", &make_config(map));
        assert_eq!(result, "FTV (民視)");
    }

    #[test]
    fn test_replace_is_case_sensitive() {
        // Replace keys are matched case-sensitively: "[not 24/7]" must not
        // remove "[Not 24/7]" from the input.
        let mut map = HashMap::new();
        map.insert("[not 24/7]".to_string(), "".to_string());
        let result = replace_with_config("Channel [Not 24/7]", &make_config(map));
        // Different casing → no match → input is returned as-is (after cleanup)
        assert_eq!(result, "Channel [Not 24/7]");
    }

    #[test]
    fn test_replace_preserves_other_single_letter_parens() {
        // Only quality-tag suffix letters (p, k, i and their uppercase) are cleaned up.
        // Other single-letter parenthesised content must be preserved.
        let mut map = HashMap::new();
        map.insert("720".to_string(), "".to_string());
        // "(A)" is not a quality-tag residual and must survive.
        let result = replace_with_config("Channel (A) (720p)", &make_config(map));
        assert_eq!(result, "Channel (A)");
    }
}

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use crate::r#const::constant::REPLACE_JSON;

/// 全局只读替换表，首次调用时从 "replace.json" 读取并解析为 HashMap<String, String>
static REPLACE_MAP: OnceLock<HashMap<String, String>> = OnceLock::new();

fn default_replace_file_path() -> PathBuf {
    // 使用编译时的工作目录（crate 根目录）
    Path::new(env!("CARGO_MANIFEST_DIR")).join(REPLACE_JSON)
}

/// 尝试从指定路径读取 JSON 并解析为 HashMap<String, String>，若失败返回空映射
fn read_replace_json<P: AsRef<Path>>(path: P) -> HashMap<String, String> {
    match fs::read_to_string(&path) {
        Ok(s) => match serde_json::from_str::<HashMap<String, String>>(&s) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("replace: failed to parse JSON from {:?}: {}", path.as_ref(), e);
                HashMap::new()
            }
        },
        Err(e) => {
            eprintln!("replace: failed to read {:?}: {}", path.as_ref(), e);
            HashMap::new()
        }
    }
}

/// 获取全局替换表，默认为当前工作目录下的 "replace.json"
fn get_replace_map() -> &'static HashMap<String, String> {
    let p = default_replace_file_path();
    REPLACE_MAP.get_or_init(|| read_replace_json(p))
}

/// 将输入字符串中出现的所有 JSON key 替换为对应的 value
///
/// 替换过程中优先替换较长的 key（避免部分匹配导致意外结果）
pub fn replace(input: &str) -> String {
    let map = get_replace_map();

    if map.is_empty() {
        return input.to_string();
    }

    // 按 key 长度降序，确保长 key 先被替换
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.len()));

    let mut out = input.to_string();
    for k in keys {
        if out.contains(k) {
            if let Some(v) = map.get(k) {
                out = out.replace(k, v);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::common::replace::{replace};
    use super::*;

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
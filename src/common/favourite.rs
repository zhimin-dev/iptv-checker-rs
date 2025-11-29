use std::{fs, io};
use crate::r#const::constant::{FAVOURITE_CONFIG_JSON_CONTENT, FAVOURITE_FILE_NAME};
use crate::utils::file_exists;
use std::io::Write;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
/// 获取某个分组（如 "频道收藏夹"）下的所有收藏频道列表
pub fn get_favourite_list(group: &str) -> Vec<String> {
    let map = get_favourite_map();
    map.get(group).cloned().unwrap_or_default()
}

/// 判断某个频道名是否在任意收藏夹或指定分组收藏中
pub fn is_in_favourite(channel_name: &str, group: Option<&str>) -> bool {
    let map = get_favourite_map();
    if let Some(g) = group {
        if let Some(arr) = map.get(g) {
            arr.iter().any(|x| x == channel_name)
        } else {
            false
        }
    } else {
        map.values().any(|v| v.iter().any(|x| x == channel_name))
    }
}

/// 返回全部分组名（收藏夹名）
/// 例如 ["频道收藏夹", "自定义"]
pub fn get_all_favourite_groups() -> Vec<String> {
    get_favourite_map().keys().cloned().collect()
}


static FAVOURITE_MAP: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();

pub fn get_favourite_map() -> &'static HashMap<String, Vec<String>> {
    let p = Path::new(format!("{}", FAVOURITE_FILE_NAME).as_mut_str()).to_owned();
    FAVOURITE_MAP.get_or_init(|| read_favourite_json(p))
}

fn read_favourite_json<P: AsRef<Path>>(path: P) -> HashMap<String, Vec<String>> {
    match fs::read_to_string(&path) {
        Ok(s) => match serde_json::from_str::<HashMap<String, Vec<String>>>(&s) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("favourite: failed to parse JSON from {:?}: {}", path.as_ref(), e);
                HashMap::new()
            }
        },
        Err(e) => {
            eprintln!("favourite: failed to read {:?}: {}", path.as_ref(), e);
            HashMap::new()
        }
    }
}

pub fn create_favourite_file() {
    if !file_exists(&FAVOURITE_FILE_NAME.to_string()) {
        let mut fd = fs::File::create(FAVOURITE_FILE_NAME.to_string())
            .expect(&format!("Failed to create file: {}", FAVOURITE_FILE_NAME.to_string()));
        fd.write_all(FAVOURITE_CONFIG_JSON_CONTENT.to_string().as_bytes())
            .expect(&format!("Failed to write file: {}", FAVOURITE_FILE_NAME.to_string()));
        fd.flush()
            .expect(&format!("Failed to flush file: {}", FAVOURITE_FILE_NAME.to_string()));
    }
}

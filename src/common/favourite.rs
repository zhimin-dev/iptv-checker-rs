use crate::r#const::constant::{FAVOURITE_CONFIG_JSON_CONTENT, FAVOURITE_FILE_NAME};
use crate::utils::file_exists;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::RwLock;
/// 获取某个分组（如 "频道收藏夹"）下的所有收藏频道列表
pub fn get_favourite_list(group: &str) -> Vec<String> {
    let map = get_favourite_map();
    map.get(group).cloned().unwrap_or_default()
}

/// 判断某个频道名是否在任意收藏夹或指定分组收藏中
// pub fn is_in_favourite(channel_name: &str, group: Option<&str>) -> bool {
//     let map = get_favourite_map();
//     if let Some(g) = group {
//         if let Some(arr) = map.get(g) {
//             arr.iter().any(|x| x == channel_name)
//         } else {
//             false
//         }
//     } else {
//         map.values().any(|v| v.iter().any(|x| x == channel_name))
//     }
// }

/// 返回全部分组名（收藏夹名）
/// 例如 ["频道收藏夹", "自定义"]
// pub fn get_all_favourite_groups() -> Vec<String> {
//     get_favourite_map().keys().cloned().collect()
// }

static FAVOURITE_MAP: Lazy<RwLock<HashMap<String, Vec<String>>>> = Lazy::new(|| {
    let p = Path::new(format!("{}", FAVOURITE_FILE_NAME).as_mut_str()).to_owned();
    RwLock::new(read_favourite_json(p))
});

pub fn get_favourite_map() -> HashMap<String, Vec<String>> {
    FAVOURITE_MAP.read().unwrap().clone()
}

/// 重新加载 favourite.json 文件
pub fn reload_favourite_map() -> Result<(), String> {
    let p = Path::new(format!("{}", FAVOURITE_FILE_NAME).as_mut_str()).to_owned();
    let new_map = read_favourite_json(&p);
    let mut map = FAVOURITE_MAP.write().unwrap();
    *map = new_map;
    Ok(())
}

fn read_favourite_json<P: AsRef<Path>>(path: P) -> HashMap<String, Vec<String>> {
    match fs::read_to_string(&path) {
        Ok(s) => {
            if s.trim().is_empty() {
                eprintln!("favourite: file {:?} is empty", path.as_ref());
                return HashMap::new();
            }
            match serde_json::from_str::<HashMap<String, Vec<String>>>(&s) {
                Ok(m) => {
                    eprintln!(
                        "favourite: successfully loaded {} entries from {:?}",
                        m.len(),
                        path.as_ref()
                    );
                    m
                }
                Err(e) => {
                    eprintln!(
                        "favourite: failed to parse JSON from {:?}: {}",
                        path.as_ref(),
                        e
                    );
                    eprintln!("favourite: file content: {}", s);
                    HashMap::new()
                }
            }
        }
        Err(e) => {
            eprintln!("favourite: failed to read {:?}: {}", path.as_ref(), e);
            HashMap::new()
        }
    }
}

pub fn create_favourite_file() {
    if !file_exists(&FAVOURITE_FILE_NAME.to_string()) {
        // 确保 core 目录存在
        if let Some(parent) = std::path::Path::new(FAVOURITE_FILE_NAME).parent() {
            fs::create_dir_all(parent).expect(&format!("Failed to create directory: {:?}", parent));
        }
        let mut fd = fs::File::create(FAVOURITE_FILE_NAME.to_string()).expect(&format!(
            "Failed to create file: {}",
            FAVOURITE_FILE_NAME.to_string()
        ));
        fd.write_all(FAVOURITE_CONFIG_JSON_CONTENT.to_string().as_bytes())
            .expect(&format!(
                "Failed to write file: {}",
                FAVOURITE_FILE_NAME.to_string()
            ));
        fd.flush().expect(&format!(
            "Failed to flush file: {}",
            FAVOURITE_FILE_NAME.to_string()
        ));
    }
}

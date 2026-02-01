use crate::r#const::constant::TRANSLATE_FILE;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::sync::OnceLock;

/// 编译时嵌入的翻译文件内容
const EMBEDDED_TRANSLATE_CONTENT: &str = include_str!("../assets/translate.txt");

/// 全局懒加载映射表（key: 繁体字符, value: 简体字符）
static TRANSLATE_MAP: OnceLock<HashMap<char, char>> = OnceLock::new();

/// 从字符串内容加载映射表：第一行为简体，第二行为繁体，按字符位置一一映射
fn load_map_from_content(content: &str) -> HashMap<char, char> {
    let mut lines = content.lines();
    let simp_line = lines.next().unwrap_or("").trim_end_matches('\r');
    let trad_line = lines.next().unwrap_or("").trim_end_matches('\r');

    let simp_chars: Vec<char> = simp_line.chars().collect();
    let trad_chars: Vec<char> = trad_line.chars().collect();

    let mut m = HashMap::new();
    for i in 0..std::cmp::min(simp_chars.len(), trad_chars.len()) {
        m.insert(trad_chars[i], simp_chars[i]);
    }
    println!("load_map_from_content --- m: {:?}", m.len());
    m
}

/// 从指定文件加载映射表：优先使用文件系统中的文件，如果不存在则使用嵌入的内容
fn load_map_from_path() -> io::Result<HashMap<char, char>> {
    // 优先尝试从文件系统读取
    match fs::read_to_string(TRANSLATE_FILE) {
        Ok(content) => {
            println!("Using translate file from filesystem: {}", TRANSLATE_FILE);
            Ok(load_map_from_content(&content))
        }
        Err(_) => {
            // 如果文件不存在，使用嵌入的内容
            println!("Translate file not found, using embedded content");
            Ok(load_map_from_content(EMBEDDED_TRANSLATE_CONTENT))
        }
    }
}

/// 初始化全局映射（首次调用自动使用项目根目录下的 translate.txt）
/// 如果加载失败，返回 io::Error；随后可继续使用 `trad_to_simp`（失败时会降级为不做替换）
pub fn init_from_default_file() -> io::Result<()> {
    init_from_file()
}

/// 使用指定文件初始化全局映射
pub fn init_from_file() -> io::Result<()> {
    let map = load_map_from_path()?;
    println!("init_from_file --- map: {:?}", map.len());
    // OnceLock::set 返回 Err(map) 如果已经设置过，这里忽略已设置的情况
    let _ = TRANSLATE_MAP.set(map);
    Ok(())
}

/// 将繁体字符串转换为简体字符串
/// - 会尝试使用已初始化的全局映射；如果未初始化，会尝试从文件系统加载，失败则使用嵌入的内容
/// - 未命中的字符原样保留
pub fn trad_to_simp(input: &str) -> String {
    let map = TRANSLATE_MAP.get_or_init(|| {
        // 尝试从文件系统加载，失败则使用嵌入的内容
        load_map_from_path().unwrap_or_else(|_| {
            println!("Failed to load translate file, using embedded content");
            load_map_from_content(EMBEDDED_TRANSLATE_CONTENT)
        })
    });

    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if let Some(&s) = map.get(&ch) {
            out.push(s);
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::utils::{get_host_ip_address, get_url_host_and_port, is_valid_ip};

    #[test]
    fn address() {
        let url = "http://drive.mxmy.net:8888/udp/239.3.1.188:8001";
        let (host_str, port) = get_url_host_and_port(&url);
        if is_valid_ip(&host_str) {
            println!("Valid ip: {}", host_str);
        } else {
            let list = get_host_ip_address(&host_str, port);
            println!("list: {:?}", list);
        }
    }

    #[test]
    fn test_trad_to_simp_basic() {
        // 构造临时文件，第一行简体，第二行繁体
        // let mut f = NamedTempFile::new().unwrap();
        // writeln!(f, "汉字测试和其它测层蹭插").unwrap(); // 简体行
        // writeln!(f, "漢字測試和其它測層蹭插").unwrap(); // 繁体行
        // f.flush().unwrap();
        // let path = f.path().to_path_buf();

        // 初始化映射
        init_from_default_file().unwrap();
        // init_from_file(default_translate_file_path()).unwrap();

        let input = "漢字測試和其它測層蹭插";

        let out = trad_to_simp(input);
        println!("Output: {}", out);
        // assert_eq!(out, "汉字測試和其它测层蹭插"); // 注意：第二个"測"同字形在简体/繁体中相同，此处只是示例
    }
}

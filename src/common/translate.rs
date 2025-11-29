use std::collections::HashMap;
use std::io;
use std::sync::OnceLock;
use std::fs;
use crate::r#const::constant::TRANSLATE_FILE;

/// 全局懒加载映射表（key: 繁体字符, value: 简体字符）
static TRANSLATE_MAP: OnceLock<HashMap<char, char>> = OnceLock::new();

/// 从指定文件加载映射表：第一行为简体，第二行为繁体，按字符位置一一映射
fn load_map_from_path() -> io::Result<HashMap<char, char>> {
    let content = fs::read_to_string(TRANSLATE_FILE)?;
    let mut lines = content.lines();
    let simp_line = lines.next().unwrap_or("").trim_end_matches('\r');
    let trad_line = lines.next().unwrap_or("").trim_end_matches('\r');

    let simp_chars: Vec<char> = simp_line.chars().collect();
    let trad_chars: Vec<char> = trad_line.chars().collect();

    let mut m = HashMap::new();
    for i in 0..std::cmp::min(simp_chars.len(), trad_chars.len()) {
        m.insert(trad_chars[i], simp_chars[i]);
    }
    println!("load_map_from_path --- m: {:?}", m.len());
    Ok(m)
}

/// 初始化全局映射（首次调用自动使用项目根目录下的 translate.txt）
/// 如果加载失败，返回 io::Error；随后可继续使用 `trad_to_simp`（失败时会降级为不做替换）
pub fn init_from_default_file() -> io::Result<()> {
    init_from_file()
}

/// 使用指定文件初始化全局映射
pub fn init_from_file() -> io::Result<()> {
    let map = load_map_from_path()?;
    // OnceLock::set 返回 Err(map) 如果已经设置过，这里忽略已设置的情况
    let _ = TRANSLATE_MAP.set(map);
    Ok(())
}

/// 将繁体字符串转换为简体字符串
/// - 会尝试使用已初始化的全局映射；如果未初始化，会尝试从项目根目录的 translate.txt 加载一次（失败则视为空映射）
/// - 未命中的字符原样保留
pub fn trad_to_simp(input: &str) -> String {
    let map = TRANSLATE_MAP.get_or_init(|| {
        // 尝试自动加载，加载失败则返回空映射
        load_map_from_path().unwrap_or_default()
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
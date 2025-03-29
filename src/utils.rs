use rand::distr::Alphanumeric;
use rand::Rng;
use regex::Regex;
use std::fs;
use std::fs::File;
use std::io::{Error, ErrorKind, Read};
use std::process::Command;
// use opencc_rust::*;

pub fn get_out_put_filename(output_file: String) -> String {
    let mut filename = output_file.clone();
    if output_file.is_empty() {
        filename = format!("static/output/{}", get_random_output_filename());
    }
    filename
}

// pub fn check_ip_address(ip: &str) -> Result<&'static str, &'static str> {
//     match ip.parse::<IpAddr>() {
//         Ok(IpAddr::V4(_)) => Ok("IPv4"),
//         Ok(IpAddr::V6(_)) => Ok("IPv6"),
//         Err(_) => Err("Invalid IP address format"),
//     }
// }

fn get_random_output_filename() -> String {
    let rng = rand::thread_rng();

    let random_string: String = rng
        .sample_iter(Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();
    format!("{}.m3u", random_string)
}

fn read_pid_contents(pid_file: String) -> Result<String, Error> {
    let mut f = File::open(pid_file)?;
    let mut contents = String::default();
    f.read_to_string(&mut contents)?;
    Ok(contents)
}

pub fn check_process(pid: u32) -> Result<bool, Error> {
    let status = Command::new("ps").arg("-p").arg(pid.to_string()).output();
    Ok(status.unwrap().status.success())
}

pub fn file_exists(file_path: &String) -> bool {
    if let Ok(metadata) = fs::metadata(file_path) {
        metadata.is_file()
    } else {
        false
    }
}

use lazy_static::lazy_static;

lazy_static! {
    static ref RE: Regex = Regex::new(r"(?m)(\d+\s)?\[\w+\]").unwrap(); // 仅编译一次
    static ref RegexPrefixNum:  Regex = Regex::new(r"^\d+\s*").unwrap();
    // static ref Translator:OpenCC = OpenCC::new(DefaultConfig::T2S).unwrap();
}

pub fn remove_other_char(str: String) -> String {
    let mut res_str = str.to_string();
    // 去掉前面的无用字符
    let result = RE.captures_iter(&str);
    for mat in result {
        if mat.len() >= 1 {
            res_str = res_str.replace(mat.get(0).unwrap().as_str(), "");
        }
    }
    let rename_channel_list: Vec<&str> = vec!["[geo-blocked]", "[ipv6]", "hevc", "50 fps", "[not 24/7]"];
    // 去掉后面特殊字符
    for change in rename_channel_list {
        res_str = res_str.replace(change, "")
    }
    let binding = res_str.to_string();
    // 去掉前面的无用数组
    let pre_num_result = RegexPrefixNum.captures_iter(&binding);
    for mat in pre_num_result {
        if mat.len() >= 1 {
            res_str = res_str.replace(mat.get(0).unwrap().as_str(), "");
        }
    }
    res_str
}
pub fn translator_t2s(str: &str) -> String {
    // Translator.convert(str)
    str.to_string()
}

#[cfg(test)]
mod tests {
    use crate::utils::remove_other_char;

    #[tokio::test]
    async fn test_str() {
        println!("{}", remove_other_char("213123 [HD]这是1".to_string()));
        println!("{}", remove_other_char("[HD]这是2".to_string()));
        println!("{}", remove_other_char("[HD]cctv3".to_string()));
        println!("{}", remove_other_char("[bd]cctv4".to_string()));
        println!("{}", remove_other_char("2323 cctv5".to_string()));
        println!("{}", remove_other_char("2323 cctv6[geo-blocked]".to_string()));

        // println!("{}", translator_t2s("FTV (民視) (720p) [Not 24/7]"));
    }
}

pub fn folder_exists(file_path: &String) -> bool {
    if let Ok(metadata) = fs::metadata(file_path) {
        metadata.is_dir()
    } else {
        false
    }
}

// 如果pid文件存在，需要将之前的pid删除，然后才能启动新的pid
pub fn check_pid_exits(pid_name: &String) {
    if file_exists(pid_name) {
        let num = read_pid_num(pid_name).expect("获取pid失败");
        let has_process = check_process(num).expect("检查pid失败");
        if has_process {
            kill_process(num);
        }
    }
}

fn kill_process(pid: u32) {
    let _output = Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .output()
        .expect("Failed to execute command");
}

pub fn read_pid_num(pid_name: &String) -> Result<u32, Error> {
    match read_pid_contents(pid_name.clone()) {
        Ok(contents) => {
            let mut n_contents = contents;
            n_contents = n_contents.replace('\n', "");
            match n_contents.parse::<u32>() {
                Ok(num) => Ok(num),
                Err(e) => Err(Error::new(ErrorKind::InvalidData, e)),
            }
        }
        Err(e) => Err(e),
    }
}

pub fn create_folder(folder_name: &String) -> Result<(), Error> {
    if !folder_exists(folder_name) {
        fs::create_dir(folder_name)
    } else {
        Ok(())
    }
}

mod common;
mod utils;
mod web;

use clap::{arg, Args as clapArgs, Parser, Subcommand};
use std::{env, thread};
use std::time::Duration;
use tempfile::tempdir;
use crate::common::do_check;

#[derive(Subcommand)]
enum Commands {
    /// web相关命令
    Web(WebArgs),
    /// 检查相关命令
    Check(CheckArgs),
}

#[derive(clapArgs)]
pub struct WebArgs {
    /// 启动一个web服务
    #[arg(long = "start", default_value_t = false)]
    start: bool,

    /// 指定这个web服务的端口号，默认8089
    #[arg(long = "port", default_value_t = 8089)]
    port: u16,

    /// 关闭这个web服务
    #[arg(long = "stop", default_value_t = false)]
    stop: bool,

    /// 输出当前web服务的状态，比如pid信息
    #[arg(long = "status", default_value_t = false)]
    status: bool,
}

#[derive(clapArgs)]
pub struct CheckArgs {
    /// 输入文件，可以是本地文件或者是网络文件，支持标准m3u格式以及非标准的格式：CCTV,https://xxxx.com/xxx.m3u8格式
    #[arg(short = 'i', long = "input-file")]
    input_file: Vec<String>,

    // /// [待实现]支持sdr、hd、fhd、uhd、fuhd搜索
    // #[arg(short = 's', long = "search_clarity", default_value_t = String::from(""))]
    // search_clarity: String,

    /// 输出文件，如果不指定，则默认生成一个随机文件名
    #[arg(short = 'o', long = "output-file", default_value_t = String::from(""))]
    output_file: String,

    /// 超时时间，默认超时时间为28秒
    #[arg(short = 't', long = "timeout", default_value_t = 28000)]
    timeout: u16,

    /// debug使用，可以看到相关的中间日志
    #[arg(long = "debug", default_value_t = false)]
    debug: bool,

    /// 并发数
    #[arg(short = 'c', long = "concurrency", default_value_t = 1)]
    concurrency: i32,
}

#[derive(Parser)]
#[command(name = "iptv-checker")]
#[command(author = "zmisgod", version = env ! ("CARGO_PKG_VERSION"), about = "a iptv-checker cmd, source code 👉 https://github.com/zhimin-dev/iptv-checker", long_about = None,)]
pub struct Args {
    #[command(subcommand)]
    command: Commands,
}

fn get_pid_file() -> String {
    if let Ok(dir) = tempdir() {
        if let Some(a) = dir.path().join("iptv_checker_web_server.pid").to_str() {
            return a.to_owned();
        }
    }
    return String::default();
}

async fn start_daemonize_web(pid_name: &String, port: u16) {
    utils::check_pid_exits(pid_name);
    println!("start web server, port:{}", port);
    // 启动 web 服务
    web::start_web(port).await;
}

pub fn show_status() {
    let pid_name = get_pid_file();
    if utils::file_exists(&pid_name) {
        match utils::read_pid_num(&pid_name) {
            Ok(num) => {
                let has_process = utils::check_process(num).unwrap();
                if has_process {
                    println!("web server running at pid = {}", num)
                }
            }
            Err(e) => {
                println!("{}", e)
            }
        }
    }
}

#[actix_web::main]
pub async fn main() {
    let pid_name = get_pid_file();
    let args = Args::parse();
    match args.command {
        Commands::Web(args) => {
            if args.status {
                show_status();
            } else if args.start {
                let mut port = args.port;
                if port == 0 {
                    port = 8080
                }
                start_daemonize_web(&pid_name, port).await;
            } else if args.stop {
                utils::check_pid_exits(&pid_name);
            }
        }
        Commands::Check(args) => {
            if args.input_file.len() > 0 {
                println!("您输入的文件地址是: {}", args.input_file.join(","));
                do_check(args.input_file.to_owned(), args.output_file.clone(), args.timeout as u64, true, args.timeout as i32, args.concurrency).await.unwrap();
            }
        }
    }
}

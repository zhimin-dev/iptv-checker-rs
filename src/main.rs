mod common;
mod live;
mod search;
mod utils;
mod web;
mod middleware;
mod config;

use crate::common::do_check;
use crate::live::do_ob;
use crate::search::{clear_search_folder, do_search};
use crate::utils::{create_folder, file_exists};
use crate::config::config::{init_config, Core};
use crate::config::*;
use chrono::Local;
use clap::{arg, Args as clapArgs, Parser, Subcommand};
use log::{error, info, LevelFilter};
use simplelog::{CombinedLogger, Config, WriteLogger};
use std::env;
use std::fs::File;
use tempfile::tempdir;

const DEFAULT_HTTP_PORT: u16 = 8089;

#[derive(Subcommand)]
enum Commands {
    /// web相关命令
    Web(WebArgs),
    /// 检查相关命令
    Check(CheckArgs),
    /// 搜索相关命令
    Search(SearchArgs),
    /// 转播相关命令
    Ob(ObArgs),
}

#[derive(clapArgs)]
pub struct SearchArgs {
    /// 搜索频道名称,如果有别名，用英文逗号分隔
    #[arg(long = "search", default_value_t = String::from(""))]
    search: String,

    /// 是否需要生成缩略图
    #[arg(long = "thumbnail", default_value_t = false)]
    thumbnail: bool,

    /// 清理资源池
    #[arg(long = "clear", default_value_t = false)]
    clear: bool,
}

#[derive(clapArgs)]
pub struct ObArgs {
    /// 需要转播的链接
    #[arg(short = 'i', long = "input-url")]
    input_url: String,
}

#[derive(clapArgs)]
pub struct WebArgs {
    /// 启动一个web服务
    #[arg(long = "start", default_value_t = false)]
    start: bool,

    /// 指定这个web服务的端口号
    #[arg(long = "port", default_value_t = DEFAULT_HTTP_PORT)]
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
    /// 输入文件，可以是本地文件或者是网络文件，支持标准m3u格式以及非标准的格式：
    /// CCTV,https://xxxx.com/xxx.m3u8格式
    #[arg(short = 'i', long = "input-file")]
    input_file: Vec<String>,

    // /// [待实现]支持sdr、hd、fhd、uhd、fuhd搜索
    // #[arg(short = 's', long = "search_clarity", default_value_t = String::from(""))]
    // search_clarity: String,
    /// 输出文件，如果不指定，则默认生成一个随机文件名
    #[arg(short = 'o', long = "output-file", default_value_t = String::from(""))]
    output_file: String,

    /// 超时时间，默认超时时间为10秒
    #[arg(short = 't', long = "timeout", default_value_t = 10000)]
    timeout: u16,

    /// debug使用，可以看到相关的中间日志
    #[arg(long = "debug", default_value_t = false)]
    debug: bool,

    /// 并发数
    #[arg(short = 'c', long = "concurrency", default_value_t = 1)]
    concurrency: i32,

    /// 想看关键词
    #[arg(long = "like")]
    keyword_like: Vec<String>,

    /// 不想看关键词
    #[arg(long = "dislike")]
    keyword_dislike: Vec<String>,

    /// 频道排序
    #[arg(long = "sort", default_value_t = false)]
    sort: bool,

    /// 是否不需要检查
    #[arg(long = "no-check", default_value_t = false)]
    no_check: bool,

    /// 去掉无用的字段
    #[arg(long = "rename", default_value_t = false)]
    rename: bool,

    /// 使用ffmpeg检查
    #[arg(long = "ffmpeg-check", default_value_t = false)]
    ffmpeg_check: bool,

    /// 如果名称相同，保存几个源，默认全部保存
    #[arg(long = "same-save-num", default_value_t = 0)]
    same_save_num: i32,

    /// 如果非http，就跳过
    #[arg(long = "not-http-skip", default_value_t = false)]
    not_http_skip: bool,
}

#[derive(Parser)]
#[command(
    name = "iptv-checker", author = "zmisgod", version = env ! ("CARGO_PKG_VERSION"),
    about = "a iptv-checker cmd, source code 👉 https://github.com/zhimin-dev/iptv-checker",
    long_about = None,
)]
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
    String::default()
}

async fn start_daemonize_web(pid_name: &String, port: u16) {
    utils::check_pid_exits(pid_name);
    info!("start web server, port:{}", port);
    // 启动 web 服务
    web::start_web(port).await;
}

fn init_folder() {
    let folder = vec!["./static",
                      "./static/input", "./static/input/live", "./static/input/search",
                      "./static/output", "./static/output/thumbnail",
                      "./static/logs"
    ];
    for f in folder {
        create_folder(&f.to_string()).unwrap()
    }
}

pub fn show_status() {
    let pid_name = get_pid_file();
    if utils::file_exists(&pid_name) {
        match utils::read_pid_num(&pid_name) {
            Ok(num) => {
                let has_process = utils::check_process(num).unwrap();
                if has_process {
                    info!("web server running at pid = {}", num)
                }
            }
            Err(e) => {
                error!("server start failed: {}", e)
            }
        }
    }
}

#[actix_web::main]
pub async fn main() {
    // Initialize logger at the start
    CombinedLogger::init(
        vec![
            WriteLogger::new(
                LevelFilter::Info,
                Config::default(),
                std::io::stdout(),
            ),
        ]
    ).unwrap();

    init_config();

    init_folder();
    let pid_name = get_pid_file();
    let args = Args::parse();
    match args.command {
        Commands::Web(args) => {
            if args.status {
                show_status();
            } else if args.start {
                let mut port = args.port;
                if port == 0 {
                    port = DEFAULT_HTTP_PORT
                }
                start_daemonize_web(&pid_name, port).await;
            } else if args.stop {
                utils::check_pid_exits(&pid_name);
            }
        }
        Commands::Check(args) => {
            if args.input_file.len() > 0 {
                info!("您输入的文件地址是: {}", args.input_file.join(","));
                do_check(
                    args.input_file.to_owned(),
                    args.output_file.clone(),
                    args.timeout as i32,
                    true,
                    args.timeout as i32,
                    args.concurrency,
                    args.keyword_like.to_owned(),
                    args.keyword_dislike.to_owned(),
                    args.sort,
                    args.no_check,
                    args.rename,
                    args.ffmpeg_check,
                    args.same_save_num,
                    args.not_http_skip,
                )
                    .await
                    .unwrap();
            }
        }
        Commands::Search(args) => {
            if args.clear {
                if let Ok(_) = clear_search_folder() {
                    info!("clear success 😄")
                } else {
                    error!("clear failed 😞")
                }
            } else {
                if args.search.len() > 0 {
                    let data = do_search(args.search.clone(), args.thumbnail).await;
                    match data {
                        Ok(data) => {
                            info!("{:?}", data)
                        }
                        Err(e) => {
                            error!("获取失败---{}", e)
                        }
                    }
                }
            }
        }
        Commands::Ob(args) => {
            let data = do_ob(args.input_url.clone());
            match data {
                Ok(_url) => {
                    info!("url - {}", _url.clone())
                }
                Err(e) => {
                    error!("ob error - {}", e);
                }
            }
        }
    }
}

mod common;
mod config;
mod r#const;
mod live;
mod search;
mod utils;
mod web;

use crate::common::{do_check, SearchOptions, SearchParams};
use crate::config::config::init_config;
use crate::live::do_ob;
use crate::r#const::constant::{
    INPUT_FOLDER, INPUT_LIVE_FOLDER, INPUT_SEARCH_FOLDER, LOGS_FOLDER, OUTPUT_FOLDER,
    OUTPUT_THUMBNAIL_FOLDER, STATIC_FOLDER,
};
use crate::search::{clear_search_folder, do_search};
use crate::utils::{create_folder, get_out_put_filename};
use clap::{arg, Args as clapArgs, Parser, Subcommand};
use log::{error, info, LevelFilter};
use simplelog::{CombinedLogger, Config, WriteLogger};
use std::env;
use tempfile::tempdir;

const DEFAULT_HTTP_PORT: u16 = 8089;

#[derive(Subcommand)]
enum Commands {
    /// Web服务相关命令
    Web(WebArgs),
    /// IPTV检查相关命令
    Check(CheckArgs),
    /// 频道搜索相关命令
    Search(SearchArgs),
    /// 转播相关命令
    Ob(ObArgs),
}

#[derive(clapArgs)]
pub struct SearchArgs {
    /// 频道名称包含的关键词
    #[arg(long = "like")]
    keyword_like: Vec<String>,

    /// 频道名称不包含的关键词
    #[arg(long = "dislike")]
    keyword_dislike: Vec<String>,

    /// 频道名称不包含的关键词
    #[arg(long = "fmword")]
    keyword_full: Vec<String>,

    /// 是否生成频道缩略图
    #[arg(long = "thumbnail", default_value_t = false)]
    thumbnail: bool,

    /// 是否清理搜索资源池
    #[arg(long = "clear", default_value_t = false)]
    clear: bool,

    /// 并发搜索数量
    #[arg(short = 'c', long = "concurrency", default_value_t = 1)]
    concurrency: i32,

    /// 检查超时时间（毫秒）
    #[arg(short = 't', long = "timeout", default_value_t = 10000)]
    timeout: u16,

    /// 输出文件路径，不指定则生成随机文件名
    #[arg(short = 'o', long = "output-file", default_value_t = String::from(""))]
    output_file: String,
}

#[derive(clapArgs)]
pub struct ObArgs {
    /// 需要转播的源链接
    #[arg(short = 'i', long = "input-url")]
    input_url: String,
}

#[derive(clapArgs)]
pub struct WebArgs {
    /// 启动Web服务
    #[arg(long = "start", default_value_t = false)]
    start: bool,

    /// 指定Web服务端口号
    #[arg(long = "port", default_value_t = DEFAULT_HTTP_PORT)]
    port: u16,

    /// 停止Web服务
    #[arg(long = "stop", default_value_t = false)]
    stop: bool,

    /// 查看Web服务状态
    #[arg(long = "status", default_value_t = false)]
    status: bool,
}

#[derive(clapArgs)]
pub struct CheckArgs {
    /// 输入文件路径，支持本地文件或网络文件，支持标准m3u格式和非标准格式
    #[arg(short = 'i', long = "input-file")]
    input_file: Vec<String>,

    /// 输出文件路径，不指定则生成随机文件名
    #[arg(short = 'o', long = "output-file", default_value_t = String::from(""))]
    output_file: String,

    /// 检查超时时间（毫秒）
    #[arg(short = 't', long = "timeout", default_value_t = 10000)]
    timeout: u16,

    /// 是否启用调试模式
    #[arg(long = "debug", default_value_t = false)]
    debug: bool,

    /// 并发检查数量
    #[arg(short = 'c', long = "concurrency", default_value_t = 1)]
    concurrency: i32,

    /// 频道名称包含的关键词
    #[arg(long = "like")]
    keyword_like: Vec<String>,

    /// 频道名称不包含的关键词
    #[arg(long = "dislike")]
    keyword_dislike: Vec<String>,

    /// 频道名称不包含的关键词
    #[arg(long = "fmword")]
    keyword_full: Vec<String>,

    /// 是否对频道进行排序
    #[arg(long = "sort", default_value_t = false)]
    sort: bool,

    /// 是否跳过检查步骤
    #[arg(long = "no-check", default_value_t = false)]
    no_check: bool,

    /// 是否重命名无用字段
    #[arg(long = "rename", default_value_t = false)]
    rename: bool,

    /// 是否使用ffmpeg进行检查
    #[arg(long = "ffmpeg-check", default_value_t = false)]
    ffmpeg_check: bool,

    /// 相同名称频道保存的最大数量
    #[arg(long = "same-save-num", default_value_t = 0)]
    same_save_num: i32,

    /// 是否跳过非HTTP协议的源
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
    web::start_web(port).await;
}

fn init_folder() {
    let folder = vec![
        STATIC_FOLDER,
        INPUT_FOLDER,
        INPUT_LIVE_FOLDER,
        INPUT_SEARCH_FOLDER,
        OUTPUT_FOLDER,
        OUTPUT_THUMBNAIL_FOLDER,
        LOGS_FOLDER,
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
    CombinedLogger::init(vec![WriteLogger::new(
        LevelFilter::Debug,
        Config::default(),
        std::io::stdout(),
    )])
    .unwrap();

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
                let output_file = get_out_put_filename(OUTPUT_FOLDER, args.output_file.to_owned());
                
                println!("output file: {}", output_file.clone());
                let data = do_search(SearchParams {
                    thumbnail: args.thumbnail,
                    concurrent: args.concurrency,
                    timeout: args.timeout,
                    output_file,
                    search_options: SearchOptions {
                        keyword_full_match: args.keyword_full,
                        keyword_like: args.keyword_like,
                        keyword_dislike: args.keyword_dislike,
                        ipv4: false,
                        ipv6: false,
                        exclude_url: vec![],
                        exclude_host: vec![],
                        quality: vec![],
                    },
                })
                .await;
                match data {
                    Ok(()) => {
                        info!("成功 ---")
                    }
                    Err(e) => {
                        error!("获取失败---{}", e)
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

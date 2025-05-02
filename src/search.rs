use crate::common::m3u::m3u::from_body_arr;
use crate::common::{M3uObject, M3uObjectList, SearchOptions};
use crate::config;
use crate::r#const::constant::{INPUT_SEARCH_FOLDER, OUTPUT_THUMBNAIL_FOLDER};
use crate::utils::{create_folder, folder_exists};
use chrono::{Datelike, Local};
use log::{debug, error, info};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Error;
use std::io::Write;
use std::{fs, vec};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GithubPageProps {
    pub props: GithubPagePropInitialPayload,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GithubSubPageProps {
    pub payload: GithubPagePropTree,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GithubPagePropInitialPayload {
    #[serde(rename = "initialPayload")]
    pub initial_payload: GithubPagePropTree,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GithubPagePropTree {
    pub tree: GithubPagePropItems,
    pub repo: GithubPagePropRepo,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GithubPagePropRepo {
    #[serde(rename = "ownerLogin")]
    pub owner_login: String,
    pub name: String,
    #[serde(rename = "defaultBranch")]
    pub default_branch: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GithubPagePropItems {
    pub items: Vec<GithubPagePropTreeItem>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GithubPagePropTreeItem {
    #[serde(rename = "contentType")]
    pub content_type: String,

    pub path: String,
    pub name: String,
}

#[derive(Debug)]
pub struct GithubInfo {
    pub content_type: String,
    pub path: String,
    pub name: String,
    pub download_url: String,
    pub extension: String, //.txt, .m3u
}

impl GithubInfo {
    pub fn new(
        content_type: String,
        path: String,
        name: String,
        download_url: String,
        extension: String,
    ) -> GithubInfo {
        GithubInfo {
            content_type,
            path,
            name,
            download_url,
            extension,
        }
    }
}

pub fn parse_github_sub_page_body_to_m3u_link(
    body: &str,
    include_files: Vec<String>,
    valid_extensions: Vec<String>,
) -> Result<Vec<GithubInfo>, Error> {
    let regex = Regex::new(r#"(?m)<script type="application\/json" data-target="react-app.embeddedData">(.+?)<\/script>"#).unwrap();
    let result = regex.captures_iter(body);

    let mut urls = vec![];
    for mat in result {
        if mat.len() > 1 {
            let reg_text = mat.get(1).unwrap().as_str();
            if reg_text.contains("defaultBranch") {
                let json_pro: serde_json::error::Result<GithubSubPageProps> =
                    serde_json::from_str(reg_text);
                match json_pro {
                    Ok(props) => {
                        for value in props.payload.tree.items.iter() {
                            let mut is_save = true;
                            if include_files.len() > 0 {
                                is_save = false;
                                for f in include_files.iter() {
                                    if value.name.eq(f) {
                                        is_save = true
                                    }
                                }
                            }
                            if is_save && value.content_type.eq("file") {
                                for ext in &valid_extensions {
                                    if value.path.ends_with(ext) {
                                        let download_url = format!("https://raw.githubusercontent.com/{}/{}/refs/heads/{}/{}", props.payload.repo.owner_login, props.payload.repo.name, props.payload.repo.default_branch, value.path);
                                        urls.push(GithubInfo::new(
                                            value.content_type.clone(),
                                            value.path.clone(),
                                            value.name.clone(),
                                            download_url,
                                            ext.to_string(),
                                        ))
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(urls)
}
pub fn parse_github_home_page_body_to_m3u_link(
    body: &str,
    include_files: Vec<String>,
    valid_extensions: Vec<String>,
) -> Result<Vec<GithubInfo>, Error> {
    let regex = Regex::new(r#"(?m)<script type="application\/json" data-target="react-partial.embeddedData">(.+?)<\/script>"#).unwrap();
    let result = regex.captures_iter(body);

    let mut urls = vec![];
    for mat in result {
        if mat.len() > 1 {
            let reg_text = mat.get(1).unwrap().as_str();
            if reg_text.contains("defaultBranch") {
                let json_pro: serde_json::error::Result<GithubPageProps> =
                    serde_json::from_str(reg_text);
                match json_pro {
                    Ok(props) => {
                        for value in props.props.initial_payload.tree.items.iter() {
                            let mut is_save = true;
                            if include_files.len() > 0 {
                                is_save = false;
                                for f in include_files.iter() {
                                    if value.name.eq(f) {
                                        is_save = true
                                    }
                                }
                            }
                            if is_save && value.content_type.eq("file") {
                                for ext in &valid_extensions {
                                    if value.path.ends_with(ext) {
                                        let download_url = format!("https://raw.githubusercontent.com/{}/{}/refs/heads/{}/{}", props.props.initial_payload.repo.owner_login, props.props.initial_payload.repo.name, props.props.initial_payload.repo.default_branch, value.path);
                                        urls.push(GithubInfo::new(
                                            value.content_type.clone(),
                                            value.path.clone(),
                                            value.name.clone(),
                                            download_url,
                                            ext.to_string(),
                                        ))
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(urls)
}

async fn fetch_github_home_page(
    url: String,
    include_files: Vec<String>,
    valid_extensions: Vec<String>,
) -> Vec<GithubInfo> {
    let body = get_url_body(url).await.expect("Failed to get body");
    match parse_github_home_page_body_to_m3u_link(
        &body,
        include_files.clone(),
        valid_extensions.clone(),
    ) {
        Ok(list) => list,
        Err(_) => vec![],
    }
}

async fn fetch_github_sub_page(
    url: String,
    include_files: Vec<String>,
    valid_extensions: Vec<String>,
) -> Vec<GithubInfo> {
    let body = get_url_body(url).await.expect("Failed to get body");
    match parse_github_sub_page_body_to_m3u_link(
        &body,
        include_files.clone(),
        valid_extensions.clone(),
    ) {
        Ok(list) => list,
        Err(_) => vec![],
    }
}

#[derive(Debug)]
pub struct EpgM3u8Info {
    pub name: String,
    pub revolution: String,
    pub reg: String,
    pub check: String,
    pub urls: Vec<String>,
}

fn epg_live_stream_html_parse(html: &str) -> Vec<EpgM3u8Info> {
    let mut result: Vec<Vec<HashMap<String, Vec<String>>>> = Vec::new();

    // 匹配行的正则表达式
    let tr_regex = Regex::new(r"<tr>([\s\S]*?)<\/tr>").unwrap();
    // 匹配单元格的正则表达式
    let td_regex = Regex::new(r"<td>([\s\S]*?)<\/td>").unwrap();
    // 匹配链接的正则表达式
    let link_regex = Regex::new(r#"(?m)<a href="([\s\S]+?)""#).unwrap();

    for tr_match in tr_regex.captures_iter(html) {
        if tr_match.len() < 2 {
            continue;
        }
        let mut row_data: Vec<HashMap<String, Vec<String>>> = Vec::new();

        // 提取当前行中的所有 td
        let row_content = &tr_match[1]; // 当前行的内容

        for td_match in td_regex.captures_iter(row_content) {
            if td_match.len() < 2 {
                continue;
            }
            let cell_content = &td_match[1];

            let mut links = Vec::new(); // 存储链接

            for link_match in link_regex.captures_iter(cell_content) {
                if link_match.len() < 2 {
                    continue;
                }
                links.push(link_match[1].to_string());
            }

            // 处理没有链接的普通文本
            let text_without_links = link_regex
                .replace_all(cell_content, "")
                .to_string()
                .replace(&['<', '>'][..], "")
                .trim()
                .to_string();

            let mut data = HashMap::new();
            data.insert("text".to_string(), vec![text_without_links]);
            data.insert("links".to_string(), links);
            row_data.push(data);
        }

        // 排除表头行
        if !row_data.is_empty() {
            result.push(row_data);
        }
    }

    let mut rows: Vec<EpgM3u8Info> = Vec::new();
    for row in result {
        if row.len() >= 5 {
            rows.push(EpgM3u8Info {
                name: row[0]["text"][0].clone(),
                revolution: row[1]["text"][0].clone(),
                reg: row[2]["text"][0].clone(),
                check: row[3]["text"][0].clone(),
                urls: row[4]["links"].clone(),
            });
        }
    }
    rows
}

fn epg_list_to_m3u_file(list: Vec<EpgM3u8Info>, file_name: String) -> Result<(), Error> {
    let mut result = M3uObjectList::new();
    let mut m3u8_list = vec![];
    for val in list {
        for url in val.urls {
            let mut one = M3uObject::new();
            one.set_name(val.name.clone());
            one.set_search_name(val.name.clone().to_lowercase());
            one.set_url(url);
            one.generate_raw();
            m3u8_list.push(one);
        }
    }
    result.set_list(m3u8_list);
    result.generate_m3u_file(file_name.clone(), false);
    Ok(())
}

async fn fetch_epg_page(url: String) -> Vec<EpgM3u8Info> {
    let body = get_url_body(url).await.expect("Failed to get body");
    epg_live_stream_html_parse(body.as_str())
}

async fn get_url_body(_url: String) -> Result<String, Error> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let resp = client.get(_url.to_owned()).send().await.unwrap();
    if resp.status().is_success() {
        Ok(resp.text().await.unwrap())
    } else {
        Ok("".to_string())
    }
}

fn check_search_data_exists() -> std::io::Result<bool> {
    let path = std::path::Path::new(INPUT_SEARCH_FOLDER);
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                return Ok(true);
            }
        }
        Ok(false)
    } else {
        Ok(false)
    }
}

async fn init_search_data() -> Result<(), Error> {
    let exists = check_search_data_exists().expect("Failed to check search data");
    if exists {
        return Ok(());
    }
    // 初始化search文件夹
    let _ = create_folder(&INPUT_SEARCH_FOLDER.to_string()).expect("文件夹创建失败");

    // 下线相关文件
    let config = read_search_configs().await.expect("配置获取失败");
    let mut i = 0;
    for fetch_values in config.source {
        match fetch_values.parse_type {
            SearchConfigParseType::EpgLivestreamPageUrl => {
                if fetch_values.urls.len() > 0 {
                    let fetch_url = fetch_values.urls[0].clone();
                    let list = fetch_epg_page(fetch_url.clone()).await;
                    // 将list转换成m3u文件
                    let save_status =
                        epg_list_to_m3u_file(list, format!("{}100-{}.m3u", INPUT_SEARCH_FOLDER, i));
                    match save_status {
                        Ok(()) => {
                            info!("{} file save success", fetch_url.clone());
                        }
                        Err(e) => {
                            error!("{} file save failed: {}", fetch_url.clone(), e);
                        }
                    }
                    i += 1;
                }
            }
            SearchConfigParseType::GithubHomeUrl => {
                for url in fetch_values.urls {
                    let m3u_and_txt_files = fetch_github_home_page(
                        url.clone(),
                        fetch_values.include_files.clone(),
                        config.extensions.clone(),
                    )
                    .await;
                    debug!("{:?}", m3u_and_txt_files);
                    // 下载m3u文件
                    for _url in m3u_and_txt_files {
                        i += 1;
                        save_data(
                            _url.download_url.clone(),
                            format!("{}200-{}{}", INPUT_SEARCH_FOLDER, i, _url.extension),
                        )
                        .await;
                    }
                }
            }
            SearchConfigParseType::GithubSubPageUrl => {
                for url in fetch_values.urls {
                    let m3u_and_txt_files = fetch_github_sub_page(
                        url.clone(),
                        fetch_values.include_files.clone(),
                        config.extensions.clone(),
                    )
                    .await;
                    debug!("{:?}", m3u_and_txt_files);
                    // 下载m3u文件
                    for _url in m3u_and_txt_files {
                        i += 1;
                        save_data(
                            _url.download_url.clone(),
                            format!("{}300-{}{}", INPUT_SEARCH_FOLDER, i, _url.extension),
                        )
                        .await;
                    }
                }
            }
            SearchConfigParseType::RawSources => {
                for url in fetch_values.urls {
                    let mut ext = ".m3u";
                    if url.contains(".txt") {
                        ext = ".txt"
                    }
                    i += 1;
                    save_data(
                        url.clone(),
                        format!("{}400-{}{}", INPUT_SEARCH_FOLDER, i, ext),
                    )
                    .await;
                }
            }
        }
    }
    Ok(())
}

async fn save_data(url: String, save_name: String) {
    let fetch_url = url.clone();
    let save_status = download_target_files(fetch_url.clone(), save_name.to_string()).await;
    match save_status {
        Ok(_) => {
            info!("{} file save success", fetch_url.clone());
        }
        Err(e) => {
            error!("{} file save failed: {}", fetch_url.clone(), e);
        }
    }
}

async fn download_target_files(_url: String, save_path: String) -> Result<(), Error> {
    let contents = crate::common::util::get_url_body(_url.clone(), 20000)
        .await
        .expect("Failed to get body");
    // 创建一个新文件，如果文件已存在，则会覆盖它
    let mut file = fs::File::create(save_path).expect("file create failed");

    // 将字符串内容写入文件
    file.write_all(contents.as_bytes())
        .expect("file save filed"); // 也可以使用 write 方法
    Ok(())
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SearchConfigs {
    pub source: Vec<SearchSource>,
    pub extensions: Vec<String>,
    pub search_list: Vec<SearchListItem>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SearchSource {
    pub urls: Vec<String>,
    pub include_files: Vec<String>,
    pub parse_type: SearchConfigParseType,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum SearchConfigParseType {
    #[serde(rename = "epg-livestream-page")]
    EpgLivestreamPageUrl,
    #[serde(rename = "github-home-page")]
    GithubHomeUrl,
    #[serde(rename = "github-sub-page")]
    GithubSubPageUrl,
    #[serde(rename = "raw-source")]
    RawSources,
}

impl std::str::FromStr for SearchConfigParseType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "epg-livestream-page" => Ok(SearchConfigParseType::EpgLivestreamPageUrl),
            "github-home-page" => Ok(SearchConfigParseType::GithubHomeUrl),
            "github-sub-page" => Ok(SearchConfigParseType::GithubSubPageUrl),
            "raw-source" => Ok(SearchConfigParseType::RawSources),
            _ => Err(format!("Unknown parse type: {}", s)),
        }
    }
}

impl std::fmt::Display for SearchConfigParseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchConfigParseType::EpgLivestreamPageUrl => write!(f, "epg-livestream-page"),
            SearchConfigParseType::GithubHomeUrl => write!(f, "github-home-page"),
            SearchConfigParseType::GithubSubPageUrl => write!(f, "github-sub-page"),
            SearchConfigParseType::RawSources => write!(f, "raw-source"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SearchListItem {
    pub id: String,
    pub config: Vec<SearchConfig>,
    pub result: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SearchConfig {
    pub search_name: Vec<String>,
    pub save_name: String,
    pub full_match: bool,
    pub exclude_url: Vec<String>,
    pub exclude_host: Vec<String>,
}

pub async fn read_search_configs() -> Result<SearchConfigs, Box<dyn std::error::Error>> {
    // 从config模块读取搜索配置
    let search_config = match config::get_search() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to read search config: {}", e);
            return Err(Box::new(e));
        }
    };

    // 转换配置格式
    let mut configs = SearchConfigs {
        source: Vec::new(),
        extensions: Vec::new(),
        search_list: Vec::new(),
    };

    // 转换搜索源
    for source in search_config.source {
        configs.source.push(SearchSource {
            urls: source.urls,
            include_files: source.include_files,
            parse_type: source.parse_type.parse().unwrap_or_else(|_| {
                error!("Invalid parse type: {}", source.parse_type);
                SearchConfigParseType::RawSources
            }),
        });
    }

    // 转换扩展名
    configs.extensions = search_config.extensions;

    // 转换搜索列表
    for item in search_config.search_list {
        let mut search_item = SearchListItem {
            id: item.id,
            config: Vec::new(),
            result: item.result,
        };

        // 转换搜索配置
        for config in item.config {
            search_item.config.push(SearchConfig {
                search_name: config.search_name,
                save_name: config.save_name,
                full_match: config.full_match,
                exclude_url: config.exclude_url,
                exclude_host: config.exclude_host,
            });
        }

        configs.search_list.push(search_item);
    }

    Ok(configs)
}

pub async fn do_search(search_name: String, thumbnail: bool, concurrent: i32) -> Result<(), Error> {
    match init_search_data().await {
        Ok(()) => {
            let mut m3u_data = load_m3u_data().expect("load m3u data failed");
            m3u_data
                .search(SearchOptions {
                    search_name: search_name.clone(),
                    full_match: false,
                    ipv4: true,
                    ipv6: true,
                    exclude_url: vec![],
                    exclude_host: vec![],
                    quality: vec![],
                })
                .await;
            info!("list1 --- {}", m3u_data.clone().get_list().len());
            if thumbnail {
                m3u_data.generate_thumbnail(concurrent).await;
            }

            info!("list2 --- {}", m3u_data.clone().get_list().len());
            m3u_data.generate_m3u_file(String::from("./static/output/1111.m3u"), true);
            m3u_data.generate_text_file(String::from("./static/output/1111.txt"));
            Ok(())
        }
        Err(e) => {
            error!("Failed to search: {}", e);
            Err(e)
        }
    }
}

pub fn clear_search_folder() -> std::io::Result<()> {
    let p = INPUT_SEARCH_FOLDER;
    fs::remove_dir_all(p)?;
    info!("Deleted directory: {}", p);
    Ok(())
}

fn load_m3u_data() -> std::io::Result<M3uObjectList> {
    let p = INPUT_SEARCH_FOLDER;
    let path = std::path::Path::new(p);
    let mut file_names = vec![];
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                file_names.push(entry.file_name().into_string().unwrap());
            }
        }
    }
    let mut contents = vec![];
    for file_name in file_names {
        let content = fs::read_to_string(format!("{}{}", p, file_name.clone()))?;
        contents.push(content)
    }
    let result = from_body_arr(contents, vec![], vec![], true, true);
    Ok(result)
}
pub fn generate_channel_thumbnail_folder_name() -> String {
    // 获取当前本地时间
    let now = Local::now();

    // 获取年、月、日
    let year = now.year();
    let month = now.month();
    let day = now.day();
    let folder = format!("{}{}{}{}/", OUTPUT_THUMBNAIL_FOLDER, year, month, day);
    if !folder_exists(&folder) {
        fs::create_dir_all(folder.clone()).unwrap()
    }
    folder
}

use crate::common::m3u::m3u::list_str2obj;
use crate::common::{M3uObject, M3uObjectList, SearchParams};
use crate::config;
use crate::config::epg::get_epg_config;
use crate::r#const::constant::{INPUT_EPG_FOLDER, INPUT_SEARCH_FOLDER, OUTPUT_THUMBNAIL_FOLDER};
use crate::utils::{create_folder, folder_exists};
use chrono::{DateTime, Datelike, FixedOffset, Local, NaiveDateTime, TimeZone};
use clap::ValueHint::Url;
use flate2::read::GzDecoder;
use log::{debug, error, info};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, Error, ErrorKind, Read, Write};
use std::string::String;
use std::{fs, vec};
use zip::read::ZipArchive;

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
    // pub content_type: String,
    // pub path: String,
    // pub name: String,
    pub download_url: String,
    pub extension: String, //.txt, .m3u
}

impl GithubInfo {
    pub fn new(
        // content_type: String,
        // path: String,
        // name: String,
        download_url: String,
        extension: String,
    ) -> GithubInfo {
        GithubInfo {
            // content_type,
            // path,
            // name,
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
                                            // value.content_type.clone(),
                                            // value.path.clone(),
                                            // value.name.clone(),
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
                                            // value.content_type.clone(),
                                            // value.path.clone(),
                                            // value.name.clone(),
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
    match get_url_body(url.clone()).await {
        Ok(body) => parse_github_home_page_body_to_m3u_link(
            &body,
            include_files.clone(),
            valid_extensions.clone(),
        )
        .unwrap_or_else(|_| vec![]),
        Err(e) => {
            error!("Failed to fetch github home page {}: {}", url, e);
            vec![]
        }
    }
}

async fn fetch_github_sub_page(
    url: String,
    include_files: Vec<String>,
    valid_extensions: Vec<String>,
) -> Vec<GithubInfo> {
    match get_url_body(url.clone()).await {
        Ok(body) => parse_github_sub_page_body_to_m3u_link(
            &body,
            include_files.clone(),
            valid_extensions.clone(),
        )
        .unwrap_or_else(|_| vec![]),
        Err(e) => {
            error!("Failed to fetch github sub page {}: {}", url, e);
            vec![]
        }
    }
}

#[derive(Debug)]
pub struct EpgM3u8Info {
    pub name: String,
    // pub revolution: String,
    // pub reg: String,
    // pub check: String,
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
                // revolution: row[1]["text"][0].clone(),
                // reg: row[2]["text"][0].clone(),
                // check: row[3]["text"][0].clone(),
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
    result.generate_m3u_file(file_name.clone(), false)?;
    Ok(())
}

async fn fetch_epg_page(url: String) -> Vec<EpgM3u8Info> {
    match get_url_body(url.clone()).await {
        Ok(body) => epg_live_stream_html_parse(body.as_str()),
        Err(e) => {
            error!("Failed to fetch epg page {}: {}", url, e);
            vec![]
        }
    }
}

async fn get_url_body(_url: String) -> Result<String, Error> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let resp = client.get(_url.to_owned()).send().await;
    return match resp {
        Ok(res) => {
            if res.status().is_success() {
                Ok(res.text().await.unwrap())
            } else {
                Ok("".to_string())
            }
        }
        Err(e) => {
            error!("get url body error: {}", e);
            Err(Error::new(ErrorKind::Other, format!("error {}", e)))
        }
    };
}

fn check_epg_data_exists() -> std::io::Result<bool> {
    let folder_name = get_epg_folder();
    let path = std::path::Path::new(&folder_name);
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

fn check_search_data_exists() -> std::io::Result<bool> {
    let folder_name = get_search_folder();
    let path = std::path::Path::new(&folder_name);
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

pub fn get_epg_folder() -> String {
    let now = chrono::Local::now();
    format!(
        "{}{:04}{:02}{:02}/",
        INPUT_EPG_FOLDER,
        now.year(),
        now.month(),
        now.day()
    )
}

/// 从 URL 取路径最后一段作为文件名
fn filename_from_epg_url(url_str: &str) -> String {
    url::Url::parse(url_str)
        .ok()
        .and_then(|u| u.path_segments().and_then(|s| s.last().map(String::from)))
        .unwrap_or_default()
}

/// 下载 URL 返回字节
async fn get_url_bytes(url: &str) -> Result<Vec<u8>, Error> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
    let bytes = client
        .get(url)
        .send()
        .await
        .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?
        .bytes()
        .await
        .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
    Ok(bytes.to_vec())
}

/// 解压 gzip 字节流
fn decompress_gz(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let mut decoder = GzDecoder::new(Cursor::new(bytes));
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
    Ok(out)
}

/// 将 zip 字节解压到指定目录（仅使用文件名，避免路径穿越）
fn extract_zip_to_folder(bytes: &[u8], folder: &str) -> Result<(), Error> {
    let cursor = Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
        let name = file.name().to_string();
        if name.ends_with('/') {
            continue;
        }
        let file_name = std::path::Path::new(&name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&name);
        let out_path = format!("{}{}", folder, file_name);
        if let Ok(mut out_file) = fs::File::create(&out_path) {
            std::io::copy(&mut file, &mut out_file)
                .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
        }
    }
    Ok(())
}

pub fn get_search_folder() -> String {
    let now = chrono::Local::now();
    format!(
        "{}{:04}{:02}{:02}/",
        INPUT_SEARCH_FOLDER,
        now.year(),
        now.month(),
        now.day()
    )
}

const EPG_SUPPORT_ZIP_EXTENSION: [&str; 2] = ["zip", "gz"];

pub fn get_url_extension(url_str: &str) -> String {
    match url::Url::parse(url_str) {
        Ok(url) => {
            let ex: Vec<&str> = url.path().split('.').collect();
            if ex.len() > 1 {
                ex[ex.len() - 1].to_string()
            } else {
                String::new()
            }
        }
        Err(e) => {
            info!("get url extension error {}", e);
            String::new()
        }
    }
}

struct EpgParseData {
    pub xml_list: Vec<String>,
    pub zip_list: Vec<String>,
}

impl EpgParseData {
    fn new() -> EpgParseData {
        EpgParseData {
            xml_list: vec![],
            zip_list: vec![],
        }
    }

    pub fn set_xml_list(&mut self, xml_list: Vec<String>) {
        self.xml_list = xml_list;
    }

    pub fn set_zip_list(&mut self, zip_list: Vec<String>) {
        self.zip_list = zip_list;
    }

    /// 将 xml_list 与 zip_list 下载到 static/epg/当前年月日/，zip/gz 会解压
    pub async fn download(&self) -> Result<(), Error> {
        let folder = get_epg_folder();
        if let Err(e) = create_folder(&folder) {
            return Err(Error::new(
                ErrorKind::Other,
                format!("创建 epg 文件夹失败: {}", e),
            ));
        }

        // 1. 下载 xml 列表
        for (i, url) in self.xml_list.iter().enumerate() {
            match get_url_bytes(url).await {
                Ok(bytes) => {
                    let name = filename_from_epg_url(url);
                    let filename = if name.is_empty() || !name.ends_with(".xml") {
                        format!("epg_xml_{}.xml", i)
                    } else {
                        name
                    };
                    let path = format!("{}{}", folder, filename);
                    if let Err(e) = fs::write(&path, &bytes) {
                        error!("保存 xml 失败 {} -> {}: {}", url, path, e);
                    } else {
                        info!("epg xml 已保存: {}", path);
                    }
                }
                Err(e) => {
                    error!("下载 epg xml 失败 {}: {}", url, e);
                }
            }
        }

        // 2. 下载 zip/gz 列表并解压
        for (i, url) in self.zip_list.iter().enumerate() {
            match get_url_bytes(url).await {
                Ok(bytes) => {
                    let ext = get_url_extension(url);
                    if ext == "gz" {
                        match decompress_gz(&bytes) {
                            Ok(decoded) => {
                                let name = filename_from_epg_url(url);
                                let out_name = name
                                    .strip_suffix(".gz")
                                    .map(String::from)
                                    .unwrap_or_else(|| format!("epg_gz_{}.xml", i));
                                let path = format!("{}{}", folder, out_name);
                                if let Err(e) = fs::write(&path, &decoded) {
                                    error!("保存 gz 解压文件失败 {} -> {}: {}", url, path, e);
                                } else {
                                    info!("epg gz 已解压保存: {}", path);
                                }
                            }
                            Err(e) => error!("解压 gz 失败 {}: {}", url, e),
                        }
                    } else if ext == "zip" {
                        if let Err(e) = extract_zip_to_folder(&bytes, &folder) {
                            error!("解压 zip 失败 {}: {}", url, e);
                        } else {
                            info!("epg zip 已解压到: {}", folder);
                        }
                    }
                }
                Err(e) => {
                    error!("下载 epg 压缩文件失败 {}: {}", url, e);
                }
            }
        }

        Ok(())
    }
}

pub fn init_epg_data() -> EpgParseData {
    let exists = check_epg_data_exists().expect("Failed to check search data");
    let mut epg_data = EpgParseData::new();
    if exists {
        return epg_data;
    }
    // 初始化search文件夹
    let _ = create_folder(&get_epg_folder()).expect("文件夹创建失败");
    // 下线相关文件
    let config = get_epg_config();
    let mut xml_url: Vec<String> = vec![];
    let mut zip_url: Vec<String> = vec![];
    for c in config.source.clone().list.iter() {
        let ext = get_url_extension(c);
        if ext.eq("xml") {
            xml_url.push(c.clone());
        } else {
            for e in EPG_SUPPORT_ZIP_EXTENSION {
                if c.contains(e) {
                    zip_url.push(c.clone());
                    break;
                }
            }
        }
    }
    epg_data.set_xml_list(xml_url);
    epg_data.set_zip_list(zip_url);
    epg_data
}

pub async fn init_search_data() -> Result<(), Error> {
    let exists = check_search_data_exists().expect("Failed to check search data");
    if exists {
        return Ok(());
    }
    // 初始化search文件夹
    let _ = create_folder(&get_search_folder()).expect("文件夹创建失败");

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
                        epg_list_to_m3u_file(list, format!("{}100-{}.m3u", get_search_folder(), i));
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
                for url in fetch_values.urls.clone() {
                    let m3u_and_txt_files = fetch_github_home_page(
                        url.clone(),
                        fetch_values.include_files.clone(),
                        fetch_values.extensions.clone(),
                    )
                    .await;
                    debug!("{:?}", m3u_and_txt_files);
                    // 下载m3u文件
                    for _url in m3u_and_txt_files {
                        i += 1;
                        save_data(
                            _url.download_url.clone(),
                            format!("{}200-{}{}", get_search_folder(), i, _url.extension),
                        )
                        .await;
                    }
                }
            }
            SearchConfigParseType::GithubSubPageUrl => {
                for url in fetch_values.urls.clone() {
                    let m3u_and_txt_files = fetch_github_sub_page(
                        url.clone(),
                        fetch_values.include_files.clone(),
                        fetch_values.extensions.clone(),
                    )
                    .await;
                    debug!("{:?}", m3u_and_txt_files);
                    // 下载m3u文件
                    for _url in m3u_and_txt_files {
                        i += 1;
                        save_data(
                            _url.download_url.clone(),
                            format!("{}300-{}{}", get_search_folder(), i, _url.extension),
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
                        format!("{}400-{}{}", get_search_folder(), i, ext),
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
    match crate::common::util::get_url_body(_url.clone(), 20000).await {
        Ok(contents) => {
            // 创建一个新文件，如果文件已存在，则会覆盖它
            let mut file = fs::File::create(save_path)?;

            // 将字符串内容写入文件
            file.write_all(contents.as_bytes())?; // 也可以使用 write 方法
            Ok(())
        }
        Err(e) => Err(Error::new(
            ErrorKind::Other,
            format!("Failed to download file {}: {}", _url, e),
        )),
    }
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
    pub extensions: Vec<String>,
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
    let search_config = config::search::get_search_config();

    // 转换配置格式
    let mut configs = SearchConfigs {
        source: Vec::new(),
        extensions: Vec::new(),
        search_list: Vec::new(),
    };

    // 转换搜索源，同时收集所有扩展名
    for source in search_config.source {
        // 收集扩展名（去重）
        for ext in &source.extensions {
            if !configs.extensions.contains(ext) {
                configs.extensions.push(ext.clone());
            }
        }

        configs.source.push(SearchSource {
            urls: source.urls,
            include_files: source.include_files,
            extensions: source.extensions,
            parse_type: source.parse_type.parse().unwrap_or_else(|_| {
                error!("Invalid parse type: {}", source.parse_type);
                SearchConfigParseType::RawSources
            }),
        });
    }

    Ok(configs)
}

pub async fn do_search(search_params: SearchParams) -> Result<(), Error> {
    match init_search_data().await {
        Ok(()) => {
            let mut m3u_data = load_m3u_data()?;
            m3u_data.t2s();
            m3u_data.search(search_params.search_options).await;
            if search_params.thumbnail {
                m3u_data
                    .generate_thumbnail(search_params.concurrent, search_params.timeout)
                    .await;
            }
            info!("list2 --- total {}", m3u_data.get_list_len());
            m3u_data
                .output_file(search_params.output_file.clone(), false)
                .await;
            Ok(())
        }
        Err(e) => {
            error!("Failed to search: {}", e);
            Err(e)
        }
    }
}

pub fn clear_search_folder() -> std::io::Result<()> {
    let p = get_search_folder();
    fs::remove_dir_all(p.clone())?;
    info!("Deleted directory: {}", &p.clone());
    Ok(())
}

fn load_m3u_data() -> std::io::Result<M3uObjectList> {
    let p = get_search_folder();
    let path = std::path::Path::new(&p);
    let mut file_names = vec![];
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                if let Ok(name) = entry.file_name().into_string() {
                    file_names.push(name);
                }
            }
        }
    }
    let mut contents = vec![];
    for file_name in file_names {
        let content = fs::read_to_string(format!("{}{}", p, file_name.clone()))?;
        contents.push(content)
    }
    let result = list_str2obj(contents, true);
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
        if let Err(e) = fs::create_dir_all(folder.clone()) {
            error!("Failed to create thumbnail folder: {}", e);
        }
    }
    folder
}

pub fn parse_epg_time_str(s: &str) -> i64 {
    // 分离本地时间部分与偏移部分
    let (dt_part, offset_part) = s.split_at(14); // "20260205092300" 和 " +0800"
    let offset_str = offset_part.trim(); // "+0800"

    // 解析本地时间: "YYYYMMDDHHMMSS"
    let naive = NaiveDateTime::parse_from_str(dt_part, "%Y%m%d%H%M%S")
        .expect("parse naive datetime failed");

    // 解析时区偏移: "+HHMM" 或 "-HHMM"
    let offset = FixedOffset::from(offset_str.parse().unwrap());

    // 组合成带偏移的时间
    let dt_with_offset = offset
        .from_local_datetime(&naive)
        .single()
        .expect("ambiguous or nonexistent local time");

    let ts_millis = dt_with_offset.timestamp_millis();

    ts_millis
}

#[cfg(test)]
mod tests {
    use super::{get_url_extension, init_epg_data, parse_epg_time_str};
    use crate::epg_xml::{parse_epg_xml_str, Channel, DisplayName, Programme, Tv};
    use std::collections::HashMap;

    #[test]
    fn convert_to_timestamp() {
        println!("{}", parse_epg_time_str("20260205092300 +0800"));
        println!("{}", parse_epg_time_str("20260205000000 +0800"));
    }

    #[test]
    fn generate_channel_thumbnail_folder_name() {
        // 方式一：分两步（先得到 Rust 对象，再转 JSON）
        let xml = std::fs::read_to_string("static/epg/20260211/epg").unwrap();
        let tv: Tv = parse_epg_xml_str(&xml).unwrap();
        let mut channel_hash_map = HashMap::new();
        let mut channel_list_map: HashMap<String, Vec<Programme>> = HashMap::new();
        for i in tv.channels {
            for c in i.display_names {
                channel_hash_map.insert(c.value.to_lowercase(), i.id.clone());
            }
        }
        for mut i in tv.programmes {
            let mut list = vec![];
            let data = channel_list_map.get(&i.channel);
            if let Some(hash_list) = data {
                list = hash_list.to_vec();
            }
            i.to_unixtime();
            list.push(i.clone());
            channel_list_map.insert(i.channel.clone(), list);
        }
        for (index, mut p_list) in channel_list_map.clone() {
            p_list.sort_by(|a, b| a.start_unix.cmp(&b.start_unix));
            channel_list_map.insert(index.clone(), p_list);
        }
        let channel_name = "CCTV-13高清".to_string();
        let channel_id = channel_hash_map.get(channel_name.to_lowercase().as_str());
        if let Some(channel_id) = channel_id {
            let mut channels = vec![];
            let mut one_channel = Channel::new();
            one_channel.set_id(channel_id.to_string());
            let mut displays = vec![];
            let mut one_display_channel_name = DisplayName::new();
            one_display_channel_name.set_lang("zh".to_string());
            one_display_channel_name.set_value(channel_name);
            displays.push(one_display_channel_name);
            one_channel.set_display_names(displays);
            channels.push(one_channel);
            let mut programs = vec![];
            for (k, v) in channel_list_map.clone() {
                programs = v;
            }
            let mut new_epg = Tv::new();
            new_epg.set_generator_info_name("iptv-checker generate".to_string());
            new_epg.set_generator_info_url("http://127.0.0.1:8081".to_string());
            new_epg.set_channels(channels);
            new_epg.set_programmes(programs);

            let _ = new_epg.to_epg_xml_file("./static/epg/1111.xml".to_string());
        } else {
            println!("channel not found");
        }
    }

    #[tokio::test]
    async fn test_init_epg_data() {
        let data = init_epg_data();
        data.download().await.unwrap();
    }
}

use crate::common::check::check::check_link_is_valid;
use crate::common::cmd::capture_stream_pic;
use crate::common::task::md5_str;
use crate::common::CheckDataStatus::{Failed, Success, Unchecked};
use crate::common::FfmpegInfo;
use crate::common::QualityType::QualityUnknown;
use crate::common::SourceType::{Normal, Quota};
use crate::search::generate_channel_thumbnail_folder_name;
use crate::utils::{remove_other_char};
use actix_rt::time;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::cmp::PartialEq;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::{thread, vec};
use std::time::Duration;
use crate::common::translate::trad_to_simp;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct M3uExtend {
    pub group_title: String,       //group title
    pub tv_logo: String,           //台标
    pub tv_language: String,       //语言
    pub tv_name: String,           //名称
    pub tv_country: String,        //国家
    pub tv_id: String,             //电视id
    pub user_agent: String,        // user-agent
    pub thumbnail: Option<String>, //缩略图
}

impl M3uExtend {
    pub fn new() -> M3uExtend {
        M3uExtend {
            group_title: "".to_string(),
            tv_logo: "".to_string(),
            tv_language: "".to_string(),
            tv_country: "".to_string(),
            tv_id: "".to_string(),
            user_agent: "".to_string(),
            thumbnail: None,
            tv_name: "".to_string(),
        }
    }

    pub fn set_group_title(&mut self, group_title: String) {
        self.group_title = group_title
    }

    pub fn set_tv_logo(&mut self, tv_logo: String) {
        self.tv_logo = tv_logo
    }

    pub fn set_thumbnail(&mut self, thumbnail: String) {
        self.thumbnail = Some(thumbnail)
    }

    pub fn set_tv_language(&mut self, tv_language: String) {
        self.tv_language = tv_language
    }

    pub fn set_tv_country(&mut self, tv_country: String) {
        self.tv_country = tv_country
    }

    pub fn set_tv_id(&mut self, tv_id: String) {
        self.tv_id = tv_id
    }

    pub fn set_tv_name(&mut self, tv_name: String) {
        self.tv_name = tv_name
    }

    pub fn set_user_agent(&mut self, user_agent: String) {
        self.user_agent = user_agent
    }
}

#[derive(Debug, Clone)]
pub struct M3uObject {
    index: i32,
    //索引
    url: String,
    //连接url
    name: String,
    //显示名称
    extend: Option<M3uExtend>,
    //扩展信息
    search_name: String,
    //搜索名称
    raw: String,
    //原始的m3u文件信息
    status: CheckDataStatus,
    //当前状态
    other_status: OtherStatus, //其它状态
}

impl M3uObject {
    pub fn new() -> M3uObject {
        return M3uObject {
            index: 0,
            url: "".to_string(),
            name: "".to_string(),
            extend: None,
            search_name: "".to_string(),
            raw: "".to_string(),
            status: Unchecked,
            other_status: OtherStatus::new(),
        };
    }

    pub fn get_obj(self) -> M3uObject {
        self
    }

    pub fn check_by_block(&mut self, request_time: i32, ffmpeg_check: bool, not_http_skip: bool) {
        let url = self.url.clone();
        let _log_url = url.clone();
        let result = actix_rt::System::new().block_on(check_link_is_valid(
            url,
            request_time as u64,
            ffmpeg_check,
            not_http_skip,
        ));
        debug!("url is: {} result: {:?}", self.url.clone(), result);
        return match result {
            Ok(data) => {
                let mut o_status = OtherStatus::new();
                o_status.set_delay(data.delay);
                o_status.set_ffmpeg_info(data.ffmpeg_info);
                self.set_other_status(o_status);
                self.set_status(Success);
            }
            Err(_e) => self.set_status(Failed),
        };
    }

    pub fn set_index(&mut self, index: i32) {
        self.index = index;
    }
    pub fn get_extend(&self) -> Option<M3uExtend> {
        self.extend.clone()
    }

    // 提供不产生额外克隆的访问器，避免不必要的内存开销
    pub fn get_extend_ref(&self) -> Option<&M3uExtend> {
        self.extend.as_ref()
    }

    pub fn get_extend_mut(&mut self) -> Option<&mut M3uExtend> {
        self.extend.as_mut()
    }

    // 将 extend 的所有权移出（移动而非克隆），使用后 self.extend 变为 None
    pub fn take_extend(&mut self) -> Option<M3uExtend> {
        self.extend.take()
    }
    
    pub fn t2s(&mut self) {
        let name = self.name.clone();
        let rename = trad_to_simp(&name);
        self.search_name = rename.clone().to_lowercase();
        self.name = rename.clone();
        self.raw = self
            .raw
            .replace(name.clone().as_str(), rename.clone().as_str());

        if self.extend.is_some() {
            let mut ext = self.extend.clone().unwrap();
            let old_tv_name = ext.tv_name.clone();
            let old_tv_language = ext.tv_language.clone();
            let old_tv_country = ext.tv_country.clone();
            let old_group_title = ext.group_title.clone();

            let tv_name_s = trad_to_simp(&old_tv_name);
            let tv_language_s = trad_to_simp(&old_tv_language);
            let tv_country_s = trad_to_simp(&old_tv_country);
            let group_title_s = trad_to_simp(&old_group_title);

            ext.tv_name = tv_name_s.clone();
            ext.tv_language = tv_language_s.clone();
            ext.tv_country = tv_country_s.clone();
            ext.group_title = group_title_s.clone();

            // 更新 extend 和 raw 内容中的繁体字段为简体
            self.extend = Some(ext);
            self.raw = self
            .raw
            .replace(&old_tv_name, &tv_name_s)
            .replace(&old_tv_language, &tv_language_s)
            .replace(&old_tv_country, &tv_country_s)
            .replace(&old_group_title, &group_title_s);
        }
    }

    pub fn remove_useless_char(&mut self) {
        let name = self.name.clone();
        let rename = remove_other_char(self.name.clone());
        self.name = rename.clone();
        self.search_name = rename.clone().to_lowercase();
        self.raw = self
            .raw
            .replace(name.clone().as_str(), rename.clone().as_str());
    }

    pub fn set_url(&mut self, url: String) {
        self.url = url
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name
    }

    pub fn get_url(&self) -> String {
        self.url.clone()
    }

    // pub fn get_name(&self) -> String {
    //     self.name.clone()
    // }

    pub fn set_search_name(&mut self, search_name: String) {
        self.search_name = search_name.to_lowercase()
    }

    pub fn set_raw(&mut self, raw: String) {
        self.raw = raw
    }

    pub fn generate_raw(&mut self) {
        let mut header = String::from("#EXTINF:-1");
        if let Some(extend) = &self.extend {
            if !extend.tv_name.is_empty() {
                header.push_str(&format!(" tvg-name=\"{}\"", extend.tv_name));
            }
            if !extend.tv_id.is_empty() {
                header.push_str(&format!(" tvg-id=\"{}\"", extend.tv_id));
            }
            if !extend.tv_logo.is_empty() {
                header.push_str(&format!(" tvg-logo=\"{}\"", extend.tv_logo));
            }
            if !extend.group_title.is_empty() {
                header.push_str(&format!(" group-title=\"{}\"", extend.group_title));
            }
            if !extend.tv_country.is_empty() {
                header.push_str(&format!(" tvg-country=\"{}\"", extend.tv_country));
            }
            if !extend.tv_language.is_empty() {
                header.push_str(&format!(" tvg-language=\"{}\"", extend.tv_language));
            }
        }
        self.raw = format!("{},{}\n{}", header, self.name, self.url);
    }

    pub fn set_extend(&mut self, extend: M3uExtend) {
        self.extend = Some(extend)
    }

    pub fn set_other_status(&mut self, other_status: OtherStatus) {
        self.other_status = other_status
    }

    pub fn set_status(&mut self, status: CheckDataStatus) {
        self.status = status;
    }
}

#[derive(Copy, Clone)]
pub struct M3uObjectListCounter {
    check_index: i32, // 当前检查的索引
    total: i32,       // 总数
    success_count: i32, // 成功数据
                      // repeat_channel_count: i32,    // 频道名称重复数
                      // no_repeat_channel_count: i32, // 频道名称最终保存数
}

#[derive(Clone)]
pub struct M3uObjectList {
    header: Option<M3uExt>,
    list: Vec<M3uObject>,
    counter: M3uObjectListCounter,
}

impl M3uObjectListCounter {
    pub fn new() -> M3uObjectListCounter {
        M3uObjectListCounter {
            check_index: 0,
            total: 0,
            success_count: 0,
            // repeat_channel_count: 0,
            // no_repeat_channel_count: 0,
        }
    }

    pub fn now_index_incr(&mut self) {
        let mut index = self.check_index;
        index += 1;
        self.check_index = index
    }

    pub fn set_success_count(&mut self, success_count: i32) {
        self.success_count = success_count;
    }

    pub fn set_total(&mut self, total: i32) {
        self.total = total
    }

    pub fn print_now_status(self) {
        debug!("\r检查进度: {}/{}", self.check_index, self.total);
        io::stdout().flush().unwrap();
    }
}

pub struct SearchParams {
    pub thumbnail: bool,
    pub concurrent: i32,
    pub timeout: u16,
    pub output_file: String,
    pub search_options: SearchOptions,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchOptions {
    pub keyword_full_match: Vec<String>,
    pub keyword_like: Vec<String>,
    pub keyword_dislike: Vec<String>,
    pub ipv4: bool,
    pub ipv6: bool,
    pub exclude_url: Vec<String>,
    pub exclude_host: Vec<String>,
    pub quality: Vec<QualityType>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CheckOptions {
    pub request_time: i32,
    pub concurrent: i32,
    pub sort: bool,
    pub no_check: bool,
    pub ffmpeg_check: bool,
    pub same_save_num: i32,
    pub not_http_skip: bool,
}

impl M3uObjectList {
    pub fn new() -> M3uObjectList {
        M3uObjectList {
            header: None,
            list: vec![],
            counter: M3uObjectListCounter::new(),
        }
    }

    pub fn set_header(&mut self, header: M3uExt) {
        self.header = Some(header)
    }

    pub fn set_list(&mut self, list: Vec<M3uObject>) {
        self.list = list
    }

    pub fn get_list(self) -> Vec<M3uObject> {
        self.list
    }

    pub fn get_header(self) -> Option<M3uExt> {
        self.header
    }

    pub fn print_result(&mut self) -> String {
        let succ_num = self.counter.success_count;
        let failed_num = self.counter.total - succ_num;
        format!("有效源: {}, 无效源: {}", succ_num, failed_num)
    }

    pub fn set_counter(&mut self, counter: M3uObjectListCounter) {
        self.counter = counter
    }

    pub fn t2s(&mut self) {
        for item in &mut self.list {
            item.t2s();
        }
    }

    pub fn remove_useless_char(&mut self) {
        for item in &mut self.list {
            item.remove_useless_char();
        }
    }

    fn search_keywords(
        &mut self,
        full_name_search: Vec<String>,
        keyword_like: Vec<String>,
        keyword_dislike: Vec<String>,
    ) {
        if keyword_like.len() == 0 && keyword_dislike.len() == 0 && full_name_search.len() == 0 {
            return;
        }
        let mut save_list = vec![];
        for i in self.list.clone() {
            let mut save = true;
            if keyword_like.len() > 0 {
                save = false;
                for lk in keyword_like.to_owned() {
                    if i.search_name.contains(&lk.to_lowercase()) {
                        save = true
                    }
                }
            }
            if keyword_dislike.len() > 0 {
                for dk in keyword_dislike.to_owned() {
                    if i.search_name.contains(&dk.to_lowercase()) {
                        save = false
                    }
                }
            }
            if full_name_search.len() > 0 {
                for fk in full_name_search.to_owned() {
                    if !i.search_name.eq(&fk.to_lowercase()) {
                        save = false
                    }
                }
            }
            if save {
                save_list.push(i);
            }
        }
        self.set_list(save_list)
    }

    pub fn search_video_quality(&mut self, quality_list: Vec<QualityType>) {
        if quality_list.is_empty() {
            return;
        }
        println!("-----quality_list len {}", quality_list.len());

        let mut filtered_list = vec![];
        let mut quality_map: HashMap<QualityType, i32> = HashMap::new();
        for item in quality_list {
            quality_map.insert(item, 1);
        }

        for item in &self.list {
            let mut is_save = false;
            if item.status != Success {
                continue;
            }

            if let Some(info) = &item.other_status.ffmpeg_info {
                if info.video.len() > 0 {
                    for v_q in info.video.clone() {
                        for (k, _) in quality_map.clone() {
                            if k == v_q.quality_type {
                                is_save = true
                            }
                        }
                    }
                    if is_save {
                        filtered_list.push(item.clone());
                    }
                }
            }
        }

        self.set_list(filtered_list);
    }

    pub async fn search(&mut self, search: SearchOptions) {
        let mut list = vec![];
        debug!(
            "query params --- full -- {:?} like --{:?} dislike{:?} search data count {:?}",
            search.keyword_full_match,
            search.keyword_like,
            search.keyword_dislike,
            self.list.len()
        );
        self.search_keywords(
            search.keyword_full_match,
            search.keyword_like,
            search.keyword_dislike,
        );
        debug!("query params --- searched data count {:?}", self.list.len());
        for v in self.list.clone() {
            let mut is_save = true;
            for ex_url in search.exclude_url.clone() {
                if v.url
                    .clone()
                    .to_lowercase()
                    .eq(&ex_url.clone().to_lowercase())
                {
                    is_save = false
                }
            }
            for ex_host in search.exclude_host.clone() {
                // 解析 URL
                let url = url::Url::parse(&*v.clone().url);
                match url {
                    Ok(url) => {
                        // 获取主机部分
                        if let Some(host) = url.host_str() {
                            if ex_host.clone().eq(&host.to_string()) {
                                is_save = false;
                            }
                        }
                    }
                    _ => {}
                }
            }
            if is_save {
                list.push(v.clone());
            }
        }
        let mut seen = HashSet::new();
        let unique_list: Vec<M3uObject> = list
            .into_iter()
            .filter(|p| seen.insert(p.url.clone()))
            .collect();
        self.set_list(unique_list)
    }

    pub async fn generate_thumbnail(&mut self, concurrent: i32, timeout_millisecond: u16) {
        let mut ffmpeg_timeout_sec = 1;
        if timeout_millisecond / 1000 >= 1 {
            ffmpeg_timeout_sec = (timeout_millisecond / 1000) as u64;
        }
        // Create a semaphore to limit concurrency
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrent as usize));
        let mut results = Vec::new();

        // Create a stream of futures
        let mut tasks = Vec::new();
        for v in self.list.clone() {
            let img_url = format!(
                "{}/{}.jpeg",
                generate_channel_thumbnail_folder_name(),
                md5_str(v.get_url())
            );
            let url = v.get_url().clone();
            let semaphore = Arc::clone(&semaphore);
            let task = tokio::spawn(async move {
                // Acquire permit from semaphore
                let _permit = semaphore.acquire().await.unwrap();
                let succ = capture_stream_pic(url, img_url.clone(), ffmpeg_timeout_sec);
                (v, succ, img_url)
            });
            tasks.push(task);
        }

        // Process results as they complete
        let mut completed = 0;
        while completed < tasks.len() {
            match futures::future::select_all(tasks).await {
                (Ok((mut v, succ, img_url)), _, remaining) => {
                    if succ {
                        let mut extend;
                        match v.get_extend() {
                            None => {
                                extend = M3uExtend::new();
                            }
                            Some(data) => extend = data.clone(),
                        }
                        extend.set_thumbnail(img_url);
                        v.set_status(Success);
                        v.set_extend(extend);
                        // Only add to results if thumbnail generation was successful
                        results.push(v);
                    } else {
                        v.set_status(Failed);
                    }
                    tasks = remaining;
                    completed += 1;
                }
                (Err(e), _, remaining) => {
                    error!("Error processing thumbnail: {}", e);
                    tasks = remaining;
                    completed += 1;
                }
            }
        }

        self.list = results
    }

    pub async fn check_data_new(&mut self, opt: CheckOptions) {
        if !opt.no_check {
            let total = self.list.len();
            info!("文件中源总数： {}", total);
            let mut counter = M3uObjectListCounter::new();
            counter.set_total(total as i32);
            self.set_counter(counter);

            let data = self.list.clone();
            let (tx, rx) = mpsc::channel();
            let (data_tx, data_rx) = mpsc::channel();
            let new_data_rx = Arc::new(Mutex::new(data_rx));

            for _i in 0..opt.concurrent {
                let tx_clone = tx.clone();
                let data_rx_clone = Arc::clone(&new_data_rx);

                thread::spawn(move || loop {
                    match data_rx_clone.lock() {
                        Ok(data) => {
                            let mut item = {
                                let rx_lock = data;
                                rx_lock.recv().unwrap_or_else(|_| M3uObject::new())
                            };
                            if item.url == "" {
                                break;
                            }
                            item.check_by_block(
                                opt.request_time,
                                opt.ffmpeg_check,
                                opt.not_http_skip,
                            );
                            tx_clone.send(item.get_obj()).unwrap()
                        }
                        Err(e) => {
                            error!("check_data_new error ---{} ", e);
                            break;
                        }
                    }
                });
            }
            for item in data {
                data_tx.send(item).unwrap();
            }
            drop(tx); // 发送完成后关闭队列

            let mut res_list = vec![];

            counter.print_now_status();
            let mut i = 0;
            loop {
                if i == counter.total {
                    break;
                }
                let result = rx.recv();
                match result {
                    Ok(data) => {
                        // 处理返回值
                        res_list.push(data);
                        counter.now_index_incr();
                        counter.print_now_status();
                        i += 1;
                    }
                    Err(_e) => {}
                }
            }
            self.set_list(res_list.clone());
        } else {
            info!("no check----{}", opt.no_check);
            let total = self.list.len();
            for item in &mut self.list {
                item.set_status(Success);
            }
            info!("文件中源总数： {}", total);
        }
        // 统计success list
        let mut succ_count = 0;
        for i in self.list.clone() {
            if i.status == Success {
                succ_count += 1;
            }
        }
        self.counter.set_success_count(succ_count);
        if opt.same_save_num > 0 {
            self.do_same_save(opt.same_save_num);
        }
        if opt.sort {
            self.do_name_sort();
        }
    }

    pub fn get_list_len(&self) -> usize {
        self.list.len()
    }

    // pub fn get_succ_list_len(&self) -> usize {
    //     let mut x = 0;
    //     for i in self.list.clone() {
    //         if i.status == Success {
    //             x += 1;
    //         }
    //     }
    //     x
    // }

    pub async fn output_file(&mut self, output_file: String, only_success: bool) {
        // 生成.m3u 文件
        self.generate_m3u_file(output_file.clone(), only_success);
        // 生成.txt 文件
        let txt_file = output_file.clone().replace(".m3u", ".txt");
        self.generate_text_file(txt_file.clone(), only_success);
        time::sleep(Duration::from_millis(500)).await;
    }

    pub fn do_same_save(&mut self, same_save_num: i32) {
        let mut hash_list: HashMap<String, Vec<M3uObject>> = HashMap::new();
        for item in self.list.clone() {
            if item.status == Success {
                let mut list = vec![];
                let list_op = hash_list.get(&item.search_name.clone());
                if list_op.is_some() {
                    list = list_op.unwrap().to_vec();
                }
                list.push(item.clone());
                hash_list.insert(item.search_name.clone(), list);
            }
        }
        let mut save_list = vec![];
        for (_, items) in hash_list {
            let mut i = 0;
            for item in items {
                if i >= same_save_num {
                    continue;
                }
                save_list.push(item.clone());
                i += 1;
            }
        }
        self.set_list(save_list)
    }

    pub fn do_name_sort(&mut self) {
        // 自定义排序
        self.list.sort_by(|a, b| {
            // 提取前缀和数字部分
            let (a_prefix, a_num) = m3u::extract_prefix_and_number(&a.search_name);
            let (b_prefix, b_num) = m3u::extract_prefix_and_number(&b.search_name);

            // 先比较前缀
            let prefix_cmp = a_prefix.cmp(&b_prefix);
            if prefix_cmp != std::cmp::Ordering::Equal {
                return prefix_cmp;
            }

            // 如果前缀相同，再比较数字
            a_num.cmp(&b_num)
        });
    }

    pub fn get_m3u_content(&mut self) -> String {
        let mut result_m3u_content = vec![];
        match &self.header {
            None => result_m3u_content.push(String::from("#EXTM3U")),
            Some(data) => {
                if data.x_tv_url.len() > 0 {
                    let exp = data.x_tv_url.join(",");
                    let header_line = format!("#EXTM3U x-tvg-url=\"{}\"", exp);
                    result_m3u_content.push(header_line.to_owned());
                } else {
                    result_m3u_content.push(String::from("#EXTM3U"))
                }
            }
        }
        for mut x in self.list.clone() {
            x.generate_raw();
            result_m3u_content.push(x.raw.clone());
        }
        result_m3u_content.join("\n")
    }

    pub fn generate_m3u_file(&mut self, output_file: String, only_succ: bool) {
        if self.list.len() > 0 {
            let mut result_m3u_content: Vec<String> = vec![];
            match &self.header {
                None => result_m3u_content.push(String::from("#EXTM3U")),
                Some(data) => {
                    if data.x_tv_url.len() > 0 {
                        let exp = data.x_tv_url.join(",");
                        let header_line = format!("#EXTM3U x-tvg-url=\"{}\"", exp);
                        result_m3u_content.push(header_line.to_owned());
                    } else {
                        result_m3u_content.push(String::from("#EXTM3U"))
                    }
                }
            }
            for mut x in self.list.clone() {
                x.generate_raw();
                if only_succ {
                    if x.status == Success {
                        result_m3u_content.push(x.raw.clone());
                    }
                } else {
                    result_m3u_content.push(x.raw.clone());
                }
            }
            let mut fd = File::create(output_file.to_owned()).unwrap();
            for x in result_m3u_content {
                let _ = fd.write(format!("{}\n", x).as_bytes());
            }
            let _ = fd.flush();
        }
    }

    pub fn generate_text_file(&mut self, output_file: String, only_succ: bool) {
        if self.list.len() > 0 {
            // 打开文件 b 并准备写入
            let mut file_b = File::create(output_file).unwrap();

            // 逐行读取文件 a 的内容
            for line in &self.list {
                if only_succ {
                    if line.status == Success {
                        let txt = format!("{},{}", line.name, line.url);
                        // 将每一行写入文件 b
                        writeln!(file_b, "{}", txt).unwrap();
                    }
                } else {
                    let txt = format!("{},{}", line.name, line.url);
                    // 将每一行写入文件 b
                    writeln!(file_b, "{}", txt).unwrap();
                }
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct M3uExt {
    pub x_tv_url: Vec<String>,
}

impl From<String> for M3uObjectList {
    fn from(_str: String) -> Self {
        let empty_data = M3uObjectList {
            header: None,
            list: vec![],
            counter: M3uObjectListCounter::new(),
        };
        let source_type = m3u::check_source_type(_str.to_owned());
        return match source_type {
            Some(Normal) => m3u::body_normal(_str.clone(), false),
            Some(Quota) => m3u::body_quota(_str.clone(), false),
            None => empty_data,
        };
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum CheckDataStatus {
    Unchecked, //未检查
    Success,   //检查成功
    Failed,    //检查失败，包含超时、无效
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OtherStatus {
    delay: i32,
    ffmpeg_info: Option<FfmpegInfo>,
}

impl OtherStatus {
    pub fn new() -> OtherStatus {
        OtherStatus {
            ffmpeg_info: None,
            delay: 0,
        }
    }

    pub fn set_delay(&mut self, delay: i32) {
        self.delay = delay
    }

    pub fn set_ffmpeg_info(&mut self, ffmpeg_info: Option<FfmpegInfo>) {
        self.ffmpeg_info = ffmpeg_info
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkInfo {
    delay: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, Hash, PartialEq)]
pub enum QualityType {
    QualityUnknown,
    Quality240P,
    Quality360P,
    Quality480P,
    Quality720P,
    Quality1080P,
    Quality2K,
    Quality4K,
    Quality8K,
}

// fn video_type_string(vt: VideoType) -> *const str {
//     return match vt {
//         VideoType::Unknown => "未知",
//         VideoType::Sd => "普清",
//         VideoType::Hd => "高清720P",
//         VideoType::Fhd => "全高清1080P",
//         VideoType::Uhd => "超高清4K",
//         VideoType::Fuhd => "全超高清8K",
//     };
// }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VideoInfo {
    pub width: i32,
    pub height: i32,
    pub codec: String,
    pub quality_type: QualityType,
}

impl VideoInfo {
    pub fn new() -> VideoInfo {
        VideoInfo {
            width: 0,
            height: 0,
            codec: "".to_string(),
            quality_type: QualityUnknown,
        }
    }

    pub fn set_width(&mut self, width: i32) {
        self.width = width
    }

    pub fn set_height(&mut self, height: i32) {
        self.height = height
    }

    pub fn set_codec(&mut self, codec: String) {
        self.codec = codec
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioInfo {
    codec: String,
    channels: i32,
}

impl AudioInfo {
    pub fn new() -> AudioInfo {
        AudioInfo {
            codec: "".to_string(),
            channels: 0,
        }
    }
    pub fn set_codec(&mut self, codec: String) {
        self.codec = codec
    }

    pub fn set_channels(&mut self, channels: i32) {
        self.channels = channels
    }
    // pub fn get_channels(self) -> i32 {
    //     self.channels
    // }
    // pub fn get_codec(self) -> String {
    //     self.codec
    // }
}

pub enum SourceType {
    Normal,
    //m3u标准文件
    Quota, //名称,url格式
}

#[allow(dead_code)]
pub mod m3u {
    use crate::common::util::{get_url_body, is_url, parse_normal_str, parse_quota_str};
    use crate::common::SourceType::{Normal, Quota};
    use crate::common::{M3uObjectList, SourceType};
    use core::option::Option;
    use log::{error, info};
    use std::fs::File;
    use std::io::Read;

    pub fn check_source_type(_body: String) -> Option<SourceType> {
        if _body.starts_with("#EXTM3U") {
            return Some(Normal);
        }
        let exp = _body.lines();
        let mut quota = false;
        for x in exp {
            if !quota {
                let exp: Vec<&str> = x.split(',').collect();
                if exp.len() >= 2 {
                    quota = true
                }
            }
        }
        if quota {
            return Some(Quota);
        }
        None
    }

    pub fn body_normal(_body: String, not_show_input_type: bool) -> M3uObjectList {
        if !not_show_input_type {
            info!("您输入是：标准格式m3u格式文件");
        }
        parse_normal_str(_body)
    }

    pub fn body_quota(_body: String, not_show_input_type: bool) -> M3uObjectList {
        if !not_show_input_type {
            info!("您输入是：非标准格式m3u格式文件，尝试解析中");
        }
        parse_quota_str(_body)
    }

    // pub fn from_body(_str: &String) -> M3uObjectList {
    //     let source_type = check_source_type(_str.to_owned());
    //     return match source_type {
    //         Some(Normal) => body_normal(_str.clone()),
    //         Some(Quota) => body_quota(_str.clone()),
    //         None => M3uObjectList::new(),
    //     };
    // }

    pub fn list_str2obj(str_arr: Vec<String>, now_show_input_top: bool) -> M3uObjectList {
        let mut obj = M3uObjectList::new();
        let mut header = vec![];
        let mut list = vec![];
        for _str in str_arr {
            let source_type = check_source_type(_str.to_owned());
            match source_type {
                Some(Normal) => {
                    let nor_data = body_normal(_str.clone(), now_show_input_top);
                    list.extend(nor_data.clone().get_list());
                    match nor_data.get_header() {
                        Some(d) => {
                            header.push(d);
                        }
                        None => {}
                    }
                }
                Some(Quota) => {
                    let quo_data = body_quota(_str.clone(), now_show_input_top);
                    list.extend(quo_data.clone().get_list());
                    match quo_data.get_header() {
                        Some(d) => {
                            header.push(d);
                        }
                        None => {}
                    }
                }
                None => {}
            };
        }
        obj.set_list(list);
        obj
    }

    // 提取前缀和数字的函数
    pub fn extract_prefix_and_number(s: &str) -> (&str, Option<usize>) {
        // 找到数字的起始位置
        let numeric_start = s.find(|c: char| c.is_digit(10));

        match numeric_start {
            Some(index) => {
                let prefix = &s[..index];
                let number = s[index..].parse::<usize>().ok(); // 尝试将数字部分转为 usize
                (prefix, number)
            }
            None => (s, None), // 如果没有找到数字，返回整个字符串作为前缀，数字部分为 None
        }
    }

    // pub async fn from_url(_url: String, timeout: u64) -> M3uObjectList {
    //     let url_body = get_url_body(_url, timeout)
    //         .await
    //         .expect("can not open this url");
    //     return from_body(&url_body);
    // }

    // pub fn from_file(_file: String) -> M3uObjectList {
    //     let mut data = File::open(_file).expect("file not exists");
    //     let mut contents = String::from("");
    //     data.read_to_string(&mut contents).unwrap();
    //     return from_body(&contents);
    // }

    pub async fn from_arr(_url: Vec<String>, _timeout: u64) -> Vec<String> {
        let mut body_arr = vec![];
        for x in _url {
            if is_url(x.clone()) {
                match get_url_body(x.clone(), _timeout).await {
                    Ok(data) => body_arr.push(data),
                    Err(e) => {
                        error!("url can not be open : {}, error: {}", x.clone(), e)
                    }
                }
            } else {
                let mut contents = String::from("");
                let mut file_name = x.clone();
                if x.starts_with("static") {
                    file_name = format!("./{}", x)
                }
                match File::open(file_name) {
                    Ok(mut data) => {
                        data.read_to_string(&mut contents).unwrap();
                        body_arr.push(contents);
                    }
                    Err(e) => {
                        error!("file {} not exists, e {}", x, e)
                    }
                }
            }
        }
        body_arr
    }
}

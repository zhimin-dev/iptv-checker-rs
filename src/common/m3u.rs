use crate::common::check::check::check_link_is_valid;
use crate::common::m3u::m3u::do_name_sort;
use crate::common::CheckDataStatus::{Failed, Success, Unchecked};
use crate::common::SourceType::{Normal, Quota};
use crate::common::VideoType::Unknown;
use actix_rt::time;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Error, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct M3uExtend {
    group_title: String,
    //group title
    tv_logo: String,
    //台标
    tv_language: String,
    //语言
    tv_country: String,
    //国家
    tv_id: String,
    //电视id
    user_agent: String, // user-agent
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
        }
    }

    pub fn set_group_title(&mut self, group_title: String) {
        self.group_title = group_title
    }

    pub fn set_tv_logo(&mut self, tv_logo: String) {
        self.tv_logo = tv_logo
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
    other_status: Option<OtherStatus>, //其它状态
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
            other_status: None,
        };
    }

    pub fn set_index(&mut self, index: i32) {
        self.index = index;
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

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn set_search_name(&mut self, search_name: String) {
        self.search_name = search_name.to_lowercase()
    }

    pub fn set_raw(&mut self, raw: String) {
        self.raw = raw
    }

    pub fn generate_raw(&mut self) {
        let mut tv_id = "".to_string();
        let mut tv_logo = "".to_string();
        let mut group_title = "".to_string();
        if self.extend.is_some() {
            tv_id = format!(" tvg-id=\"{}\"", self.extend.clone().unwrap().tv_id.clone());
            tv_logo = format!(
                " tvg-logo=\"{}\"",
                self.extend.clone().unwrap().tv_id.clone()
            );
            group_title = format!(
                " group-title=\"{}\"",
                self.extend.clone().unwrap().tv_id.clone()
            );
        }
        self.raw = format!(
            "#EXTINF:-1 {}{}{},{}\n{}",
            tv_id, tv_logo, group_title, self.name, self.url
        );
    }

    pub fn set_extend(&mut self, extend: M3uExtend) {
        self.extend = Some(extend)
    }

    pub fn set_other_status(&mut self, other_status: OtherStatus) {
        self.other_status = Some(other_status)
    }

    pub fn set_status(&mut self, status: CheckDataStatus) {
        self.status = status;
    }
}

#[derive(Copy, Clone)]
pub struct M3uObjectListCounter {
    check_index: i32,
    //当前检查的索引
    total: i32,
    // 总数
    success_count: i32, // 成功数据
}

#[derive(Clone)]
pub struct M3uObjectList {
    header: Option<M3uExt>,
    list: Vec<M3uObject>,
    result_list: Vec<M3uObject>,
    debug: bool,
    search_clarity: Option<VideoType>,
    counter: Option<M3uObjectListCounter>,
}

impl M3uObjectListCounter {
    pub fn new() -> M3uObjectListCounter {
        M3uObjectListCounter {
            check_index: 0,
            total: 0,
            success_count: 0,
        }
    }

    pub fn now_index_incr(&mut self) {
        let mut index = self.check_index;
        index += 1;
        self.check_index = index
    }

    // pub fn now_index_incr_and_print(&mut self) {
    //     let mut index = self.check_index;
    //     index += 1;
    //     self.check_index = index;
    //     self.print_now_status();
    // }

    pub fn incr_succ(&mut self) {
        self.success_count += 1
    }

    pub fn set_total(&mut self, total: i32) {
        self.total = total
    }

    pub fn print_now_status(self) {
        print!("\r检查进度: {}/{}", self.check_index, self.total);
        io::stdout().flush().unwrap();
    }

    // pub fn get_now_status(self) -> (i32, i32) {
    //     (self.check_index, self.total)
    // }
}

impl M3uObjectList {
    pub fn new() -> M3uObjectList {
        M3uObjectList {
            header: None,
            list: vec![],
            result_list: vec![],
            debug: false,
            search_clarity: None,
            counter: None,
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
        let succ_num = self.counter.unwrap().success_count;
        let failed_num = self.counter.unwrap().total - succ_num;
        format!("有效源: {}, 无效源: {}", succ_num, failed_num)
    }

    pub fn set_counter(&mut self, counter: M3uObjectListCounter) {
        self.counter = Some(counter)
    }

    pub fn set_debug_mod(&mut self, debug: bool) {
        self.debug = debug
    }

    pub async fn search(
        &self,
        search_name: String,
        full_match: bool,
        ipv4: bool,
        ipv6: bool,
        exclude_url: Vec<String>,
        exclude_host: Vec<String>,
    ) -> Result<Vec<M3uObject>, Error> {
        let mut list = vec![];
        let s_name = search_name.clone();
        let exp_list: Vec<&str> = s_name.split(",").collect();
        println!(
            "query params ----{:?} search data count --- {}",
            exp_list,
            self.list.len()
        );
        for v in self.list.clone() {
            let mut is_save = false;
            for e in exp_list.clone() {
                if full_match {
                    if v.search_name.eq(e.to_string().as_str()) {
                        is_save = true;
                    }
                } else {
                    if v.search_name.contains(e.to_string().as_str()) {
                        is_save = true;
                    }
                }
            }
            // let mut now_ip_type = 0;
            // match v.url.clone().parse::<IpAddr>() {
            //     Ok(IpAddr::V4(_)) => {
            //         now_ip_type = 1; // ipv4
            //     }
            //     Ok(IpAddr::V6(_)) => {
            //         now_ip_type = 2; // ipv6
            //     }
            //     _ => {}
            // }
            // if now_ip_type == 0 {
            //     continue;
            // }
            // if now_ip_type == 1 && ipv4 {
            //     is_save = true
            // } else if now_ip_type == 2 && ipv6 {
            //     is_save = true
            // }
            // for ex_url in exclude_url.clone() {
            //     if v.url.clone().to_lowercase().eq(&ex_url.clone().to_lowercase()) {
            //         is_save = false
            //     }
            // }
            // for ex_host in exclude_host.clone() {
            //     // 解析 URL
            //     let url = url::Url::parse(&*v.clone().url);
            //     match url {
            //         Ok(url) => {
            //             // 获取主机部分
            //             if let Some(host) = url.host_str() {
            //                 if ex_host.clone().eq(&host.to_string()) {
            //                     is_save = false;
            //                 }
            //             }
            //         }
            //         _ => {}
            //     }
            // }
            if is_save {
                list.push(v.clone());
            }
        }

        Ok(list)
    }

    pub async fn check_data_new(
        &mut self,
        request_time: i32,
        _concurrent: i32,
        sort: bool,
        no_check: bool,
    ) {
        let mut search_clarity = false;
        match &self.search_clarity {
            Some(_d) => search_clarity = true,
            None => {}
        }
        if !no_check {
            let total = self.list.len();
            println!("文件中源总数： {}", total);
            let mut counter = M3uObjectListCounter::new();
            counter.set_total(total as i32);
            self.set_counter(counter);
            let debug = self.debug;

            let data = self.list.clone();
            let (tx, rx) = mpsc::channel();
            let (data_tx, data_rx) = mpsc::channel();
            let new_data_rx = Arc::new(Mutex::new(data_rx));

            for _i in 0.._concurrent {
                let tx_clone = tx.clone();
                let data_rx_clone = Arc::clone(&new_data_rx);

                thread::spawn(move || loop {
                    match data_rx_clone.lock() {
                        Ok(data) => {
                            let item = {
                                let rx_lock = data;
                                rx_lock.recv().unwrap_or_else(|_| M3uObject::new())
                            };
                            if item.url == "" {
                                break;
                            }
                            let result = set_one_item(debug, item, request_time, search_clarity);
                            tx_clone.send(result).unwrap()
                        }
                        Err(e) => {
                            println!("error ---{} ", e);
                            break;
                        }
                    }
                });
            }
            for item in data {
                data_tx.send(item).unwrap();
            }
            drop(tx); // 发送完成后关闭队列

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
                        self.result_list.push(data);
                        counter.now_index_incr();
                        counter.print_now_status();
                        i += 1;
                    }
                    Err(_e) => {}
                }
            }
        } else {
            let total = self.list.len();
            println!("文件中源总数： {}", total);
            self.result_list = self.list.clone()
        }
        if sort {
            self.result_list = do_name_sort(self.result_list.clone());
        }
    }

    pub async fn output_file(&mut self, output_file: String) {
        let mut lines: Vec<String> = vec![];
        let mut counter = M3uObjectListCounter::new();
        match self.counter {
            Some(data) => counter = data,
            None => {}
        }
        for x in &self.result_list {
            if x.status == Success {
                counter.incr_succ();
                let exp: Vec<&str> = x.raw.lines().collect();
                for o in exp {
                    lines.push(o.to_owned());
                }
            }
        }
        self.set_counter(counter);
        // 生成.m3u 文件
        self.generate_m3u_file_from_giving_list(output_file.clone(), lines);
        // 生成.txt 文件
        self.generate_text_file(output_file.clone());
        time::sleep(Duration::from_millis(500)).await;
    }

    pub fn generate_m3u_file(&mut self, output_file: String) {
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
            for x in self.list.clone() {
                result_m3u_content.push(x.raw.clone());
            }
            let mut fd = File::create(output_file.to_owned()).unwrap();
            for x in result_m3u_content {
                let _ = fd.write(format!("{}\n", x).as_bytes());
            }
            let _ = fd.flush();
        }
    }

    pub fn generate_m3u_file_from_giving_list(&mut self, output_file: String, lines: Vec<String>) {
        if lines.len() > 0 {
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
            for x in lines {
                let temp = x.clone();
                result_m3u_content.push(temp.to_owned());
            }
            let mut fd = File::create(output_file.to_owned()).unwrap();
            for x in result_m3u_content {
                let _ = fd.write(format!("{}\n", x).as_bytes());
            }
            let _ = fd.flush();
        }
    }

    pub fn generate_text_file(&mut self, output_file: String) {
        let txt_sub = output_file.replace(".m3u", ".txt");

        // 打开文件 b 并准备写入
        let mut file_b = File::create(txt_sub).unwrap();

        // 逐行读取文件 a 的内容
        for line in &self.result_list {
            if line.status == Success {
                let txt = format!("{},{}", line.name, line.url);
                // 将每一行写入文件 b
                writeln!(file_b, "{}", txt).unwrap();
            }
        }
    }
}

fn set_one_item(
    debug: bool,
    mut x: M3uObject,
    request_time: i32,
    search_clarity: bool,
) -> M3uObject {
    let url = x.url.clone();
    let _log_url = url.clone();
    let result = actix_rt::System::new().block_on(check_link_is_valid(
        url,
        request_time as u64,
        search_clarity,
    ));
    if debug {
        println!("url is: {} result: {:?}", x.url.clone(), result);
    }
    return match result {
        Ok(data) => {
            let mut status = OtherStatus::new();
            match data.audio {
                Some(a) => status.set_audio(a),
                None => {}
            }
            match data.video {
                Some(v) => status.set_video(v),
                None => {}
            }
            x.set_status(Success);
            x.set_other_status(status);
            x
        }
        Err(_e) => {
            x.set_status(Failed);
            x
        }
    };
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct M3uExt {
    pub(crate) x_tv_url: Vec<String>,
}

impl From<String> for M3uObjectList {
    fn from(_str: String) -> Self {
        let empty_data = M3uObjectList {
            header: None,
            list: vec![],
            result_list: vec![],
            debug: false,
            search_clarity: None,
            counter: None,
        };
        let source_type = m3u::check_source_type(_str.to_owned());
        return match source_type {
            Some(Normal) => m3u::body_normal(_str.clone()),
            Some(Quota) => m3u::body_quota(_str.clone()),
            None => empty_data,
        };
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum CheckDataStatus {
    Unchecked,
    //未检查
    Success,
    //检查成功
    Failed, //检查失败，包含超时、无效
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OtherStatus {
    video: Option<VideoInfo>,
    //视频信息
    audio: Option<AudioInfo>,
    //音频信息
    network: Option<NetworkInfo>, //网路状态信息
}

impl OtherStatus {
    pub fn new() -> OtherStatus {
        OtherStatus {
            video: None,
            audio: None,
            network: None,
        }
    }

    pub fn set_video(&mut self, video: VideoInfo) {
        self.video = Some(video)
    }

    pub fn set_audio(&mut self, audio: AudioInfo) {
        self.audio = Some(audio)
    }

    // pub fn set_network(&mut self, network: NetworkInfo) {
    //     self.network = Some(network)
    // }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkInfo {
    delay: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum VideoType {
    Unknown,
    Sd,
    Hd,
    Fhd,
    Uhd,
    Fuhd,
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
    width: i32,
    height: i32,
    codec: String,
    video_type: VideoType,
}

impl VideoInfo {
    pub fn new() -> VideoInfo {
        VideoInfo {
            width: 0,
            height: 0,
            codec: "".to_string(),
            video_type: Unknown,
        }
    }

    pub fn set_width(&mut self, width: i32) {
        self.width = width
    }

    pub fn set_height(&mut self, height: i32) {
        self.height = height
    }

    // pub fn set_video_type(&mut self, video_type: VideoType) {
    //     self.video_type = video_type
    // }

    pub fn set_codec(&mut self, codec: String) {
        self.codec = codec
    }

    // pub fn get_width(self) -> i32 {
    //     self.width
    // }
    //
    // pub fn get_height(self) -> i32 {
    //     self.height
    // }
    //
    // pub fn get_video_type(self) -> VideoType {
    //     self.video_type
    // }
    //
    // pub fn get_codec(self) -> String {
    //     self.codec
    // }
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
    use crate::common::{M3uObject, M3uObjectList, SourceType};
    use core::option::Option;
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
        return None;
    }

    pub(crate) fn body_normal(_body: String) -> M3uObjectList {
        println!("您输入是：标准格式m3u格式文件");
        parse_normal_str(_body)
    }

    pub(crate) fn body_quota(_body: String) -> M3uObjectList {
        println!("您输入是：非标准格式m3u格式文件，尝试解析中");
        parse_quota_str(_body)
    }

    pub fn from_body(_str: &String) -> M3uObjectList {
        let source_type = check_source_type(_str.to_owned());
        return match source_type {
            Some(Normal) => body_normal(_str.clone()),
            Some(Quota) => body_quota(_str.clone()),
            None => M3uObjectList::new(),
        };
    }

    pub fn filter_by_keyword(
        list: Vec<M3uObject>,
        keyword_like: Vec<String>,
        keyword_dislike: Vec<String>,
    ) -> Vec<M3uObject> {
        if keyword_like.len() == 0 && keyword_dislike.len() == 0 {
            return list;
        }
        let mut save_list = vec![];
        for i in list {
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
            if save {
                save_list.push(i);
            }
        }
        save_list
    }

    pub fn from_body_arr(
        str_arr: Vec<String>,
        keyword_like: Vec<String>,
        keyword_dislike: Vec<String>,
    ) -> M3uObjectList {
        let mut obj = M3uObjectList::new();
        let mut header = vec![];
        let mut list = vec![];
        for _str in str_arr {
            let source_type = check_source_type(_str.to_owned());
            match source_type {
                Some(Normal) => {
                    let nor_data = body_normal(_str.clone());
                    list.extend(nor_data.clone().get_list());
                    match nor_data.get_header() {
                        Some(d) => {
                            header.push(d);
                        }
                        None => {}
                    }
                }
                Some(Quota) => {
                    let quo_data = body_quota(_str.clone());
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
        let save_keyword = filter_by_keyword(list, keyword_like, keyword_dislike);
        obj.set_list(save_keyword);
        return obj;
    }

    pub fn do_name_sort(list: Vec<M3uObject>) -> Vec<M3uObject> {
        let mut new_list = list.clone();

        // new_list.sort_by(|a_value, b_value| {
        //     a_value.search_name.cmp(&b_value.search_name)
        // });
        // 自定义排序
        new_list.sort_by(|a, b| {
            // 提取前缀和数字部分
            let (a_prefix, a_num) = extract_prefix_and_number(&a.search_name);
            let (b_prefix, b_num) = extract_prefix_and_number(&b.search_name);

            // 先比较前缀
            let prefix_cmp = a_prefix.cmp(&b_prefix);
            if prefix_cmp != std::cmp::Ordering::Equal {
                return prefix_cmp;
            }

            // 如果前缀相同，再比较数字
            a_num.cmp(&b_num)
        });
        return new_list;
    }

    // 提取前缀和数字的函数
    fn extract_prefix_and_number(s: &str) -> (&str, Option<usize>) {
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

    pub async fn from_url(_url: String, timeout: u64) -> M3uObjectList {
        let url_body = get_url_body(_url, timeout)
            .await
            .expect("can not open this url");
        return from_body(&url_body);
    }

    pub fn from_file(_file: String) -> M3uObjectList {
        let mut data = File::open(_file).expect("file not exists");
        let mut contents = String::from("");
        data.read_to_string(&mut contents).unwrap();
        return from_body(&contents);
    }

    pub async fn from_arr(
        _url: Vec<String>,
        _timeout: u64,
        keyword_like: Vec<String>,
        keyword_dislike: Vec<String>,
    ) -> M3uObjectList {
        let mut body_arr = vec![];
        for x in _url {
            if is_url(x.clone()) {
                match get_url_body(x.clone(), _timeout).await {
                    Ok(data) => body_arr.push(data),
                    Err(e) => {
                        println!("url can not be open : {}, error: {}", x.clone(), e)
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
                        println!("file {} not exists, e {}", x, e)
                    }
                }
            }
        }
        from_body_arr(body_arr, keyword_like, keyword_dislike)
    }
}

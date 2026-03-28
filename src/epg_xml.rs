//! EPG XML (XMLTV 格式) 解析：XML 字符串 -> Rust 对象 -> JSON
//!
//! 对应 all.xml 结构：
//! - 根节点 `<tv>` 属性 generator-info-name, generator-info-url
//! - 子节点 `<channel id="...">`，内嵌 `<display-name lang="...">文本</display-name>`
//! - 子节点 `<programme start="..." stop="..." channel="...">`，内嵌 `<title lang="...">文本</title>`

use crate::search::parse_epg_time_str;
use crate::common::translate::trad_to_simp;
use crate::epg_mapping::get_best_tvg_id;
use log::error;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Reader;
use quick_xml::Writer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::format;
use std::fs::File;
use std::io::{Cursor, Error, ErrorKind, Write};
use std::sync::{Arc, RwLock};
use lazy_static::lazy_static;

// ============== 全局 EPG 缓存 ==============
lazy_static! {
    pub static ref GLOBAL_EPG_CACHE: Arc<RwLock<HashMap<String, Vec<Programme>>>> = Arc::new(RwLock::new(HashMap::new()));
}

/// 安全地更新全局 EPG 缓存
pub fn update_global_epg_cache(tv: &Tv) {
    let mut new_cache: HashMap<String, Vec<Programme>> = HashMap::new();
    
    // 建立 channel id 到 channel name 的映射
    let mut channel_id_to_name: HashMap<String, String> = HashMap::new();
    for ch in &tv.channels {
        if let Some(dn) = ch.display_names.first() {
            channel_id_to_name.insert(ch.id.clone(), dn.value.clone());
        }
    }

    // 根据 channel id 将 programme 分组，并使用 channel name 作为 key
    for pr in &tv.programmes {
        if let Some(channel_name) = channel_id_to_name.get(&pr.channel) {
            new_cache
                .entry(channel_name.clone())
                .or_insert_with(Vec::new)
                .push(pr.clone());
        }
    }

    if let Ok(mut cache) = GLOBAL_EPG_CACHE.write() {
        *cache = new_cache;
    }
}

/// 根据频道名称查询 EPG 缓存
pub fn query_epg_by_channel(channel_name: &str) -> Vec<Programme> {
    if let Ok(cache) = GLOBAL_EPG_CACHE.read() {
        if let Some(programmes) = cache.get(channel_name) {
            return programmes.clone();
        }
    }
    Vec::new()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpgChannelItem {
    pub name: String,
    pub channel: String,
}

/// 获取所有可用的 EPG 频道列表
pub fn get_all_epg_channels() -> Vec<EpgChannelItem> {
    let mut result = Vec::new();
    if let Ok(cache) = GLOBAL_EPG_CACHE.read() {
        for (name, programmes) in cache.iter() {
            if let Some(first_prog) = programmes.first() {
                result.push(EpgChannelItem {
                    name: name.clone(),
                    channel: first_prog.channel.clone(),
                });
            }
        }
    }
    result
}

/// 根据频道名称列表生成自定义 EPG XML 字符串
pub fn generate_custom_epg_xml(channel_names: Vec<String>) -> Result<String, String> {
    let mut tv = Tv {
        generator_info_name: Some("iptv-checker-rs".to_string()),
        generator_info_url: Some("https://github.com/iptv-checker-rs".to_string()),
        channels: Vec::new(),
        programmes: Vec::new(),
    };

    let mut added_channels = std::collections::HashSet::new();

    if let Ok(cache) = GLOBAL_EPG_CACHE.read() {
        for name in channel_names {
            if added_channels.contains(&name) {
                continue;
            }
            if let Some(programmes) = cache.get(&name) {
                if let Some(first_prog) = programmes.first() {
                    let channel_id = first_prog.channel.clone();
                    tv.channels.push(Channel {
                        id: channel_id,
                        display_names: vec![DisplayName {
                            lang: Some("zh".to_string()),
                            value: name.clone(),
                        }],
                    });
                    tv.programmes.extend(programmes.clone());
                    added_channels.insert(name);
                }
            }
        }
    }

    tv_to_epg_xml(&tv)
}

// ============== JSON 可序列化结构（与 XML 语义一致） ==============

/// 根节点 tv
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tv {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generator_info_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generator_info_url: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub channels: Vec<Channel>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub programmes: Vec<Programme>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EpgAllListItem {
    channel_map: HashMap<String, String>,
    list_map: HashMap<String, Vec<Programme>>,
}

impl EpgAllListItem {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn set_channel_map(&mut self, channel_map: HashMap<String, String>) {
        self.channel_map = channel_map;
    }
    
    pub fn set_list_map(&mut self, list_map: HashMap<String, Vec<Programme>>) {
        self.list_map = list_map;
    }
    
    pub fn save_json_file(self, file_name:String) {
        serde_json::to_writer(File::create(file_name).unwrap(), &self).unwrap();
    }
}

/// 频道
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    #[serde(rename = "displayNames")]
    pub display_names: Vec<DisplayName>,
}

impl Channel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    pub fn set_display_names(&mut self, display_names: Vec<DisplayName>) {
        self.display_names = display_names;
    }
}

/// 显示名称（多语言）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DisplayName {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    pub value: String,
}

impl DisplayName {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_lang(&mut self, lang: String) {
        self.lang = Some(lang);
    }
    pub fn set_value(&mut self, value: String) {
        self.value = value;
    }
}

/// 节目单条
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Programme {
    pub start: String,
    pub stop: String,
    pub start_unix: i64,
    pub stop_unix: i64,
    pub channel: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub titles: Vec<ProgrammeTitle>,
}

impl Programme {
    pub fn to_unixtime(&mut self) {
        self.start_unix = parse_epg_time_str(&self.start);
        self.stop_unix = parse_epg_time_str(&self.stop);
    }
}

/// 节目标题（多语言）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProgrammeTitle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    pub value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EpgProgram {
    pub start: String,
    pub stop: String,
    pub channel: String,
    pub title: String,
    pub lang: String,
}

impl EpgProgram {
    pub fn new() -> EpgProgram {
        EpgProgram {
            start: String::default(),
            stop: String::default(),
            channel: String::default(),
            title: String::default(),
            lang: String::default(),
        }
    }
    pub fn set_start(&mut self, start: String) {
        self.start = start;
    }
    pub fn set_stop(&mut self, stop: String) {
        self.stop = stop;
    }
    pub fn set_channel(&mut self, channel: String) {
        self.channel = channel;
    }
    pub fn set_titles(&mut self, title: String) {
        self.title = title
    }
    pub fn set_lang(&mut self, lang: String) {
        self.lang = lang
    }
}

// ============== 解析实现 ==============

/// 将 XML 字符串解析为 EPG 根对象 `Tv`
pub fn parse_epg_xml_str(xml_str: &str) -> Result<Tv, String> {
    let mut reader = Reader::from_reader(Cursor::new(xml_str.as_bytes()));
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut tv = Tv::default();
    let mut stack: Vec<String> = Vec::new();
    let mut current_channel: Option<Channel> = None;
    let mut current_programme: Option<Programme> = None;
    let mut attrs_map: HashMap<String, String> = HashMap::new();
    let mut channel_id_mapping: HashMap<String, String> = HashMap::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                attrs_map.clear();
                for attr in e.attributes().flatten() {
                    let k = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
                    let v = String::from_utf8_lossy(attr.value.as_ref()).into_owned();
                    attrs_map.insert(k, v);
                }

                match name.as_str() {
                    "tv" => {
                        tv.generator_info_name = attrs_map.remove("generator-info-name");
                        tv.generator_info_url = attrs_map.remove("generator-info-url");
                        stack.push(name);
                    }
                    "channel" => {
                        let id = attrs_map.remove("id").unwrap_or_default();
                        current_channel = Some(Channel {
                            id,
                            display_names: Vec::new(),
                        });
                        stack.push(name);
                    }
                    "display-name" => {
                        let lang = attrs_map.remove("lang");
                        stack.push(name.clone());
                        if let Some(ref mut ch) = current_channel {
                            ch.display_names.push(DisplayName {
                                lang,
                                value: String::new(),
                            });
                        }
                    }
                    "programme" => {
                        let mut channel_id = attrs_map.remove("channel").unwrap_or_default();
                        // Map the original channel ID to the standardized one
                        if let Some(mapped_id) = channel_id_mapping.get(&channel_id) {
                            channel_id = mapped_id.clone();
                        }
                        
                        current_programme = Some(Programme {
                            start: attrs_map.remove("start").unwrap_or_default(),
                            stop: attrs_map.remove("stop").unwrap_or_default(),
                            channel: channel_id,
                            start_unix: 0,
                            stop_unix: 0,
                            titles: Vec::new(),
                        });
                        stack.push(name);
                    }
                    "title" => {
                        let lang = attrs_map.remove("lang");
                        stack.push(name.clone());
                        if let Some(ref mut pr) = current_programme {
                            pr.titles.push(ProgrammeTitle {
                                lang,
                                value: String::new(),
                            });
                        }
                    }
                    _ => {
                        stack.push(name);
                    }
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();
                if text.is_empty() {
                    continue;
                }
                if let Some(top) = stack.last().map(String::as_str) {
                    match top {
                        "display-name" => {
                            if let Some(ref mut ch) = current_channel {
                                if let Some(last) = ch.display_names.last_mut() {
                                    // Convert to simplified Chinese
                                    last.value = trad_to_simp(&text);
                                }
                            }
                        }
                        "title" => {
                            if let Some(ref mut pr) = current_programme {
                                if let Some(last) = pr.titles.last_mut() {
                                    last.value = text;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if stack.pop().as_deref() != Some(name.as_str()) {
                    // 不严格校验栈
                }
                match name.as_str() {
                    "channel" => {
                        if let Some(mut ch) = current_channel.take() {
                            // After parsing all display names, map the channel ID
                            if let Some(dn) = ch.display_names.first() {
                                let original_id = ch.id.clone();
                                let standardized_id = get_best_tvg_id(None, &dn.value);
                                
                                // Save mapping for programmes
                                channel_id_mapping.insert(original_id, standardized_id.clone());
                                
                                // Update channel ID
                                ch.id = standardized_id;
                            }
                            tv.channels.push(ch);
                        }
                    }
                    "programme" => {
                        if let Some(pr) = current_programme.take() {
                            tv.programmes.push(pr);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML 解析错误: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(tv)
}

// ============== 序列化实现（Tv -> XML 字符串） ==============

/// 对 XML 文本内容进行转义（用于元素文本）
fn escape_xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

impl Tv {
    pub fn new() -> Self {
        Tv::default()
    }

    pub fn set_generator_info_url(&mut self, url: String) {
        self.generator_info_url = Some(url);
    }
    pub fn set_generator_info_name(&mut self, generator_info_name: String) {
        self.generator_info_name = Some(generator_info_name);
    }
    pub fn set_channels(&mut self, channels: Vec<Channel>) {
        self.channels = channels;
    }
    pub fn set_programmes(&mut self, programmes: Vec<Programme>) {
        self.programmes = programmes;
    }

    pub fn to_epg_xml_file(self, file_name: String) -> Result<(), Error> {
        let res = self.to_epg_xml_str();
        println!("------{:?}", res);
        match res {
            Ok(data) => {
                let res_file = File::create(file_name);
                match res_file {
                    Ok(mut file) => {
                        let _ = file.write_all(data.as_bytes());
                    }
                    Err(e) => {
                        return Err(Error::new(ErrorKind::Other, e.to_string()));
                    }
                }
            }
            Err(e) => {
                return Err(Error::new(ErrorKind::Other, e.to_string()));
            }
        }
        Ok(())
    }

    pub fn to_epg_xml_str(self) -> Result<String, String> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));

        // <?xml version="1.0" encoding="UTF-8"?>
        writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
            .map_err(|e| format!("写入 XML 声明失败: {}", e))?;

        // <tv generator-info-name="..." generator-info-url="...">
        let mut tv_start = BytesStart::new("tv");
        if let Some(ref name) = self.generator_info_name {
            tv_start.push_attribute(("generator-info-name", name.as_str()));
        }
        if let Some(ref url) = self.generator_info_url {
            tv_start.push_attribute(("generator-info-url", url.as_str()));
        }
        writer
            .write_event(Event::Start(tv_start))
            .map_err(|e| format!("写入 tv 开始标签失败: {}", e))?;

        // <channel id="..."> ... </channel>
        for ch in &self.channels {
            let mut ch_start = BytesStart::new("channel");
            ch_start.push_attribute(("id", ch.id.as_str()));
            writer
                .write_event(Event::Start(ch_start))
                .map_err(|e| format!("写入 channel 开始标签失败: {}", e))?;
            for dn in &ch.display_names {
                let mut dn_start = BytesStart::new("display-name");
                if let Some(ref lang) = dn.lang {
                    dn_start.push_attribute(("lang", lang.as_str()));
                }
                writer
                    .write_event(Event::Start(dn_start))
                    .map_err(|e| format!("写入 display-name 开始标签失败: {}", e))?;
                writer
                    .write_event(Event::Text(BytesText::from_escaped(
                        escape_xml_text(&dn.value).as_str(),
                    )))
                    .map_err(|e| format!("写入 display-name 文本失败: {}", e))?;
                writer
                    .write_event(Event::End(BytesEnd::new("display-name")))
                    .map_err(|e| format!("写入 display-name 结束标签失败: {}", e))?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("channel")))
                .map_err(|e| format!("写入 channel 结束标签失败: {}", e))?;
        }

        // <programme start="..." stop="..." channel="..."> ... </programme>
        for pr in &self.programmes {
            let mut pr_start = BytesStart::new("programme");
            pr_start.push_attribute(("start", pr.start.as_str()));
            pr_start.push_attribute(("stop", pr.stop.as_str()));
            pr_start.push_attribute(("channel", pr.channel.as_str()));
            writer
                .write_event(Event::Start(pr_start))
                .map_err(|e| format!("写入 programme 开始标签失败: {}", e))?;
            for t in &pr.titles {
                let mut title_start = BytesStart::new("title");
                if let Some(ref lang) = t.lang {
                    title_start.push_attribute(("lang", lang.as_str()));
                }
                writer
                    .write_event(Event::Start(title_start))
                    .map_err(|e| format!("写入 title 开始标签失败: {}", e))?;
                writer
                    .write_event(Event::Text(BytesText::from_escaped(
                        escape_xml_text(&t.value).as_str(),
                    )))
                    .map_err(|e| format!("写入 title 文本失败: {}", e))?;
                writer
                    .write_event(Event::End(BytesEnd::new("title")))
                    .map_err(|e| format!("写入 title 结束标签失败: {}", e))?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("programme")))
                .map_err(|e| format!("写入 programme 结束标签失败: {}", e))?;
        }

        // </tv>
        writer
            .write_event(Event::End(BytesEnd::new("tv")))
            .map_err(|e| format!("写入 tv 结束标签失败: {}", e))?;

        let bytes = writer.into_inner().into_inner();
        String::from_utf8(bytes).map_err(|e| format!("UTF-8 转换失败: {}", e))
    }
}

/// 将 `Tv` 对象序列化为 XMLTV 格式的 XML 字符串
pub fn tv_to_epg_xml(tv: &Tv) -> Result<String, String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // <?xml version="1.0" encoding="UTF-8"?>
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(|e| format!("写入 XML 声明失败: {}", e))?;

    // <tv generator-info-name="..." generator-info-url="...">
    let mut tv_start = BytesStart::new("tv");
    if let Some(ref name) = tv.generator_info_name {
        tv_start.push_attribute(("generator-info-name", name.as_str()));
    }
    if let Some(ref url) = tv.generator_info_url {
        tv_start.push_attribute(("generator-info-url", url.as_str()));
    }
    writer
        .write_event(Event::Start(tv_start))
        .map_err(|e| format!("写入 tv 开始标签失败: {}", e))?;

    // <channel id="..."> ... </channel>
    for ch in &tv.channels {
        let mut ch_start = BytesStart::new("channel");
        ch_start.push_attribute(("id", ch.id.as_str()));
        writer
            .write_event(Event::Start(ch_start))
            .map_err(|e| format!("写入 channel 开始标签失败: {}", e))?;
        for dn in &ch.display_names {
            let mut dn_start = BytesStart::new("display-name");
            if let Some(ref lang) = dn.lang {
                dn_start.push_attribute(("lang", lang.as_str()));
            }
            writer
                .write_event(Event::Start(dn_start))
                .map_err(|e| format!("写入 display-name 开始标签失败: {}", e))?;
            writer
                .write_event(Event::Text(BytesText::from_escaped(
                    escape_xml_text(&dn.value).as_str(),
                )))
                .map_err(|e| format!("写入 display-name 文本失败: {}", e))?;
            writer
                .write_event(Event::End(BytesEnd::new("display-name")))
                .map_err(|e| format!("写入 display-name 结束标签失败: {}", e))?;
        }
        writer
            .write_event(Event::End(BytesEnd::new("channel")))
            .map_err(|e| format!("写入 channel 结束标签失败: {}", e))?;
    }

    // <programme start="..." stop="..." channel="..."> ... </programme>
    for pr in &tv.programmes {
        let mut pr_start = BytesStart::new("programme");
        pr_start.push_attribute(("start", pr.start.as_str()));
        pr_start.push_attribute(("stop", pr.stop.as_str()));
        pr_start.push_attribute(("channel", pr.channel.as_str()));
        writer
            .write_event(Event::Start(pr_start))
            .map_err(|e| format!("写入 programme 开始标签失败: {}", e))?;
        for t in &pr.titles {
            let mut title_start = BytesStart::new("title");
            if let Some(ref lang) = t.lang {
                title_start.push_attribute(("lang", lang.as_str()));
            }
            writer
                .write_event(Event::Start(title_start))
                .map_err(|e| format!("写入 title 开始标签失败: {}", e))?;
            writer
                .write_event(Event::Text(BytesText::from_escaped(
                    escape_xml_text(&t.value).as_str(),
                )))
                .map_err(|e| format!("写入 title 文本失败: {}", e))?;
            writer
                .write_event(Event::End(BytesEnd::new("title")))
                .map_err(|e| format!("写入 title 结束标签失败: {}", e))?;
        }
        writer
            .write_event(Event::End(BytesEnd::new("programme")))
            .map_err(|e| format!("写入 programme 结束标签失败: {}", e))?;
    }

    // </tv>
    writer
        .write_event(Event::End(BytesEnd::new("tv")))
        .map_err(|e| format!("写入 tv 结束标签失败: {}", e))?;

    let bytes = writer.into_inner().into_inner();
    String::from_utf8(bytes).map_err(|e| format!("UTF-8 转换失败: {}", e))
}

/// 将 Tv 对象序列化为 JSON 字符串
pub fn epg_to_json_string(tv: &Tv) -> Result<String, String> {
    serde_json::to_string_pretty(tv).map_err(|e| format!("JSON 序列化错误: {}", e))
}

/// 一步：XML 字符串 -> Tv 对象 -> JSON 字符串
pub fn epg_xml_str_to_json(xml_str: &str) -> Result<String, String> {
    let tv = parse_epg_xml_str(xml_str)?;
    epg_to_json_string(&tv)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<tv generator-info-name="https://vip.erw.cc" generator-info-url="kuke21@vip.qq.com">
<channel id="1">
<display-name lang="zh">CCTV1</display-name>
</channel>
<programme start="20260205000000 +0800" stop="20260205010100 +0800" channel="1">
<title lang="zh">非遗里的中国Ⅳ(6)</title>
</programme>
<programme start="20260205010100 +0800" stop="20260205014500 +0800" channel="1">
<title lang="zh">生活圈</title>
</programme>
</tv>"#;

    #[test]
    fn parse_epg_xml_to_struct() {
        let tv = parse_epg_xml_str(SAMPLE).unwrap();
        assert_eq!(
            tv.generator_info_name.as_deref(),
            Some("https://vip.erw.cc")
        );
        assert_eq!(tv.channels.len(), 1);
        assert_eq!(tv.channels[0].id, "1");
        assert_eq!(tv.channels[0].display_names[0].value, "CCTV1");
        assert_eq!(tv.programmes.len(), 2);
        assert_eq!(tv.programmes[0].titles[0].value, "非遗里的中国Ⅳ(6)");
    }

    #[test]
    fn epg_to_json() {
        let tv = parse_epg_xml_str(SAMPLE).unwrap();
        let json = epg_to_json_string(&tv).unwrap();
        assert!(json.contains("1") && json.contains("CCTV1"));
        assert!(json.contains("非遗里的中国Ⅳ(6)"));
    }

    #[test]
    fn epg_xml_str_to_json_one_shot() {
        let json = epg_xml_str_to_json(SAMPLE).unwrap();
        let _: serde_json::Value = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn tv_to_epg_xml_roundtrip() {
        let tv = parse_epg_xml_str(SAMPLE).unwrap();
        let xml = tv_to_epg_xml(&tv).unwrap();
        assert!(xml.contains("<?xml"));
        assert!(xml.contains("<tv"));
        assert!(xml.contains("</tv>"));
        assert!(xml.contains(r#"<channel id="1">"#));
        assert!(xml.contains("CCTV1"));
        assert!(xml.contains("非遗里的中国Ⅳ(6)"));
        // 再解析一次应得到等价数据
        let tv2 = parse_epg_xml_str(&xml).unwrap();
        assert_eq!(tv.channels.len(), tv2.channels.len());
        assert_eq!(tv.programmes.len(), tv2.programmes.len());
        assert_eq!(tv.channels[0].id, tv2.channels[0].id);
        assert_eq!(
            tv.channels[0].display_names[0].value,
            tv2.channels[0].display_names[0].value
        );
        assert_eq!(
            tv.programmes[0].titles[0].value,
            tv2.programmes[0].titles[0].value
        );
    }
}

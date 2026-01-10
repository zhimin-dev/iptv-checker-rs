pub static TASK_JSON: &str = "static/core/task.json";
pub static LOGOS_JSON: &str = "static/core/logos.json";
pub static REPLACE_JSON: &str = "static/core/replace.json";
pub static SEARCH_JSON: &str = "static/core/search.json";
pub static FAVOURITE_JSON: &str = "static/core/favourite.json";

pub static TRANSLATE_FILE: &str = "./src/assets/translate.txt";

pub static STATIC_FOLDER: &str = "./static/";
pub static INPUT_FOLDER: &str = "./static/input/";
pub static INPUT_SEARCH_FOLDER: &str = "./static/search/";
pub static INPUT_LIVE_FOLDER: &str = "./static/live/";
pub static OUTPUT_FOLDER: &str = "./static/output/";
pub static OUTPUT_THUMBNAIL_FOLDER: &str = "./static/thumbnail/";
pub static LOGS_FOLDER: &str = "./static/logs/";
pub static LOGOS_FOLDER: &str = "/static/core/logos/";

pub static FAVOURITE_CONFIG_JSON_CONTENT: &str = r#"{
  "like": [],
  "equal": []
}"#;

pub static REPLACE_TXT_CONTENT: &str = r#"{
    "replace_string": false,
    "replace_map": {
        "[geo-blocked]": "",
        "[ipv6]": "",
        "hevc": "",
        "50 fps": "",
        "[not 24/7]": "",
        " (600p)": "",
        " ": ""
    }
}"#;

pub static TASK_DATA: &str = r#"{
    "now": null,
    "task": {}
}"#;

pub static SEARCH_CONFIG_JSON_CONTENT: &str = r#"{
  "source": [
    {
      "urls": [
        "https://github.com/YueChan/Live",
        "https://github.com/YanG-1989/m3u",
        "https://github.com/fanmingming/live",
        "https://github.com/qwerttvv/Beijing-IPTV",
        "https://github.com/joevess/IPTV",
        "https://github.com/cymz6/AutoIPTV-Hotel",
        "https://github.com/skddyj/iptv",
        "https://github.com/suxuang/myIPTV"
      ],
      "include_files": [],
      "parse_type": "github-home-page"
    },
    {
      "urls": [
        "https://live.zbds.top/tv/iptv6.m3u",
        "https://live.zbds.top/tv/iptv4.m3u"
      ],
      "include_files": [],
      "parse_type": "raw-source"
    },
    {
      "urls": [
        "https://github.com/iptv-org/iptv/tree/master/streams"
      ],
      "include_files": [
        "cn.m3u",
        "tw.m3u",
        "hk.m3u"
      ],
      "parse_type": "github-sub-page"
    }
  ],
  "extensions": [
    ".txt",
    ".m3u"
  ]
}"#;

pub static LOGOS_CONFIG_JSON_CONTENT: &str = r#"{
  "host": "",
  "remote_url2local_images": false,
  "logos": []
}"#;

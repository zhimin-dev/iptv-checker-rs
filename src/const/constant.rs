pub static CORE_JSON: &str = "core/core.json";

pub static STATIC_FOLDER: &str = "./static/";

pub static TRANSLATE_FILE: &str = "./src/assets/translate.txt";

pub static INPUT_FOLDER: &str = "./static/input/";

pub static INPUT_SEARCH_FOLDER: &str = "./static/input/search/";
pub static INPUT_LIVE_FOLDER: &str = "./static/input/live/";

pub static OUTPUT_FOLDER: &str = "./static/output/";

pub static REPLACE_JSON: &str = "core/replace.json";

pub static OUTPUT_THUMBNAIL_FOLDER: &str = "./static/output/thumbnail/";

pub static LOGS_FOLDER: &str = "./static/logs/";

pub static LOGOS_FOLDER: &str = "./static/input/logos/";
pub static LOGOS_JSON_FILE: &str = "core/logos.json";

pub static GLOBAL_CONFIG_FILE_NAME: &str = "core/global_config.json";

pub static FAVOURITE_FILE_NAME: &str = "core/favourite.json";

pub static FAVOURITE_CONFIG_JSON_CONTENT: &str = r#"{
  "like": [],
  "equal": []
}"#;

pub static GLOBAL_CONFIG_CONTENT:&str= r#"{
    "remote_url2local_images": false,
    "search": {
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
                    "https://live.zbds.top/tv/iptv4.m3u",
                    "https://raw.githubusercontent.com/jackell777/jackell777.github.io/fa8f1249b67cff645628b6e08fa6f802d430afbb/list.txt",
                    "https://raw.githubusercontent.com/sake0116/0305/983fcb9a7ea4cea08a4c177495d34d9ce76db757/2185"
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
        ],
        "search_list": []
    }
}"#;

pub static REPLACE_TXT_CONTENT:&str = r#"{
    "[geo-blocked]": "",
    "[ipv6]": "",
    "hevc": "",
    "50 fps": "",
    "[not 24/7]": "",
    " (600p)": "",
    " ": ""
}"#;

pub static CORE_DATA:&str = r#"{
  "check": {
    "now": null,
    "task": {
    }
  },
  "others": {
    "replace_dic":"core/replace.json",
    "translate_dic":"./translate.txt"
  },
  "ob": {
    "list": []
  },
  "search": {
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
          "https://live.zbds.top/tv/iptv4.m3u",
          "https://raw.githubusercontent.com/jackell777/jackell777.github.io/fa8f1249b67cff645628b6e08fa6f802d430afbb/list.txt",
          "https://raw.githubusercontent.com/sake0116/0305/983fcb9a7ea4cea08a4c177495d34d9ce76db757/2185"
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
    ],
    "search_list": []
  }
}"#;
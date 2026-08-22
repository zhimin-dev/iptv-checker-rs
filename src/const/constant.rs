pub static TASK_JSON: &str = "static/core/task.json";
pub static LOGOS_JSON: &str = "static/core/logos.json";
pub static REPLACE_JSON: &str = "static/core/replace.json";
pub static SEARCH_JSON: &str = "static/core/search.json";
pub static FAVOURITE_JSON: &str = "static/core/favourite.json";
pub static BASE_JSON: &str = "static/core/base.json";
pub static EPG_JSON: &str = "static/core/epg.json";
pub static NETWORK_JSON: &str = "static/core/network.json";
pub static GROUP_MAPPING_JSON: &str = "static/core/group_mapping.json";
pub static TRANSLATE_FILE: &str = "./src/assets/translate.txt";

pub static STATIC_FOLDER: &str = "./static/";
pub static UPLOAD_FOLDER: &str = "./static/core/upload/";
pub static INPUT_FOLDER: &str = "./static/input/";
pub static INPUT_SEARCH_FOLDER: &str = "./static/search/";
pub static INPUT_EPG_FOLDER: &str = "./static/epg/";
pub static INPUT_LIVE_FOLDER: &str = "./static/live/";
pub static PLAYER_FOLDER: &str = "./static/player/";
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
        " (600p)": ""
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
      "extensions": [
        ".txt",
        ".m3u"
      ],
      "parse_type": "github-home-page"
    },
    {
      "urls": [
        "https://live.zbds.top/tv/iptv6.m3u",
        "https://live.zbds.top/tv/iptv4.m3u"
      ],
      "include_files": [],
      "extensions": [],
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
      "extensions": [],
      "parse_type": "github-sub-page"
    }
  ]
}"#;

pub static LOGOS_CONFIG_JSON_CONTENT: &str = r#"{
  "logos": []
}"#;

pub static BASE_CONFIG_JSON_CONTENT: &str = r#"{
  "host": "",
  "replace_string": false,
  "remote_url2local_images": false,
  "github_token": "",
  "player_cache_ttl_hours": 24
}"#;

pub static EPG_CONFIG_JSON_CONTENT: &str = r#"{
  "source": {
    "list": [
      "http://epg.mb6.top/heiptv.xml"
    ]
  }
}"#;

pub static NETWORK_CONFIG_JSON_CONTENT: &str = r#"{
  "proxy_url": "",
  "use_system_proxy": true,
  "custom_headers": {},
  "user_agent": ""
}"#;

pub static GROUP_MAPPING_CONFIG_JSON_CONTENT: &str = r#"{
  "groups": [
    "卫视",
    "央视"
  ],
  "mapping": {
    "CCTV5": "央视",
    "深圳卫视": "卫视",
    "CCTV1": "央视",
    "天津卫视": "卫视",
    "旅游卫视": "卫视",
    "湖北卫视": "卫视",
    "CCTV11": "央视",
    "CCTV3": "央视",
    "CCTV6": "央视",
    "甘肃卫视": "卫视",
    "浙江卫视": "卫视",
    "CCTV4": "央视",
    "湖南卫视": "卫视",
    "宁夏卫视": "卫视",
    "广西卫视": "卫视",
    "陕西卫视": "卫视",
    "东方卫视": "卫视",
    "云南卫视": "卫视",
    "山东卫视": "卫视",
    "吉林卫视": "卫视",
    "江苏卫视": "卫视",
    "黑龙江卫视": "卫视",
    "安徽卫视": "卫视",
    "CCTV16": "央视",
    "CCTV13": "央视",
    "CCTV15": "央视",
    "黄河卫视": "卫视",
    "CCTV14": "央视",
    "CCTV5+": "央视",
    "叁沙卫视": "卫视",
    "山西卫视": "卫视",
    "CCTV12": "央视",
    "CCTV8": "央视",
    "CCTV17": "央视",
    "河南卫视": "卫视",
    "康巴卫视": "卫视",
    "CCTV4K": "央视",
    "西藏卫视": "卫视",
    "CCTV10": "央视",
    "四川卫视": "卫视",
    "重庆卫视": "卫视",
    "海峡卫视": "卫视",
    "CCTV7": "央视",
    "CCTV4EUO": "央视",
    "广东卫视": "卫视",
    "辽宁卫视": "卫视",
    "江西卫视": "卫视",
    "CCTV4AME": "央视",
    "厦门卫视": "卫视",
    "CCTV2": "央视",
    "兵团卫视": "卫视",
    "青海卫视": "卫视",
    "河北卫视": "卫视",
    "贵州卫视": "卫视",
    "CCTV9": "央视",
    "延边卫视": "卫视",
    "东南卫视": "卫视",
    "北京卫视": "卫视"
  }
}"#;

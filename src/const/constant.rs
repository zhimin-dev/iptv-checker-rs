pub static CORE_JSON: &str = "core.json";

pub static STATIC_FOLDER: &str = "./static/";

pub static INPUT_FOLDER: &str = "./static/input/";

pub static INPUT_SEARCH_FOLDER: &str = "./static/input/search/";
pub static INPUT_LIVE_FOLDER: &str = "./static/input/live/";

pub static OUTPUT_FOLDER: &str = "./static/output/";

pub static OUTPUT_THUMBNAIL_FOLDER: &str = "./static/output/thumbnail/";

pub static LOGS_FOLDER: &str = "./static/logs/";

pub static CORE_DATA:&str = r#"{
  "check": {
    "now": null,
    "task": {
    }
  },
  "others": {
    "replace_empty":["[geo-blocked]", "[ipv6]", "hevc", "50 fps", "[not 24/7]", " (600p) "],
    "replace_chars": [
        {"name":"鳳","replace":"凤"},
        {"name":"娛","replace":"娱"},
        {"name":"樂","replace":"乐"},
        {"name":"時","replace":"时"},
        {"name":"動","replace":"动"},
        {"name":"無","replace":"无"},
        {"name":"亞","replace":"亚"},
        {"name":"電","replace":"电"},
        {"name":"東","replace":"东"},
        {"name":"態","replace":"动"},
        {"name":"衛","replace":"卫"},
        {"name":"軍","replace":"军"},
        {"name":"視","replace":"视"},
        {"name":"臺","replace":"台"},
        {"name":"國","replace":"国"},
        {"name":"親","replace":"亲"},
        {"name":"麗","replace":"丽"},
        {"name":"質","replace":"质"},
        {"name":"際","replace":"际"},
        {"name":"記","replace":"记"},
        {"name":"偵","replace":"侦"},
        {"name":"緝","replace":"缉"},
        {"name":"學","replace":"学"},
        {"name":"錄","replace":"录"},
        {"name":"頻","replace":"频"},
        {"name":"兒","replace":"儿"},
        {"name":"歐","replace":"欧"},
        {"name":"覺","replace":"觉"},
        {"name":"歡","replace":"欢"},
        {"name":"畫","replace":"画"},
        {"name":"聯","replace":"联"},
        {"name":"購","replace":"购"},
        {"name":"網","replace":"网"},
        {"name":"風","replace":"风"},
        {"name":"無","replace":"无"},
        {"name":"雲","replace":"云"},
        {"name":"爾","replace":"尔"},
        {"name":"達","replace":"达"},
        {"name":"體","replace":"体"},
        {"name":"線","replace":"线"},
        {"name":"戲","replace":"戏"},
        {"name":"劇","replace":"剧"},
        {"name":"聞","replace":"闻"},
        {"name":"華","replace":"华"},
        {"name":"綫","replace":"线"},
        {"name":"龍","replace":"龙"},
        {"name":"場","replace":"场"},
        {"name":"禧","replace":"禧"},
        {"name":"麗","replace":"丽"},
        {"name":"業","replace":"业"},
        {"name":"壹","replace":"壹"},
        {"name":"韓","replace":"韩"},
        {"name":"經","replace":"经"},
        {"name":"遊","replace":"游"},
        {"name":"創","replace":"创"},
        {"name":"聯","replace":"联"},
        {"name":"蓮","replace":"莲"},
        {"name":"馬","replace":"马"},
        {"name":"財","replace":"财"},
        {"name":"優","replace":"优"},
        {"name":"語","replace":"语"},
        {"name":"灣","replace":"湾"},
        {"name":"紀","replace":"纪"},
        {"name":"實","replace":"实"},
        {"name":"納","replace":"纳"},
        {"name":"運","replace":"运"},
        {"name":"萬","replace":"万"},
        {"name":"數","replace":"数"},
        {"name":"鏡","replace":"镜"},
        {"name":"愛","replace":"爱"},
        {"name":"緯","replace":"纬"},
        {"name":"綜","replace":"综"},
        {"name":"藝","replace":"艺"},
        {"name":"資","replace":"资"},
        {"name":"訊","replace":"资"}
    ]
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
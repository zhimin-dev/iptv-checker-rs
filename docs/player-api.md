# 播放器 API 与「流畅播放中继」接口文档

本文档描述 iptv-checker-rs 为播放客户端（iptv-checker-desktop）提供的接口。
所有接口均支持 CORS（服务端已启用 permissive CORS）。

## 1. 频道列表

### GET /api/player/channels?source=all|like|checked

返回服务端的频道列表（去重、繁转简、按分组/名称排序）。

请求参数：

| 参数 | 必填 | 说明 |
|------|------|------|
| source | 否 | 频道源：all=爬取的全部频道；like=喜欢（收藏关键词过滤）；checked=定时检查过的频道（仅检查成功的，播放器客户端默认使用）；兼容旧参数 filter |
| refresh | 否 | refresh=1 时强制重新解析生成（绕过缓存） |

响应示例：

    {
      "list": [
        {
          "id": "a1b2c3...",          // md5(url)，频道稳定标识
          "name": "CCTV-1 综合",
          "group": "央视",
          "logo": "http://.../logo.png",
          "url": "http://.../playlist.m3u8",   // 频道源（直连播放用）
          "epg_id": "cctv1",        // EPG 频道 id（用于 /epg?channel=）
          "user_agent": ""           // 源站要求的 UA（中继模式可用）
        }
      ],
      "total": 1234,
      "generated_at": 1753320000
    }

说明：

- 若服务端今日搜索数据不存在，返回 500 且 msg 提示先执行搜索任务。
- 直连播放时浏览器无法自定义 User-Agent；开启中继模式后，客户端会把 user_agent 作为请求头交给服务端 ffmpeg。

## 1.1 搜索历史与推荐

服务端记录用户搜索过的频道名称，用于「添加想看频道」时推荐。

- `POST /api/player/search-history` 请求体 `{"name": "CCTV"}` —— 记录一次搜索（累计次数）。
- `GET /api/player/search-history?keyword=&limit=20` —— 按搜索次数排序返回推荐，支持关键字过滤。

数据持久化在 static/core/search_history.json（最多保留 300 条）。播放器客户端搜索频道、点击频道时会自动上报；网页版「收藏」设置页会展示「经常搜索」推荐。

## 1.1 播放历史（后台播放列表）

服务端记录用户播放过的频道（名称 + 链接 + 可播放标识）。

- `POST /api/player/play-history` 请求体 `{"name": "CCTV-1", "url": "http://...", "playable": true}` —— 记录一次播放：同一链接更新名称/可播放标识/时间，累计播放次数。
- `GET /api/player/play-history?playable=1|0&keyword=&limit=` —— 查询播放列表：playable=1 仅可播放、playable=0 仅不可播放、缺省全部；keyword 过滤名称；按最近播放排序。
- `DELETE /api/player/play-history/{id}` —— 删除单条。
- `DELETE /api/player/play-history` —— 清空。

数据持久化在 static/core/play_history.json（上限 1000 条）。播放器客户端在频道开始播放时上报 playable=true，播放失败时上报 playable=false。

## 1.2 频道收藏（具体频道）

- `GET /api/player/favourites` —— 收藏列表（主页「已收藏」区块数据源）。
- `POST /api/player/favourites` 请求体 `{"name": "CCTV-1", "url": "http://..."}` —— 收藏（按 url 去重，幂等）。
- `DELETE /api/player/favourites/{id}` —— 取消收藏。
- `GET /api/player/favourites/check?url=` —— 检查某个链接是否已收藏，返回 `{"favourited": true, "id": "..."}`。

数据持久化在 static/core/player_favourites.json。播放器客户端播放页右上角 ♥ 按钮、播放历史列表的收藏按钮都调用这些接口。

## 1.3 频道列表缓存

服务端对解析结果做内存缓存（默认 24 小时，可配置），命中缓存时响应时间从十几秒降至毫秒级。

- 响应中附带 `cached`（是否命中缓存）、`cached_at`（缓存生成时间）、`ttl_hours`（有效期）。
- `GET /api/player/channels?refresh=1` —— 强制重新解析生成（绕过缓存）。
- `DELETE /api/player/channels/cache` —— 手动清除缓存。
- `GET /api/player/cache-config` —— 查询缓存配置（ttl_hours）。
- `POST /api/player/cache-config` 请求体 `{"ttl_hours": 24}` —— 修改有效期（0 表示不缓存，每次实时解析），修改后立即清空现有缓存。

客户端（iptv-checker-desktop）还会把频道列表存入本地 IndexedDB：进入首页先用本地缓存秒开，再后台拉取最新数据；设置页可修改 TTL 与手动清除服务端/本地缓存。

## 2. 流畅播放中继（服务器缓冲）

当客户端直连源站播放卡顿时，可让服务端“接管”源链接：服务端用 ffmpeg 持续拉取源流，
切片缓存到本地磁盘（static/player/{sid}/），客户端改播服务端本地 HLS。
画面相比直播源会有 分片时长 x 保留分片数 秒左右的延迟（默认 4s x 12 ≈ 48s）。

### POST /api/player/relay/start

启动一个中继会话。

请求体：

    {
      "url": "http://example.com/live/playlist.m3u8",  // 必填，http/https/rtmp/rtsp
      "headers": { "User-Agent": "...", "Referer": "..." },  // 可选，附加请求头
      "hls_time": 4,          // 可选，分片时长（秒），2-30，默认 4
      "keep_segments": 12     // 可选，保留分片数（缓冲窗口），3-60，默认 12
    }

响应：

    {
      "sid": "3f2a...",                       // 会话 id
      "playlist_url": "/api/player/relay/3f2a.../index.m3u8",
      "hls_time": 4,
      "keep_segments": 12,
      "msg": "relay session started"
    }

失败时返回 400 与 msg（例如服务端未安装 ffmpeg、URL 非法、会话数达到上限）。

### GET /api/player/relay/{sid}/status

查询会话状态：

    {
      "sid": "...",
      "url": "源地址",
      "alive": true,            // ffmpeg 进程是否存活
      "playlist_ready": true,   // index.m3u8 是否已生成（可开始播放）
      "playlist_url": "/api/player/relay/{sid}/index.m3u8",
      "started_at": 1753320000,
      "age_secs": 12,
      "idle_secs": 1,           // 距离上次拉取 playlist/分片 的秒数
      "hls_time": 4,
      "keep_segments": 12,
      "segment_count": 8,       // 本地已缓存的分片数
      "last_error": ""          // ffmpeg 最近报错（尾部）
    }

客户端典型流程：start 后轮询 status（每 1s），playlist_ready 为 true 后切换播放源。

### GET /api/player/relay/{sid}/{file}

拉取本地 HLS 文件（index.m3u8 / segment_00001.ts ...）。
playlist 响应带 Cache-Control: no-store；每次请求会刷新会话活动时间。

### DELETE /api/player/relay/{sid}

停止会话：终止 ffmpeg 进程并删除缓存目录。

### GET /api/player/relay

列出所有会话状态（调试用）。

## 3. 会话生命周期与清理

- 会话上限 16 个：超出时自动淘汰最久没有活动的会话。
- 闲置 90 秒自动停止（客户端在播放期间会持续拉取 playlist/分片，属于活跃状态）。
- 后台每 60 秒扫描一次：回收已死/闲置会话，并删除服务端重启后遗留的孤儿缓存目录（static/player/*）。
- 服务端进程退出时不会主动清理；下次启动后由孤儿扫描回收。

## 4. 播放相关依赖

- 中继模式依赖服务端安装 ffmpeg（与截图、转播功能共用同一依赖）。
- ffmpeg 参数：-c copy（不转码）、-rw_timeout 15s（源站卡死自动重连）、-hls_flags delete_segments（滚动删除旧分片）。

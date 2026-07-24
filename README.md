# iptv-checker-rs

## command usage

iptv-checker-rs 包含2个命令

```bash
Usage: iptv-checker-rs <COMMAND>

Commands:
  web    web相关命令
  check  检查相关命令
  fetch  搜索相关命令
  ob     转播相关命令
  help   Print this message or the help of the given subcommand(s)
```

iptv-checker-rs 检查相关的命令

```bash
Usage: iptv-checker-rs check [OPTIONS]

Options:
  -i, --input-file <INPUT_FILE>    输入文件，可以是本地文件或者是网络文件，支持标准m3u格式以及非标准的格式： CCTV,https://xxxx.com/xxx.m3u8格式
  -o, --output-file <OUTPUT_FILE>  输出文件，如果不指定，则默认生成一个随机文件名 [default: ]
  -t, --timeout <TIMEOUT>          超时时间，默认超时时间为28秒 [default: 28000]
      --debug                      debug使用，可以看到相关的中间日志
  -c, --concurrency <CONCURRENCY>  并发数 [default: 1]
      --like <KEYWORD_LIKE>        想看关键词
      --dislike <KEYWORD_DISLIKE>  不想看关键词
      --sort                       频道排序
      --no_check                   是否不需要检查
      --rename                     去掉无用的字段
      --ffmepg_check               使用ffmpeg检查
  -h, --help                       Print help
```

iptv-checker-rs web相关的命令

```bash
Usage: iptv-checker-rs web [OPTIONS]

Options:
      --start        启动一个web服务
      --port <PORT>  指定这个web服务的端口号，默认8089 [default: 8089]
      --stop         关闭这个web服务
      --status       输出当前web服务的状态，比如pid信息
  -h, --help         Print help
```

iptv-checker-rs fetch搜索相关的命令

```bash

Usage: iptv-checker-rs fetch [OPTIONS]

Options:
      --search <SEARCH>  搜索频道名称,如果有别名，用英文逗号分隔 [default: ]
      --thumbnail        是否需要生成缩略图
      --clear            清理资源池
  -h, --help             Print help
```

iptv-checker-rs ob转播相关的命令

```bash

Usage: iptv-checker-rs ob --input-url <INPUT_URL>

Options:
  -i, --input-url <INPUT_URL>  需要转播的链接
  -h, --help                   Print help
```

## 环境变量

| 变量名 | 说明 | 默认值 |
| --- | --- | --- |
| `WEB_PORT` | Web 服务端口号 | `8089` |

## GitHub Token 配置

GitHub API 对未认证请求有严格的频率限制（60次/时），配置 token 后可提升至 5000次/时。

### 申请步骤

1. 登录 GitHub，访问 **[github.com/settings/tokens](https://github.com/settings/tokens)**
2. 点击 **Generate new token** → 选择 **Fine-grained token**（推荐）或 **Tokens (classic)**
3. 配置权限（最小化原则）：
   - **Expiration**: 自定义过期时间
   - **Repository access**: 选择 `Public repositories (read-only)`
   - **Permissions**: `Contents` → `Read-only`
4. 点击 **Generate token**，复制生成的 `github_pat_xxxxxxxxxxxx` 字符串
5. 打开 Web 管理后台 → 系统配置 → base.json，将 token 填入 `github_token` 字段

> 保存时系统会自动调用 GitHub API 验证 token 是否有效，无效会提示错误。

### 请求策略

| 场景 | 行为 |
| --- | --- |
| 已配置有效 token | 使用 GitHub API（认证，5000次/时） |
| 未配置 token | 先尝试 API，触发限流（403/429）后自动降级为 HTML 页面解析 |
| token 无效 | 保存时拒绝，提示错误信息 |

## build

```bash
make build
```

## build 打包问题处理

### windows

使用windows需要安装下面的连接器

- `brew install mingw-w64` #链接器

## 更新日志

- 4.7.1
  - **Bug 修复**:
    - 修复 bool 字段接收字符串 `"true"`/`"false"` 导致 400 错误（`fast_sort`、`sort`、`no_check` 等 10 个字段）
    - 修复创建任务时 URL 前导/后置空格未过滤的问题
    - 修复 `/tasks/detail` 返回的 M3U 内容缺少 `x-tvg-url` header
    - 修复 `no_check=true` + `ffmpeg_check=true` + `video_quality` 组合导致频道全部被清空
    - 修复 `video_quality` 在 `ffmpeg_check=false` 时静默忽略且无提示
    - 清理 `valid()` 冗余代码和 `SearchOptions.quality` 未使用字段
  - **安全加固**: 所有 bool 字段统一使用灵活反序列化（`deserialize_bool_flexible`）
- 4.7.0
  - **网络配置独立**: proxy、headers、user_agent 从 base.json 拆分到 network.json
  - **EPG 管理**: 新增 EPG 源配置/同步/缓存 API、频道列表查询、自定义 XML 生成
  - **分组映射**: 新增频道分组映射及未映射频道查询 API
  - **配置导入导出**: 支持 ZIP 格式的系统配置备份和恢复
  - **安全**: SSRF 防护（内网 IP + 危险协议拦截）、URL 合法性校验
- 4.6.0
  - **GitHub 抓取迁移至 REST API**: 将 GitHub 仓库文件获取从 HTML 页面解析改为 GitHub REST API (`api.github.com`)
    - `github_token` 配置在 base.json 中，保存时自动验证有效性
    - 未配置 token 时 API 优先，触发限流自动降级为 HTML 页面解析
    - 修复 `extensions` 为空且 `include_files` 非空时无法获取文件的问题
  - **安全加固**:
    - 修复 `/system/open-url` 端点 SSRF 漏洞，增加内网 IP 和危险协议拦截
    - 修复 `/media/upload` 端点路径穿越漏洞，文件名增加安全过滤
    - GitHub API 客户端移除 `danger_accept_invalid_certs`，始终验证 TLS 证书
  - **性能优化**:
    - 新增共享 HTTP 客户端 (`HTTP_CLIENT` / `GITHUB_CLIENT`)，复用连接池
    - 修复 `Task::run_inner` 中 15+ 次不必要的 `self.clone()`，改为借用
    - EPG HTML 解析的正则表达式改为静态编译
    - 删除重复的 `get_url_body` 函数
  - **代码质量**:
    - 将所有 `println!` / `eprintln!` 迁移至 `log` 宏
    - 清理未使用的导入和依赖（移除 `tempfile` crate）
  - **Bug 修复**:
    - 修复 EPG 定时刷新从未执行的问题（`init_epg_data` 漏了 `.await`）
    - 修复任务 panic 后 `is_running` 永久卡在 true 的问题
    - 修复字符串替换默认配置 `" ": ""` 会删除所有空格的问题
    - 修复替换配置保存时错误调用无关初始化函数的问题
    - 修复 `init_search_data` 使用 `.expect()` 导致 panic 的问题
    - 修复 `do_check` 错误返回值被丢弃的问题
    - 修复调度器线程无法优雅退出的问题
    - 修复检查结果计数在去重前计算导致终端显示数量与实际输出不一致的问题
    - 修复 `/q` 端点 host 只从 logos.json 读取，base.json 配置不生效的问题
    - 修复 `/q` 端点 host 为空时仍输出无效 `x-tvg-url` 的问题
    - 修复 gz 解压失败无降级方案的问题（尝试作为原始 XML 处理）
    - 新增默认 EPG 源：`http://epg.51zmt.top:8000/e.xml.gz`
- 4.4.0
  - 支持配置备份和恢复
  - 支持ipv4、ipv6结果的单独导出
  - 一些问题的修复
- 4.3.0
  - 新增台标上传配置
  - 修复爬取频道的错误导致服务异常
- 4.2.0
  - 新增想看频道、爬虫相关接口
  - json配置移动到core文件夹下
- 4.1.9
  - 将繁体字转换简体字
  - 修复检查后，最后更新时间没有更新的问题
  - 增加【字符替换配置】接口
- 4.1.7
  - 支持视频分辨率过滤(仅当ffmpeg检查生效)
  - 修复后台可能不正常执行的bug
- 4.1.6.1
  - 尝试使用cursor编程
  - 大改json数据结构
  - 编译后无相关错误信息
  - bug修复
    - 修复检查部分有效的源判定为无效
- 4.1.6
  - 已解决ffmpeg检查卡顿的问题
  - 解决网页刷新后显示不存在的问题
-4.1.5
  - 尝试解决ffmpeg检查卡顿的问题
- 4.1.4.5
  - 增加文件日志，方便排查问题
- 4.1.4
  - 修复了ffmepg检查导致后台任务无法进行的问题
  - 优化了重命名频道名称导致检查卡住的问题
  - 支持仅保留2个相同名称的源
  - http检查时rtmp等非http源，可跳过
- 4.1.3
  - 修复后台检查失败，导致所有任务无法进行
- 4.1.2
  - 去掉节目名称中的一些无用字符，比如`[HD]`或者`123231 [SD]`
  - 修复不检查时导出的文件为空的bug
  - cmd模式搜索频道模式
  - 支持强制ffmpeg检查，检测结果更加准确
- 4.1.1
  - 修复无法解析复杂的m3u文件的bug
- 4.1.0
  - 排序支持按照字母加数组的排序，而非自然语言排序（原x1,x10,x11,x2，更改后x1,x2,x10,x11）
  - 修复【是否不需要检查】参数保存不生效
  - 任务列表每次出现都是随机，未按照创建时间排序
  - 任务结束后同步生成.txt文件
- 4.0.1
  - web任务
    - 支持不检查（仅获取源）
    - 支持任务导入、导出
- 3.2.1
  - web支持并发、排序设置
- 3.2.0
  - 支持关键词匹配
  - 支持超时时间配置
- 3.1.1
  - 修复后台检查后cpu增高的问题
- 3.1.0
  - 支持任务编辑
  - 支持任务立即执行
- 3.0.0
  - 支持后台检查
- 1.0.2
  - 优化了错误信息
  - 支持多个文件检查
- 1.0.1
  - 支持并发
- 1.0.0
  - rust版本支持

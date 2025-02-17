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

## build

```bash
make build
```

## build 打包问题处理

### windows

使用windows需要安装下面的连接器

- `brew install mingw-w64` #链接器

## 更新日志

- 4.1.4
  - 修复了ffmepg检查导致后台任务无法进行的问题
  - 优化了重命名频道名称导致检查卡住的问题
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

# 更新日志 / Changelog

本项目的所有显著变更记录于此文件。
格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [0.4.0] - 2026-11-21

### 新增

- **播放列表拖拽排序**：按住曲目拖到目标位置即可重排，橙色指示线标示插入点；
  播放中的曲目重排不中断，当前曲目自动跟随移动；靠近列表边缘自动滚动；
  搜索过滤状态下禁用拖拽（可见项与真实索引不连续）
- **缓冲进度显示**：主界面与视频浮层进度条实时显示已缓冲段
  （主界面橙色 / 视频浮层白色半透明），流式加载卡顿时可直观看到加载进度

### 修复

- **持久化写入被播放饿死**：播放中 `timeupdate`（约 4 次/秒）不断重置 500ms
  防抖定时器，导致播放期间列表/位置完全不落盘（实测故障）。改为关键操作
  （导入/删除/清空/拖拽排序/切曲/暂停/模式切换）立即落盘；位置写入改为节流，
  续播位置滞后从"可能数分钟"收敛到约 1 秒
- 播放补全曲目时长后立即落盘（原先依赖被饿死的防抖，重启后时长丢失）

### CI

- lint-test job 补占位 ffmpeg sidecar，修复 tauri-build 编译期存在性校验
  （`resource path binaries/ffmpeg-... doesn't exist`）导致的门禁失败
- `decode_key` 按 cfg 拆分 base64 解码链，修复 Linux 下 clippy
  `bind_instead_of_map`（`-D warnings`）报错

## [0.3.0] - 2026-11-19

### 新增

- **播放状态持久化**：播放列表、播放模式、上次播放曲目与位置跨重启保留，
  重启后点播放键从上次位置续播
- **窗口几何记忆**：位置、尺寸与最大化状态跨重启保留；位置落到屏幕外时自动放弃恢复
- **系统托盘**：托盘菜单（显示主窗口 / 迷你播放器 / 退出）；"关闭窗口时
  最小化到托盘"默认开启（设置页可关），关闭按钮不再直接退出
- **单实例锁**：二次启动立即退出新进程并唤起已有主窗口
- **倍速播放**：1.0 / 1.25 / 1.5 / 2.0 / 0.75 循环切换，记忆上次选择
- **字幕支持**：视频字幕轨道列表（SRT/ASS/MOV_TEXT/WebVTT 文本轨，
  位图轨自动排除），按需提取为 WebVTT 缓存，视频浮层 CC 按钮循环切换
- **MediaSession**：系统媒体键、锁屏与系统媒体面板控制（播放/暂停/切歌/seek）
- **拖放导入**：文件或文件夹直接拖入窗口即导入（新增 `scan_paths` 命令）
- **快捷键扩展**：`↑/↓` 音量、`M` 静音、`N/P` 切歌、`F` 窗口全屏、
  `L` 播放模式、`0-9` 跳到对应百分比（原有 `Space`/`←→`/`Esc` 保留）
- **播放列表搜索**：按标题/艺术家实时过滤
- **设置页**新增"通用"分组（关闭到托盘开关）
- CI：tag `v*` 触发自动发布 Release；新增 fmt/clippy/test 质量门禁 job；
  ffmpeg sidecar 改为 CI 内预编译下载（Linux/Windows）与脚本自建（macOS）并缓存
- 新增 macOS ffmpeg 构建脚本与 `tauri.macos.conf.json` 平台配置
- 新增 `dependabot.yml`（actions / cargo / npm）

### 修复

- **Windows 性能**：`safe_link` 不再为 probe/decode/remux 整文件复制媒体文件
  （此前 probe 一个 4GB 视频会先复制 4GB）
- **后端健壮性**：ffmpeg/ffprobe 调用增加超时保护（probe 30s / cover 60s /
  decode 900s / remux 3600s），坏文件不再可能永久挂起；解码与重封装经
  全局闸门串行化，快速切歌不再重复解码同一文件
- **回环服务安全**：本地 HTTP 服务增加每次启动随机的访问 token，
  其他本地进程无法通过该端口读取任意文件
- 主题背景硬编码 `#fbf5ef` 导致换任何主题窗口背景都不变 → 跟随主题色板
- 空媒体时按 `←` 抛 `InvalidStateError` → 补时长保护
- 迷你播放器 `data-tauri-drag-region="deep"` 无效值导致几乎无法拖动
- 滚轮调音量不持久化；音量保存改为防抖写盘
- 视频单击/双击播放状态冲突（双击先触发两次 click）
- 迷你卡 seek 逐 input 事件的 IPC 风暴 → 预览 + change 提交
- 迷你卡清空列表后标题/封面残留、封面无 onerror 回退
- 时长显示支持小时（`h:mm:ss`）
- 缓存清理覆盖子目录；日志保留上一代（`player.log.1`）便于复盘崩溃
- ffmpeg 构建脚本：移除硬编码 `/tmp/opencode` 路径与失效 zlib 依赖
  （Windows 脚本改为自建 zlib）；下载源改官方源优先；可选库开关统一为
  `ENABLE_X264` / `ENABLE_DAV1D`（兼容旧 `FFMPEG_NO_*`）；
  dav1d（BSD 许可）不再强制 `--enable-gpl`

### 变更

- Windows 构建目标统一为 `x86_64-pc-windows-msvc`（与 CI/README 一致；
  sidecar 二进制为独立进程，不受宿主工具链影响）
- 各平台打包目标显式收窄：Windows NSIS（中英双语安装界面）、
  Linux deb+AppImage（补 `libwebkit2gtk-4.1-0`/`libgtk-3-0` 依赖声明）、
  macOS app+dmg
- CI 移除暂不可构建的 Linux aarch64 与 macOS x86_64（原因见 workflow 注释）
- 删除 CI 中无效的 Tauri v1 签名环境变量；tauri-cli 改用 npm 预编译安装
- 清理前端死代码（隐藏 play/pause 按钮、注释块、无效 `-webkit-app-region` 等）

## [0.2.1] - 2026-11-14

- 仓库整理：四语 README、MIT LICENSE、THIRD_PARTY_NOTICES；
  FFmpeg 二进制移出版本管理

## [0.2.0] - 2026-11-14

- 初始公开版本：音频播放与常见视频容器可用
- 透明无边框窗口、三种全屏、迷你播放器、播放列表（文件/文件夹导入，
  列表循环/单曲循环/随机）、封面与标签（ffprobe）、主题系统
  （默认/月份/自定义）、回环 HTTP 流服务（Range 请求）
- 平台差异处理：Windows 走 WebView2 原生解码判定 + ffmpeg→WAV 回退，
  Linux 走 WebKitGTK/GStreamer

## [0.1.0] - 2026-11（内部）

- 项目雏形

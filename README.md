<div align="center">

# VinePalma Player 🌴

**基于 FFmpeg 的本地音视频播放器 —— 透明无边框窗口，一个可拎出来的迷你播放器**

<a href="#-简体中文">简体中文</a> ·
<a href="#-繁體中文">繁體中文</a> ·
<a href="#-english">English</a> ·
<a href="#-日本語">日本語</a>

</div>

---

## 🇨🇳 简体中文

VinePalma Player 是 **Tauri 2 + Rust** 写的桌面播放器。前端是原生 HTML / CSS / JavaScript，没有打包器；后端负责解码调度、本地流服务和元数据读取。音视频文件**只在本机读取**，不联网、不上传。

> 状态：**0.3.0**。音频/视频播放、播放列表持久化、字幕轨道、系统托盘、迷你播放器等功能已可用。

### 环境要求

| 依赖 | 版本 | 说明 |
|---|---|---|
| Rust | ≥ 1.77 | Tauri 2 要求（Windows 用 MSVC 工具链） |
| Node.js | ≥ 18 | 用来跑 Tauri CLI 的 npm 脚本 |
| Tauri CLI | 2.x | `npm i -g @tauri-apps/cli@^2`（预编译，推荐） |
| FFmpeg 构建工具链 | 见下 | 仓库**不包含** ffmpeg 二进制，需要自己构建或下载预编译 |

Windows 还需 **WebView2** 与 **MSVC 生成工具**（Visual Studio Build Tools，含 C++ 工作负载）；Linux 需要 `libwebkit2gtk-4.1-dev` 等 Tauri 常规依赖；macOS 需要 Xcode 命令行工具。

### 关键一步：先准备 FFmpeg sidecar

播放器的「非原生格式」全靠随应用分发的 `ffmpeg` / `ffprobe` 静态二进制。为了不把 GPL 构建产物塞进 git 历史，这些文件**不在仓库里**，放在 `src-tauri/binaries/`（命名 `<名字>-<target-triple>`）。两种获取方式：

**方式一：本机下载预编译（最快，推荐 Windows）**

从 [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds/releases)（win64/linux64）或 [gyan.dev](https://www.gyan.dev/ffmpeg/builds/)（Windows）下载静态构建，把 `ffmpeg.exe` / `ffprobe.exe` 重命名为：

- Windows: `ffmpeg-x86_64-pc-windows-gnu.exe` / `ffprobe-x86_64-pc-windows-gnu.exe`
- Linux: `ffmpeg-x86_64-unknown-linux-gnu` / `ffprobe-x86_64-unknown-linux-gnu`
- macOS: `ffmpeg-aarch64-apple-darwin` / `ffprobe-aarch64-apple-darwin`

> sidecar 是独立进程，mingw 编译的 ffmpeg.exe 配 MSVC 构建的应用没有影响；
> `-pc-windows-gnu` 后缀只描述二进制本身的来源。

**方式二：源码构建**

```bash
npm run build:ffmpeg:windows   # 在 Linux 上交叉编译 Windows 版（需 mingw-w64）
npm run build:ffmpeg:linux     # Linux x86_64 本机构建
npm run build:ffmpeg:macos     # macOS 本机构建（arm64/x86_64）
```

> 注意：Windows 版脚本是 **Linux 交叉编译脚本**，不能在 Windows 本机直接运行；
> Windows 本机请用方式一。脚本自建 zlib、x264（Windows/Linux）与 dav1d（Linux，
> 可选），首次编译耗时较长。

> 脚本默认开启 `--enable-gpl --enable-libx264`（H.264 编码需要）。要避开 GPL，设置 `ENABLE_X264=0` 重新构建即可，代价是无法编码 H.264。其余开关：`ENABLE_DAV1D=0` 关闭 AV1 解码、`ENABLE_ZLIB=0` 关闭 zlib（仅 Windows 脚本）。

### 构建与运行

```bash
tauri dev                    # 开发模式（或 cargo tauri dev）
npm run build                # 打包（当前平台全部目标）
npm run build:windows        # Windows NSIS（x86_64-pc-windows-msvc）
npm run build:linux          # Linux deb + AppImage
npm run build:macos          # macOS app + dmg（Apple Silicon）
```

### 功能

- **透明无边框窗口**：可拖拽、可置顶、记住大小与位置
- **三种全屏**：窗口全屏、内容全屏、退出后回到原尺寸
- **迷你播放器**：一键收成小窗，常驻桌面角落
- **系统托盘**：关闭窗口可最小化到托盘继续播放（可在设置中关闭），托盘菜单退出
- **单实例锁**：重复启动自动唤起已有窗口
- **播放列表**：选择文件、导入文件夹或直接拖入文件/文件夹；列表循环 / 单曲循环 / 随机；搜索过滤；**列表与播放位置跨重启保留**
- **倍速播放**：1.0–2.0x 循环切换并记忆
- **字幕**：视频文本字幕轨（SRT/ASS/MOV_TEXT/WebVTT）自动转 WebVTT，CC 按钮切换
- **视频播放**：原生容器直出，其余容器用 ffmpeg 重封装后再播
- **音频**：浏览器能解的直出，解不了的走 ffmpeg → WAV 回退，保证听起来一致
- **封面与标签**：从 ffprobe 读 title / artist / album，缓存封面
- **MediaSession**：系统媒体键、系统媒体面板控制
- **快捷键**：`Space` 播放/暂停、`←/→` 快退/快进、`↑/↓` 音量、`M` 静音、
  `N/P` 切歌、`F` 窗口全屏、`L` 播放模式、`0-9` 跳百分比、`Esc` 退出全屏/关列表
- **主题**：默认主题、月份主题、自定义配色

### 支持的格式

- **音频**：mp3、wav、flac、m4a、aac、ogg、opus、wma、ape、aiff、wv、mpc、tak、tta、alac、amr、ac3、eac3、dts、dsf、caf、m4b、mka
- **视频**：mp4、m4v、mkv、webm、mov、avi、wmv、3gp、mpg、mpeg、ts、m2ts、flv、rm、rmvb、asf
- **字幕**（视频容器内嵌文本轨）：subrip/srt、ass/ssa、mov_text、webvtt

实际能不能播取决于构建进 ffmpeg 的解码器；精简构建会少一部分冷门格式。

### 目录结构

```
src/                 前端（原生 HTML / CSS / JS）
  index.html         主窗口
  mini.html          迷你播放器
  settings/          设置页
  theme.js           主题与配色
src-tauri/
  src/lib.rs         Tauri 入口、命令注册、托盘、窗口事件
  src/media.rs       格式判定、ffmpeg 调度、元数据、字幕提取
  src/server.rs      127.0.0.1 回环 HTTP 服务（Range 请求 + 访问 token）
  src/log.rs         日志
  binaries/          ffmpeg / ffprobe（不纳入版本管理）
scripts/             构建 ffmpeg 的脚本（windows=Linux 交叉编译 / linux / macos）
.github/workflows/   CI：质量门禁 + 三平台构建 + tag 发布
```

### 它是怎么播的

1. 前端通过 `tauri://` 命令把文件路径交给 Rust
2. Rust 判断容器能否被 WebView 原生播放
   - 能 → 直接从本地回环 HTTP 服务推流
   - 不能 → 先用 ffmpeg 重封装（视频）或转 WAV（音频），再推流
3. 回环服务只绑定 `127.0.0.1`，支持 Range 请求，所以拖动进度条可用；URL 带每次启动随机的访问 token，其他本地进程无法借此读取文件

### 已知限制

- 只做播放，不做转码导出
- 位图字幕（PGS/DVB/DVD 字幕）不支持显示
- 播放列表不支持拖拽排序
- `macOS` 构建尚未实测（CI 已产出 aarch64 产物）

## 🇹🇼 繁體中文

VinePalma Player 是以 **Tauri 2 + Rust** 撰寫的桌面播放器。前端是原生 HTML / CSS / JavaScript，沒有打包器；後端負責解碼調度、本機串流服務與中繼資料讀取。影音檔案**只在本機讀取**，不連網、不上傳。

> 狀態：**0.3.0**。音訊／視訊播放、播放清單持久化、字幕軌、系統托盤、迷你播放器等功能已可用。

### 環境需求

| 依賴 | 版本 | 說明 |
|---|---|---|
| Rust | ≥ 1.77 | Tauri 2 要求（Windows 用 MSVC 工具鏈） |
| Node.js | ≥ 18 | 用來執行 Tauri CLI 的 npm 腳本 |
| Tauri CLI | 2.x | `npm i -g @tauri-apps/cli@^2` |
| FFmpeg 建置工具鏈 | 見下 | 倉庫**不含** ffmpeg 二進位檔，需自行建置或下載預編譯 |

### 關鍵一步：先準備 FFmpeg sidecar

「非原生格式」全靠隨應用分发的 `ffmpeg` / `ffprobe` 靜態二進位檔。為了不把 GPL 建置產物塞進 git 歷史，這些檔案**不在倉庫裡**。可從 [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds/releases) 下載預編譯版本重命名放入 `src-tauri/binaries/`，或以腳本源碼建置：

```bash
npm run build:ffmpeg:windows   # 在 Linux 上交叉編譯 Windows 版（需 mingw-w64）
npm run build:ffmpeg:linux     # Linux 本機
npm run build:ffmpeg:macos     # macOS 本機
```

結果會放到 `src-tauri/binaries/`，命名為 `<名稱>-<target-triple>`。

> 腳本預設開啟 `--enable-gpl --enable-libx264`。要避開 GPL，設定 `ENABLE_X264=0` 重新建置，代價是無法編碼 H.264。

### 建置與執行

```bash
npm run dev
npm run build:windows   # Windows NSIS（x86_64-pc-windows-msvc）
npm run build:linux     # Linux deb + AppImage
npm run build:macos     # macOS app + dmg
```

### 功能

- **透明無邊框視窗**：可拖曳、可置頂、記住大小與位置
- **三種全螢幕**：視窗全螢幕、內容全螢幕
- **迷你播放器**：一鍵收成小窗
- **系統托盤**：關閉可最小化到托盤續播；**單實例鎖**
- **播放清單**：選檔／資料夾／拖放匯入，列表／單曲／隨機、搜尋過濾、**跨重啟保留**
- **倍速播放**：1.0–2.0x
- **字幕**：內嵌文字字幕軌（SRT/ASS/MOV_TEXT/WebVTT）轉 WebVTT，CC 切換
- **視訊播放**：原生容器直出，其餘容器先重封裝
- **音訊**：可直出的直出，其餘走 ffmpeg → WAV 回退
- **封面與標籤**：ffprobe 讀 title / artist / album
- **MediaSession**：系統媒體鍵控制
- **主題**：預設、月份、自訂配色

### 支援格式

- **音訊**：mp3、wav、flac、m4a、aac、ogg、opus、wma、ape、aiff、wv、mpc、tak、tta、alac、amr、ac3、eac3、dts、dsf、caf、m4b、mka
- **視訊**：mp4、m4v、mkv、webm、mov、avi、wmv、3gp、mpg、mpeg、ts、m2ts、flv、rm、rmvb、asf
- **字幕**：subrip/srt、ass/ssa、mov_text、webvtt（內嵌文字軌）

## 🇬🇧 English

VinePalma Player is a desktop player built with **Tauri 2 + Rust**. The front end is plain HTML / CSS / JavaScript with no bundler; the Rust side handles decode dispatch, a local streaming server and metadata. Media files are **read locally only** — nothing is uploaded.

> Status: **0.3.0**. Audio/video playback, playlist persistence, subtitles, tray and the mini player are all working.

### Requirements

| Dependency | Version | Notes |
|---|---|---|
| Rust | ≥ 1.77 | required by Tauri 2 (MSVC toolchain on Windows) |
| Node.js | ≥ 18 | to run the Tauri CLI npm scripts |
| Tauri CLI | 2.x | `npm i -g @tauri-apps/cli@^2` |
| FFmpeg toolchain | see below | the repo does **not** ship ffmpeg binaries — build or download them |

### First: prepare the FFmpeg sidecar

Formats the WebView cannot decode natively rely on the bundled static `ffmpeg` / `ffprobe`. To keep GPL build artifacts out of git history, those files are **not in the repo** (`src-tauri/binaries/`, named `<name>-<target-triple>`). Either download prebuilt static builds from [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds/releases) and rename them, or build from source:

```bash
npm run build:ffmpeg:windows   # cross-compiles the Windows build on Linux (needs mingw-w64)
npm run build:ffmpeg:linux     # native Linux build
npm run build:ffmpeg:macos     # native macOS build
```

> The Windows script is a **Linux cross-compilation script** — it cannot run on Windows itself. Use the prebuilt download path there instead.

> The script enables `--enable-gpl --enable-libx264` by default. Set `ENABLE_X264=0` to build without GPL code — you lose H.264 encoding.

### Build and run

```bash
npm run dev                 # development
npm run build               # release bundle
npm run build:windows       # Windows NSIS (x86_64-pc-windows-msvc)
npm run build:linux         # Linux deb + AppImage
npm run build:macos         # macOS app + dmg (Apple Silicon)
```

### Features

- **Transparent frameless window** — draggable, pinnable, remembers size and position
- **Three fullscreen modes** — window fullscreen, content fullscreen
- **Mini player** — collapse into a small always-available window
- **System tray** — close-to-tray keeps playing in the background; single-instance lock
- **Playlist** — import files, folders or drag & drop; list loop / repeat one / shuffle; search filter; **persisted across restarts with the last playback position**
- **Playback speed** — 1.0–2.0x, remembered
- **Subtitles** — embedded text tracks (SRT/ASS/MOV_TEXT/WebVTT) converted to WebVTT, CC button to switch
- **Video** — native containers stream as-is, others are remuxed through ffmpeg first
- **Audio** — native when the WebView supports it, otherwise an ffmpeg → WAV fallback for consistent sound
- **Covers and tags** — title / artist / album via ffprobe, cover art cached
- **MediaSession** — system media keys and OS media panel control
- **Themes** — default, monthly presets, custom colours

### Supported formats

- **Audio**: mp3, wav, flac, m4a, aac, ogg, opus, wma, ape, aiff, wv, mpc, tak, tta, alac, amr, ac3, eac3, dts, dsf, caf, m4b, mka
- **Video**: mp4, m4v, mkv, webm, mov, avi, wmv, 3gp, mpg, mpeg, ts, m2ts, flv, rm, rmvb, asf
- **Subtitles** (embedded text tracks): subrip/srt, ass/ssa, mov_text, webvtt

Whether a format actually plays depends on the decoders compiled into your ffmpeg build; minimal builds drop the rarer ones.

### How playback works

1. The front end hands a file path to Rust through Tauri commands
2. Rust decides whether the container is natively playable in the WebView
   - yes → stream it straight from the loopback HTTP server
   - no → remux (video) or transcode to WAV (audio) with ffmpeg, then stream
3. The loopback server binds `127.0.0.1` only, honours Range requests (so seeking works), and requires a per-launch random token so other local processes cannot read files through it

### Known limitations

- Playback only, no transcoding/export
- Bitmap subtitle formats (PGS/DVB/DVD subtitles) are not rendered
- No drag-to-reorder in the playlist
- macOS builds are not hand-tested yet (CI produces aarch64 artifacts)

## 🇯🇵 日本語

VinePalma Player は **Tauri 2 + Rust** 製のデスクトッププレイヤーです。フロントエンドは素の HTML / CSS / JavaScript でバンドラなし、Rust 側がデコードの振り分け・ローカル配信・メタデータ取得を担当します。メディアファイルは**ローカルで読み込むだけ**で、送信はしません。

> 状態：**0.3.0**。音声／動画再生、プレイリスト永続化、字幕トラック、システムトレイ、ミニプレイヤーが動作します。

### 必要なもの

| 依頼 | バージョン | 備考 |
|---|---|---|
| Rust | ≥ 1.77 | Tauri 2 の要件（Windows は MSVC ツールチェーン） |
| Node.js | ≥ 18 | Tauri CLI の npm スクリプト用 |
| Tauri CLI | 2.x | `npm i -g @tauri-apps/cli@^2` |
| FFmpeg ビルド環境 | 下記 | リポジトリに ffmpeg バイナリは**含まれません** — ビルドまたは prebuilt を取得 |

### 最初に FFmpeg sidecar を準備

WebView がネイティブに扱えない形式は、同梱の静的 `ffmpeg` / `ffprobe` に依存します。GPL のビルド成果物を git 履歴に入れないため、これらは**リポジトリに含めていません**（`src-tauri/binaries/`、命名 `<name>-<target-triple>`）。[BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds/releases) から prebuilt を取得してリネームするか、ソースからビルドします：

```bash
npm run build:ffmpeg:windows   # Linux 上で Windows 版をクロスコンパイル（mingw-w64 必須）
npm run build:ffmpeg:linux     # Linux ネイティブ
npm run build:ffmpeg:macos     # macOS ネイティブ
```

> Windows 用スクリプトは **Linux 上のクロスコンパイル用**です。Windows では prebuilt の取得を推奨します。

> 既定で `--enable-gpl --enable-libx264` を有効にします。GPL を避けたい場合は `ENABLE_X264=0` で再ビルドしてください（H.264 エンコードは不可）。

### ビルドと実行

```bash
npm run dev
npm run build:windows   # Windows NSIS（x86_64-pc-windows-msvc）
npm run build:linux     # Linux deb + AppImage
npm run build:macos     # macOS app + dmg
```

### 機能

- **透明・枠なしウィンドウ** — ドラッグ可、最前面固定、サイズと位置を記憶
- **3 種類の全画面** — ウィンドウ全画面／コンテンツ全画面
- **ミニプレイヤー** — 小さな常駐ウィンドウに折りたたみ
- **システムトレイ** — 閉じてもトレイで再生継続；**シングルインスタンスロック**
- **プレイリスト** — ファイル／フォルダ／ドラッグ＆ドロップで読み込み、リストループ／1 曲リピート／シャッフル、検索フィルタ、**再起動後も復元**
- **再生速度** — 1.0–2.0x、記憶あり
- **字幕** — 埋め込みテキスト字幕（SRT/ASS/MOV_TEXT/WebVTT）を WebVTT に変換、CC ボタンで切替
- **動画** — ネイティブ対応コンテナはそのまま、それ以外は ffmpeg で再ラップ
- **音声** — WebView が再生できるものはそのまま、できないものは ffmpeg → WAV にフォールバック
- **カバーとタグ** — ffprobe で title / artist / album を取得しキャッシュ
- **MediaSession** — システムメディアキー制御
- **テーマ** — 既定、月別プリセット、カスタム配色

### 対応形式

- **音声**：mp3、wav、flac、m4a、aac、ogg、opus、wma、ape、aiff、wv、mpc、tak、tta、alac、amr、ac3、eac3、dts、dsf、caf、m4b、mka
- **動画**：mp4、m4v、mkv、webm、mov、avi、wmv、3gp、mpg、mpeg、ts、m2ts、flv、rm、rmvb、asf
- **字幕**：subrip/srt、ass/ssa、mov_text、webvtt（埋め込みテキストトラック）

実際に再生できるかは、ビルドした ffmpeg に含まれるデコーダ次第です。

### 既知の制限

- 再生のみ（変換・書き出しなし）
- ビットマップ字幕（PGS/DVB/DVD 字幕）は未対応
- プレイリストのドラッグ並べ替えは未対応
- macOS ビルドは実機未検証（CI は aarch64 生成物を作成）

---

## 许可 / License

代码以 [MIT](./LICENSE) 发布。随应用分发的 FFmpeg / x264 二进制适用各自的许可证，详见 [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)。

Code is released under the [MIT License](./LICENSE). The FFmpeg / x264 binaries you build and ship alongside the app are covered by their own licenses — see [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md).

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

> 状态：**0.2.0，开发中**。音频播放与常见视频容器已可用，UI 仍在打磨。

### 环境要求

| 依赖 | 版本 | 说明 |
|---|---|---|
| Rust | ≥ 1.77 | Tauri 2 要求 |
| Node.js | ≥ 18 | 只用来跑 `cargo tauri` 的 npm 脚本 |
| Tauri CLI | 2.x | `cargo install tauri-cli --version "^2"` |
| FFmpeg 构建工具链 | 见下 | 仓库**不包含** ffmpeg 二进制，需要自己构建 |

Windows 还需 **WebView2** 与 MSVC 生成工具；Linux 需要 `libwebkit2gtk-4.1-dev` 等 Tauri 常规依赖。

### 关键一步：先构建 FFmpeg

播放器的「非原生格式」全靠随应用分发的 `ffmpeg` / `ffprobe` 静态二进制。为了不把 GPL 构建产物塞进 git 历史，这些文件**不在仓库里**，需要本地生成：

```bash
# Windows（在 Git Bash / MSYS2 里执行）
npm run build:ffmpeg:windows

# Linux
npm run build:ffmpeg:linux
```

脚本会把结果放到 `src-tauri/binaries/`，命名规则是 `<名字>-<target-triple>`，例如 `ffmpeg-x86_64-pc-windows-gnu.exe`。这一步需要能联网拉取 FFmpeg 与 x264 源码，首次编译耗时较长。

> 脚本默认开启 `--enable-gpl --enable-libx264`（H.264 编码需要）。要避开 GPL，设置 `ENABLE_X264=0` 重新构建即可，代价是无法编码 H.264。

### 构建与运行

```bash
cargo tauri dev              # 开发模式
cargo tauri build            # 打包
npm run build:windows        # Windows NSIS（x86_64-pc-windows-gnu）
npm run build:linux          # Linux deb + AppImage
```

### 功能

- **透明无边框窗口**：可拖拽、可置顶、记住大小
- **三种全屏**：窗口全屏、内容全屏、退出后回到原尺寸
- **迷你播放器**：一键收成小窗，常驻桌面角落
- **播放列表**：选择文件或整个文件夹导入，列表循环 / 单曲循环 / 随机
- **视频播放**：原生容器直出，其余容器用 ffmpeg 重封装后再播
- **音频**：浏览器能解的直出，解不了的走 ffmpeg → WAV 回退，保证听起来一致
- **封面与标签**：从 ffprobe 读 title / artist / album，缓存封面
- **主题**：默认主题、月份主题、自定义配色

### 支持的格式

- **音频**：mp3、wav、flac、m4a、aac、ogg、opus、wma、ape、aiff、wv、mpc、tak、tta、alac、amr、ac3、eac3、dts、dsf、caf、m4b、mka
- **视频**：mp4、m4v、mkv、webm、mov、avi、wmv、3gp、mpg、mpeg、ts、m2ts、flv、rm、rmvb、asf

实际能不能播取决于构建进 ffmpeg 的解码器；精简构建会少一部分冷门格式。

### 目录结构

```
src/                 前端（原生 HTML / CSS / JS）
  index.html         主窗口
  mini.html          迷你播放器
  settings/          设置页
  theme.js           主题与配色
src-tauri/
  src/lib.rs         Tauri 入口、命令注册
  src/media.rs       格式判定、ffmpeg 调度、元数据
  src/server.rs      127.0.0.1 回环 HTTP 服务（Range 请求）
  src/log.rs         日志
  binaries/          ffmpeg / ffprobe（不纳入版本管理）
scripts/             构建 ffmpeg 的脚本
.github/workflows/   Linux / Windows / macOS 构建
```

### 它是怎么播的

1. 前端通过 `tauri://` 命令把文件路径交给 Rust
2. Rust 判断容器能否被 WebView 原生播放
   - 能 → 直接从本地回环 HTTP 服务推流
   - 不能 → 先用 ffmpeg 重封装（视频）或转 WAV（音频），再推流
3. 回环服务只绑定 `127.0.0.1`，支持 Range 请求，所以拖动进度条可用

### 已知限制

- 只做播放，不做转码导出
- 没有字幕轨道选择
- 播放列表不持久化，重启后需要重新导入
- `macOS` 构建尚未验证

## 🇹🇼 繁體中文

VinePalma Player 是以 **Tauri 2 + Rust** 撰寫的桌面播放器。前端是原生 HTML / CSS / JavaScript，沒有打包器；後端負責解碼調度、本機串流服務與中繼資料讀取。影音檔案**只在本機讀取**，不連網、不上傳。

> 狀態：**0.2.0，開發中**。音訊播放與常見視訊容器已可用，UI 仍在打磨。

### 環境需求

| 依賴 | 版本 | 說明 |
|---|---|---|
| Rust | ≥ 1.77 | Tauri 2 要求 |
| Node.js | ≥ 18 | 只用來執行 `cargo tauri` 的 npm 腳本 |
| Tauri CLI | 2.x | `cargo install tauri-cli --version "^2"` |
| FFmpeg 建置工具鏈 | 見下 | 倉庫**不含** ffmpeg 二進位檔，需自行建置 |

### 關鍵一步：先建置 FFmpeg

「非原生格式」全靠隨應用分发的 `ffmpeg` / `ffprobe` 靜態二進位檔。為了不把 GPL 建置產物塞進 git 歷史，這些檔案**不在倉庫裡**：

```bash
npm run build:ffmpeg:windows   # Windows（Git Bash / MSYS2）
npm run build:ffmpeg:linux     # Linux
```

結果會放到 `src-tauri/binaries/`，命名為 `<名稱>-<target-triple>`。

> 腳本預設開啟 `--enable-gpl --enable-libx264`。要避開 GPL，設定 `ENABLE_X264=0` 重新建置，代價是無法編碼 H.264。

### 建置與執行

```bash
cargo tauri dev
cargo tauri build
npm run build:windows
npm run build:linux
```

### 功能

- **透明無邊框視窗**：可拖曳、可置頂
- **三種全螢幕**：視窗全螢幕、內容全螢幕
- **迷你播放器**：一鍵收成小窗
- **播放清單**：選檔或整個資料夾匯入，列表 / 單曲 / 隨機
- **視訊播放**：原生容器直出，其餘容器先重封裝
- **音訊**：可直出的直出，其餘走 ffmpeg → WAV 回退
- **封面與標籤**：ffprobe 讀 title / artist / album
- **主題**：預設、月份、自訂配色

### 支援格式

- **音訊**：mp3、wav、flac、m4a、aac、ogg、opus、wma、ape、aiff、wv、mpc、tak、tta、alac、amr、ac3、eac3、dts、dsf、caf、m4b、mka
- **視訊**：mp4、m4v、mkv、webm、mov、avi、wmv、3gp、mpg、mpeg、ts、m2ts、flv、rm、rmvb、asf

## 🇬🇧 English

VinePalma Player is a desktop player built with **Tauri 2 + Rust**. The front end is plain HTML / CSS / JavaScript with no bundler; the Rust side handles decode dispatch, a local streaming server and metadata. Media files are **read locally only** — nothing is uploaded.

> Status: **0.2.0, in development**. Audio playback and common video containers work; the UI is still being polished.

### Requirements

| Dependency | Version | Notes |
|---|---|---|
| Rust | ≥ 1.77 | required by Tauri 2 |
| Node.js | ≥ 18 | only to run the `cargo tauri` npm scripts |
| Tauri CLI | 2.x | `cargo install tauri-cli --version "^2"` |
| FFmpeg toolchain | see below | the repo does **not** ship ffmpeg binaries |

### First: build FFmpeg

Formats the WebView cannot decode natively rely on the bundled static `ffmpeg` / `ffprobe`. To keep GPL build artifacts out of git history, those files are **not in the repo**:

```bash
npm run build:ffmpeg:windows   # Windows (run inside Git Bash / MSYS2)
npm run build:ffmpeg:linux     # Linux
```

Output lands in `src-tauri/binaries/` as `<name>-<target-triple>`. The script fetches FFmpeg and x264 sources, so the first build takes a while.

> The script enables `--enable-gpl --enable-libx264` by default. Set `ENABLE_X264=0` to build without GPL code — you lose H.264 encoding.

### Build and run

```bash
cargo tauri dev              # development
cargo tauri build            # release bundle
npm run build:windows        # Windows NSIS
npm run build:linux          # Linux deb + AppImage
```

### Features

- **Transparent frameless window** — draggable, pinnable, remembers its size
- **Three fullscreen modes** — window fullscreen, content fullscreen
- **Mini player** — collapse into a small always-available window
- **Playlist** — import files or a whole folder; list loop / repeat one / shuffle
- **Video** — native containers stream as-is, others are remuxed through ffmpeg first
- **Audio** — native when the WebView supports it, otherwise an ffmpeg → WAV fallback for consistent sound
- **Covers and tags** — title / artist / album via ffprobe, cover art cached
- **Themes** — default, monthly presets, custom colours

### Supported formats

- **Audio**: mp3, wav, flac, m4a, aac, ogg, opus, wma, ape, aiff, wv, mpc, tak, tta, alac, amr, ac3, eac3, dts, dsf, caf, m4b, mka
- **Video**: mp4, m4v, mkv, webm, mov, avi, wmv, 3gp, mpg, mpeg, ts, m2ts, flv, rm, rmvb, asf

Whether a format actually plays depends on the decoders compiled into your ffmpeg build; minimal builds drop the rarer ones.

### How playback works

1. The front end hands a file path to Rust through Tauri commands
2. Rust decides whether the container is natively playable in the WebView
   - yes → stream it straight from the loopback HTTP server
   - no → remux (video) or transcode to WAV (audio) with ffmpeg, then stream
3. The loopback server binds `127.0.0.1` only and honours Range requests, so seeking works

### Known limitations

- Playback only, no transcoding/export
- No subtitle track selection
- The playlist is not persisted across restarts
- macOS builds are untested

## 🇯🇵 日本語

VinePalma Player は **Tauri 2 + Rust** 製のデスクトッププレイヤーです。フロントエンドは素の HTML / CSS / JavaScript でバンドラなし、Rust 側がデコードの振り分け・ローカル配信・メタデータ取得を担当します。メディアファイルは**ローカルで読み込むだけ**で、送信はしません。

> 状態：**0.2.0、開発中**。音声再生と一般的な動画コンテナは動作しますが、UI は調整中です。

### 必要なもの

| 依存 | バージョン | 備考 |
|---|---|---|
| Rust | ≥ 1.77 | Tauri 2 の要件 |
| Node.js | ≥ 18 | `cargo tauri` の npm スクリプト用 |
| Tauri CLI | 2.x | `cargo install tauri-cli --version "^2"` |
| FFmpeg ビルド環境 | 下記 | リポジトリに ffmpeg バイナリは**含まれません** |

### 最初に FFmpeg をビルド

WebView がネイティブに扱えない形式は、同梱の静的 `ffmpeg` / `ffprobe` に依存します。GPL のビルド成果物を git 履歴に入れないため、これらは**リポジトリに含めていません**：

```bash
npm run build:ffmpeg:windows   # Windows（Git Bash / MSYS2 内で実行）
npm run build:ffmpeg:linux     # Linux
```

出力は `src-tauri/binaries/<name>-<target-triple>` になります。初回は FFmpeg と x264 のソース取得＋コンパイルで時間がかかります。

> 既定で `--enable-gpl --enable-libx264` を有効にします。GPL を避けたい場合は `ENABLE_X264=0` で再ビルドしてください（H.264 エンコードは不可）。

### ビルドと実行

```bash
cargo tauri dev
cargo tauri build
npm run build:windows
npm run build:linux
```

### 機能

- **透明・枠なしウィンドウ** — ドラッグ可、最前面固定、サイズ記憶
- **3 種類の全画面** — ウィンドウ全画面／コンテンツ全画面
- **ミニプレイヤー** — 小さな常駐ウィンドウに折りたたみ
- **プレイリスト** — ファイルまたはフォルダ単位で読み込み、リストループ／1 曲リピート／シャッフル
- **動画** — ネイティブ対応コンテナはそのまま、それ以外は ffmpeg で再ラップ
- **音声** — WebView が再生できるものはそのまま、できないものは ffmpeg → WAV にフォールバック
- **カバーとタグ** — ffprobe で title / artist / album を取得しキャッシュ
- **テーマ** — 既定、月別プリセット、カスタム配色

### 対応形式

- **音声**：mp3、wav、flac、m4a、aac、ogg、opus、wma、ape、aiff、wv、mpc、tak、tta、alac、amr、ac3、eac3、dts、dsf、caf、m4b、mka
- **動画**：mp4、m4v、mkv、webm、mov、avi、wmv、3gp、mpg、mpeg、ts、m2ts、flv、rm、rmvb、asf

実際に再生できるかは、ビルドした ffmpeg に含まれるデコーダ次第です。

### 既知の制限

- 再生のみ（変換・書き出しなし）
- 字幕トラックの選択は未対応
- プレイリストは永続化されません
- macOS ビルドは未検証

---

## 许可 / License

代码以 [MIT](./LICENSE) 发布。随应用分发的 FFmpeg / x264 二进制适用各自的许可证，详见 [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)。

Code is released under the [MIT License](./LICENSE). The FFmpeg / x264 binaries you build and ship alongside the app are covered by their own licenses — see [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md).

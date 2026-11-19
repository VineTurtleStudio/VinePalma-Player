# 第三方组件 / Third-party notices

本仓库的**源代码**以 MIT 发布（见 [LICENSE](./LICENSE)）。但应用在运行时会调用
FFmpeg 等可执行文件，这些组件由**使用者本地构建**（或按 CI 中的脚本获取），
适用各自的许可证。仓库本身不包含这些二进制文件。

The **source code** in this repository is MIT-licensed (see [LICENSE](./LICENSE)).
At runtime the app invokes FFmpeg and other executables. Those are built **locally
by you** (or fetched per the CI scripts), carry their own licenses, and are not
committed to this repository.

---

## FFmpeg

- 上游 / Upstream: <https://ffmpeg.org/>
- 源码 / Source: <https://ffmpeg.org/download.html> ·
  <https://git.ffmpeg.org/ffmpeg.git>
- 许可证 / License: LGPL-2.1-or-later；启用 GPL 组件后整体为 GPL-2.0-or-later（见下）
- 构建脚本 / Build scripts: [`scripts/build-ffmpeg-windows.sh`](./scripts/build-ffmpeg-windows.sh)、
  [`scripts/build-ffmpeg-linux.sh`](./scripts/build-ffmpeg-linux.sh)、
  [`scripts/build-ffmpeg-macos.sh`](./scripts/build-ffmpeg-macos.sh)

脚本默认使用 `--enable-gpl --enable-libx264`，因此产出的二进制是 **GPL** 版本。
若以 GPL 版本分发应用，需要同时提供对应源码（或有效的书面提供承诺）。本项目采取的做法是：

1. 构建脚本随仓库公开，可复现同样的二进制；
2. 二进制本身不进入版本管理；
3. 分发时请附上本文件与 FFmpeg 的许可证副本。

The scripts default to `--enable-gpl --enable-libx264`, so the resulting binaries are
**GPL**. If you distribute an app bundled with them, you must also make the
corresponding source available (or provide a valid written offer). This project keeps
the build scripts public and reproducible, keeps the binaries out of version control,
and asks distributors to ship this file together with the FFmpeg license text.

### 不使用 GPL 组件 / Avoiding GPL

```bash
ENABLE_X264=0 npm run build:ffmpeg:windows   # 或 :linux / :macos
```

设置后不启用 libx264，产出为 LGPL 构建，代价是无法编码 H.264（解码不受影响）。
注意 dav1d（AV1 解码器，BSD 许可）不需要 GPL，单独启用它不会改变许可证。

---

## x264

- 上游 / Upstream: <https://www.videolan.org/developers/x264.html>
- 许可证 / License: GPL-2.0-or-later

仅在 `ENABLE_X264=1`（默认）时引入。分发含 x264 的构建与分发 GPL 版 FFmpeg
义务相同：提供对应源码或书面提供承诺，并附 GPL-2.0 许可证文本。
Only pulled in when `ENABLE_X264=1` (the default). Distributing a build that
bundles x264 carries the same obligations as distributing GPL FFmpeg: offer the
corresponding source (or a written offer) and include the GPL-2.0 license text.

## dav1d（可选）

- 上游 / Upstream: <https://code.videolan.org/videolan/dav1d>
- 许可证 / License: BSD-2-Clause
- 仅 Linux/macOS 脚本在工具链可用时引入（`ENABLE_DAV1D=1`，默认）。分发时
  需附带其 BSD-2-Clause 许可证文本。

## zlib

- 上游 / Upstream: <https://zlib.net/>
- 许可证 / License: zlib License（允许静态链接与商业分发，需附许可证文本）

---

## 运行时分发组件 / Runtime distribution components

按平台打包后的安装包还会包含以下分发组件：

| 组件 | 平台 | 许可证 / 说明 |
|---|---|---|
| WebView2 Runtime | Windows | 由 NSIS 安装器引导下载/随系统提供，微软专用许可 |
| NSIS 安装器 | Windows | CPL-1.0 / zlib-libpng 混合许可（安装器本身不随应用再分发修改版时无需附加） |
| libwebkit2gtk / GTK | Linux | 通过 `deb.depends` 由系统包管理器安装，LGPL-2.1+ |
| WKWebView | macOS | 系统组件，随 macOS 提供 |

## Rust / JavaScript 依赖

Rust 依赖以 `src-tauri/Cargo.lock` 为准（tauri、tiny_http、base64、
tauri-plugin-dialog、tauri-plugin-single-instance 等），各自遵循其仓库声明的
许可证（以 MIT / Apache-2.0 双许可为主）。`package.json` 当前**没有** npm
依赖（仅脚本），如未来加入，以其各自许可证为准。

运行 `cargo metadata` / `npm ls` 可列出完整清单。
Rust dependencies are pinned in `src-tauri/Cargo.lock` and keep their own
licenses (mostly MIT / Apache-2.0 dual). `package.json` currently has **no** npm
dependencies (scripts only). Run `cargo metadata` / `npm ls` to enumerate them.

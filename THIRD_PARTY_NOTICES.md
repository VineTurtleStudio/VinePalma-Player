# 第三方组件 / Third-party notices

本仓库的**源代码**以 MIT 发布（见 [LICENSE](./LICENSE)）。但应用在运行时会调用
FFmpeg 与 x264 的可执行文件，这些组件由**使用者本地构建**，适用各自的许可证。
仓库本身不包含这些二进制文件。

The **source code** in this repository is MIT-licensed (see [LICENSE](./LICENSE)).
At runtime the app invokes FFmpeg and x264 executables. Those are built **locally by
you**, carry their own licenses, and are not committed to this repository.

---

## FFmpeg

- 上游 / Upstream: <https://ffmpeg.org/>
- 源码 / Source: <https://ffmpeg.org/download.html> ·
  <https://git.ffmpeg.org/ffmpeg.git>
- 构建脚本 / Build scripts: [`scripts/build-ffmpeg-windows.sh`](./scripts/build-ffmpeg-windows.sh)、
  [`scripts/build-ffmpeg-linux.sh`](./scripts/build-ffmpeg-linux.sh)

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
ENABLE_X264=0 npm run build:ffmpeg:windows
```

设置后不启用 libx264，产出为 LGPL 构建，代价是无法编码 H.264（解码不受影响）。

---

## x264

- 上游 / Upstream: <https://www.videolan.org/developers/x264.html>
- 许可证 / License: GPL-2.0-or-later

仅在 `ENABLE_X264=1`（默认）时引入。Only pulled in when `ENABLE_X264=1` (the default).

---

## Rust / JavaScript 依赖

`src-tauri/Cargo.lock` 与 `package.json` 中的依赖遵循各自仓库的许可证。
运行 `cargo metadata` 或 `npm ls` 可列出完整清单。

Dependencies listed in `src-tauri/Cargo.lock` and `package.json` keep their own
licenses; `cargo metadata` / `npm ls` enumerate them.

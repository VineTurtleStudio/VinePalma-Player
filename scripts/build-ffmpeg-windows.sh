#!/usr/bin/env bash
# Build a minimal static FFmpeg (ffmpeg + ffprobe) for Windows x86_64,
# cross-compiled from Linux with mingw-w64. Same trimmed component set as the
# Linux build, so the bundled sidecars stay small.
#
# 注意：这是 **Linux 上交叉编译 Windows 版** 的脚本（需要
# x86_64-w64-mingw32-gcc）。在 Windows 本机想快速拿到 sidecar，可以直接
# 下载 BtbN/FFmpeg-Builds 或 gyan.dev 的预编译 ffmpeg.exe/ffprobe.exe，
# 重命名为 src-tauri/binaries/ffmpeg-x86_64-pc-windows-gnu.exe（ffprobe 同理）。
#
# Optional: set ENABLE_X264=0 to skip the H.264 encoder. It is ON by default
# because the re-encode fallback (remux_video) must produce H.264: Chromium /
# WebView2 (Windows Edge) cannot decode MPEG-4 Part 2 (`mpeg4`), so without
# libx264 the fallback would only ever make "video shows no picture" on Windows.
#
# 产物名保留 `-x86_64-pc-windows-gnu` 后缀：sidecar 是独立进程，与宿主应用
# 的构建工具链（MSVC/GNU）无关，命名只反映二进制本身的来源（mingw 交叉编译）。
set -euo pipefail

FFMPEG_VERSION="${FFMPEG_VERSION:-8.1.2}"
BUILD_DIR="${BUILD_DIR:-${TMPDIR:-/tmp}/vinepalma-ffmpeg-build}"
OUT_DIR="${OUT_DIR:-$(cd "$(dirname "$0")/.." && pwd)/src-tauri/binaries}"
JOBS="${JOBS:-$(nproc)}"
# ENABLE_X264=0 关闭 H.264 编码（兼容旧的 FFMPEG_NO_X264=1 写法）
if [ "${FFMPEG_NO_X264:-0}" = "1" ]; then ENABLE_X264="${ENABLE_X264:-0}"; fi
ENABLE_X264="${ENABLE_X264:-1}"
SRC_TARBALL="${SRC_TARBALL:-$BUILD_DIR/ffmpeg-$FFMPEG_VERSION.tar.xz}"
BUILD_WIN_DIR="$BUILD_DIR/ffmpeg-win-$FFMPEG_VERSION"

if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
  echo "[ffmpeg-win] 缺少 x86_64-w64-mingw32-gcc，请先安装 mingw-w64" >&2
  echo "[ffmpeg-win] （Debian/Ubuntu: apt install mingw-w64）" >&2
  exit 1
fi

mkdir -p "$OUT_DIR" "$BUILD_DIR"

# Extract a fresh copy of the source for the Windows build.
if [ ! -d "$BUILD_WIN_DIR" ]; then
  if [ ! -f "$SRC_TARBALL" ]; then
    echo "[ffmpeg-win] downloading source ffmpeg-$FFMPEG_VERSION ..."
    # 官方源优先，Debian 源回退
    curl -fL --retry 2 -o "$SRC_TARBALL" \
      "https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz" \
      || curl -fL --retry 2 -o "$SRC_TARBALL" \
      "https://deb.debian.org/debian/pool/main/f/ffmpeg/ffmpeg_${FFMPEG_VERSION}.orig.tar.xz"
  fi
  mkdir -p "$BUILD_WIN_DIR"
  tar -C "$BUILD_WIN_DIR" --strip-components=1 -xf "$SRC_TARBALL"
fi

cd "$BUILD_WIN_DIR"

# ---- optional external libraries -------------------------------------------
EXTRA_OPTIONS=()
EXTRA_CFLAGS=()
EXTRA_LDFLAGS=(-static)
EXTRA_LIBS=()
ZLIB_FLAG="--enable-zlib"

fetch_git() { # $1=url $2=dir
  local url="$1" dir="$2"
  if [ -d "$dir" ]; then return 0; fi
  git clone --depth 1 "$url" "$dir" >/dev/null 2>&1 || true
  if [ -d "$dir" ]; then return 0; fi
  return 1
}

# zlib（跨平台容器支持）：自行交叉编译静态库；失败则退回 --disable-zlib。
ZLIB_PREFIX="$BUILD_DIR/zlib-win-install"
if [ ! -f "$ZLIB_PREFIX/lib/libz.a" ]; then
  echo "[ffmpeg-win] fetching zlib source ..."
  if [ ! -d "$BUILD_DIR/zlib" ]; then
    curl -fL --retry 2 -o "$BUILD_DIR/zlib.tar.gz" \
      "https://github.com/madler/zlib/releases/download/v1.3.1/zlib-1.3.1.tar.gz" \
      || curl -fL --retry 2 -o "$BUILD_DIR/zlib.tar.gz" \
      "https://zlib.net/zlib-1.3.1.tar.gz" || true
    if [ -f "$BUILD_DIR/zlib.tar.gz" ]; then
      mkdir -p "$BUILD_DIR/zlib"
      tar -C "$BUILD_DIR/zlib" --strip-components=1 -xzf "$BUILD_DIR/zlib.tar.gz" || true
    fi
  fi
  if [ -f "$BUILD_DIR/zlib/configure" ]; then
    echo "[ffmpeg-win] building zlib (static, mingw cross) ..."
    if ( cd "$BUILD_DIR/zlib" && \
         make distclean >/dev/null 2>&1 || true; \
         CC=x86_64-w64-mingw32-gcc \
         AR=x86_64-w64-mingw32-ar \
         RANLIB=x86_64-w64-mingw32-ranlib \
         STRIP=x86_64-w64-mingw32-strip \
         ./configure --prefix="$ZLIB_PREFIX" --static >/dev/null 2>&1 && \
         make -j"$JOBS" >/dev/null 2>&1 && \
         make install >/dev/null 2>&1 ); then
      echo "[ffmpeg-win] zlib built -> $ZLIB_PREFIX"
    else
      echo "[ffmpeg-win] 警告: zlib 构建失败，将使用 --disable-zlib"
    fi
  else
    echo "[ffmpeg-win] 警告: 无法获取 zlib 源码，将使用 --disable-zlib"
  fi
fi
if [ -f "$ZLIB_PREFIX/lib/libz.a" ]; then
  EXTRA_CFLAGS+=("-I$ZLIB_PREFIX/include")
  EXTRA_LDFLAGS+=("-L$ZLIB_PREFIX/lib")
  EXTRA_LIBS+=(-lz)
else
  ZLIB_FLAG="--disable-zlib"
fi

# x264（可选）：H.264 编码器，重编码回退必需。
X264_PREFIX="$BUILD_DIR/x264-win-install"
if [ "$ENABLE_X264" = "1" ] && [ ! -f "$X264_PREFIX/lib/libx264.a" ]; then
  echo "[ffmpeg-win] fetching x264 source ..."
  fetch_git "https://code.videolan.org/videolan/x264.git" "$BUILD_DIR/x264" \
    || fetch_git "https://github.com/mirror/x264.git" "$BUILD_DIR/x264" || true
  if [ -f "$BUILD_DIR/x264/configure" ]; then
    echo "[ffmpeg-win] building x264 (static, single-threaded) ..."
    if ( cd "$BUILD_DIR/x264" && \
         make distclean >/dev/null 2>&1 || true; \
         ./configure --host=x86_64-w64-mingw32 --cross-prefix=x86_64-w64-mingw32- \
                     --prefix="$X264_PREFIX" --enable-static --disable-cli \
                     --disable-shared --disable-asm --disable-thread >/dev/null 2>&1 && \
         make -j"$JOBS" >/dev/null 2>&1 && \
         make install >/dev/null 2>&1 ); then
      echo "[ffmpeg-win] x264 built -> $X264_PREFIX"
    else
      echo "[ffmpeg-win] 警告: x264 构建失败，重编码回退将使用内置 MPEG-4 编码器"
    fi
  else
    echo "[ffmpeg-win] 警告: 无法获取 x264 源码，重编码回退将使用内置 MPEG-4 编码器"
  fi
fi
if [ -f "$X264_PREFIX/lib/libx264.a" ]; then
  # 只有 x264 是 GPL，zlib/dav1d 都不是
  EXTRA_OPTIONS+=(--enable-gpl --enable-libx264)
  EXTRA_CFLAGS+=("-I$X264_PREFIX/include")
  EXTRA_LDFLAGS+=("-L$X264_PREFIX/lib")
  EXTRA_LIBS+=(-lx264)
fi

echo "[ffmpeg-win] configuring minimal Windows build ..."
make distclean >/dev/null 2>&1 || true
./configure \
  --prefix="$OUT_DIR" \
  --enable-cross-compile \
  --cross-prefix=x86_64-w64-mingw32- \
  --target-os=mingw32 \
  --arch=x86_64 \
  --disable-everything \
  --disable-doc \
  --disable-debug \
  --disable-network \
  --disable-avdevice \
  --enable-swscale \
  --disable-shared \
  --enable-static \
  --enable-w32threads \
  --disable-pthreads \
  "$ZLIB_FLAG" \
  --enable-ffmpeg \
  --enable-ffprobe \
  --disable-ffplay \
  --disable-autodetect \
  --disable-x86asm \
  "${EXTRA_OPTIONS[@]}" \
  --enable-protocol=file,pipe \
  --enable-avcodec \
  --enable-avformat \
  --enable-avutil \
  --enable-avfilter \
  --enable-swresample \
  --enable-filter=anull,aresample,aformat,scale,format \
  --enable-parser=mpegaudio,aac,ac3,dca,flac,vorbis,opus,alac,truehd,mlp,tak,h264,hevc,mpeg4video \
  --enable-demuxer=mov,matroska,mp3,wav,flac,ogg,aac,aiff,asf,wv,ape,mpc,tak,tta,caf,w64,au,ac3,eac3,dts,avi,dsf,mpegts,flv,mpegvideo,rm,ivf \
  --enable-decoder=mp3,mp1,mp2,aac,aac_latm,flac,vorbis,opus,ac3,eac3,dca,alac,ape,wavpack,wmav1,wmav2,wmapro,wmalossless,wmavoice,amrnb,amrwb,cook,tak,tta,truehd,mpc7,mpc8,atrac3,gsm,gsm_ms,als,shorten,binkaudio_dct,binkaudio_rdft,qdm2,real_144 \
  --enable-decoder=pcm_s16le,pcm_s16be,pcm_s24le,pcm_s32le,pcm_f32le,pcm_f64le,pcm_u8,pcm_u16le,pcm_u24le,pcm_u32le,pcm_alaw,pcm_mulaw,pcm_s16le_planar,pcm_s24le_planar,pcm_s32le_planar,pcm_f32le_planar,pcm_f64le_planar \
  --enable-decoder=h264,hevc,vp8,vp9,mpeg4,mjpeg,png,bmp \
  --enable-decoder=srt,ass,movtext,webvtt \
  --enable-encoder=pcm_s16le,mjpeg,aac,mpeg4,webvtt \
  --enable-muxer=wav,image2,mp4,mov,webm,matroska,mpegts,adts,mp3,ogg,webvtt \
  --extra-cflags="${EXTRA_CFLAGS[*]}" \
  --extra-ldflags="${EXTRA_LDFLAGS[*]}" \
  --extra-libs="${EXTRA_LIBS[*]}"

echo "[ffmpeg-win] compiling with $JOBS jobs ..."
make -j"$JOBS"

echo "[ffmpeg-win] installing binaries ..."
cp -f ffmpeg.exe "$OUT_DIR/ffmpeg-x86_64-pc-windows-gnu.exe"
cp -f ffprobe.exe "$OUT_DIR/ffprobe-x86_64-pc-windows-gnu.exe"

ls -lh "$OUT_DIR"
echo "[ffmpeg-win] done."

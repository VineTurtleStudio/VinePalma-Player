#!/usr/bin/env bash
# Build a minimal static FFmpeg (ffmpeg + ffprobe) for Linux x86_64.
# Only the codecs/containers needed by a local player are compiled in,
# which keeps the bundled binaries small (a fraction of a full FFmpeg build).
#
# Optional external libraries are built when the required tooling is present
# and skipped (with a warning) otherwise:
#   - x264   -> H.264 encoder, used by the remux re-encode fallback
#   - dav1d  -> AV1 decoder
# Set FFMPEG_NO_X264=1 / FFMPEG_NO_DAV1D=1 to skip them explicitly.
set -euo pipefail

FFMPEG_VERSION="${FFMPEG_VERSION:-8.1.2}"
BUILD_DIR="${BUILD_DIR:-/tmp/opencode/ffmpeg-build}"
OUT_DIR="${OUT_DIR:-$(cd "$(dirname "$0")/.." && pwd)/src-tauri/binaries}"
JOBS="${JOBS:-$(nproc)}"
# If the tarball is already present (e.g. downloaded from a mirror), skip re-downloading.
SRC_TARBALL="${SRC_TARBALL:-$BUILD_DIR/ffmpeg-$FFMPEG_VERSION.tar.xz}"

mkdir -p "$BUILD_DIR" "$OUT_DIR"

if [ ! -d "$BUILD_DIR/ffmpeg-$FFMPEG_VERSION" ]; then
  if [ ! -f "$SRC_TARBALL" ]; then
    echo "[ffmpeg] downloading source ffmpeg-$FFMPEG_VERSION ..."
    curl -fL -o "$SRC_TARBALL" \
      "https://deb.debian.org/debian/pool/main/f/ffmpeg/ffmpeg_${FFMPEG_VERSION}.orig.tar.xz"
  fi
  tar -C "$BUILD_DIR" -xf "$SRC_TARBALL"
fi

cd "$BUILD_DIR/ffmpeg-$FFMPEG_VERSION"

# ---- optional external libraries -------------------------------------------
EXTRA_OPTIONS=()   # ./configure feature flags (--enable-gpl, --enable-libx264, ...)
EXTRA_CFLAGS=()
EXTRA_LDFLAGS=()
EXTRA_LIBS=()
HAS_OPTIONAL=0

fetch_git() { # $1=url $2=dir ; tries git clone, falls back to a second URL set by caller
  local url="$1" dir="$2"
  if [ -d "$dir" ]; then return 0; fi
  git clone --depth 1 "$url" "$dir" >/dev/null 2>&1 || true
  if [ -d "$dir" ]; then return 0; fi
  return 1
}

# x264: H.264 encoder (used by remux re-encode fallback).
X264_PREFIX="$BUILD_DIR/x264-install"
if [ "${FFMPEG_NO_X264:-0}" != "1" ] && [ ! -f "$X264_PREFIX/lib/libx264.a" ]; then
  echo "[ffmpeg] fetching x264 source ..."
  fetch_git "https://code.videolan.org/videolan/x264.git" "$BUILD_DIR/x264" \
    || fetch_git "https://github.com/mirror/x264.git" "$BUILD_DIR/x264" || true
  if [ -f "$BUILD_DIR/x264/configure" ]; then
    echo "[ffmpeg] building x264 (static) ..."
    if ( cd "$BUILD_DIR/x264" && \
         make distclean >/dev/null 2>&1 || true; \
         ./configure --prefix="$X264_PREFIX" --enable-static --disable-cli \
                     --disable-shared --enable-pic >/dev/null 2>&1 && \
         make -j"$JOBS" >/dev/null 2>&1 && \
         make install >/dev/null 2>&1 ); then
      echo "[ffmpeg] x264 built -> $X264_PREFIX"
    else
      echo "[ffmpeg] 警告: x264 构建失败，重编码回退将使用内置 MPEG-4 编码器"
    fi
  else
    echo "[ffmpeg] 警告: 无法获取 x264 源码，重编码回退将使用内置 MPEG-4 编码器"
  fi
fi
if [ -f "$X264_PREFIX/lib/libx264.a" ]; then
  EXTRA_OPTIONS+=(--enable-gpl --enable-libx264)
  EXTRA_CFLAGS+=("-I$X264_PREFIX/include")
  EXTRA_LDFLAGS+=("-L$X264_PREFIX/lib")
  EXTRA_LIBS+=(-lx264)
  HAS_OPTIONAL=1
fi

# dav1d: AV1 decoder.
DAV1D_PREFIX="$BUILD_DIR/dav1d-install"
if [ "${FFMPEG_NO_DAV1D:-0}" != "1" ] && [ ! -f "$DAV1D_PREFIX/lib/libdav1d.a" ]; then
  if command -v meson >/dev/null 2>&1 && command -v ninja >/dev/null 2>&1; then
    echo "[ffmpeg] fetching dav1d source ..."
    fetch_git "https://code.videolan.org/videolan/dav1d.git" "$BUILD_DIR/dav1d" \
      || fetch_git "https://github.com/videolan/dav1d.git" "$BUILD_DIR/dav1d" || true
    if [ -d "$BUILD_DIR/dav1d" ]; then
      echo "[ffmpeg] building dav1d (static) ..."
      if ( cd "$BUILD_DIR/dav1d" && \
           meson setup build --prefix "$DAV1D_PREFIX" --libdir lib \
                             --default-library static >/dev/null 2>&1 && \
           ninja -C build >/dev/null 2>&1 && \
           ninja -C build install >/dev/null 2>&1 ); then
        echo "[ffmpeg] dav1d built -> $DAV1D_PREFIX"
      else
        echo "[ffmpeg] 警告: dav1d 构建失败，跳过 AV1 解码"
      fi
    else
      echo "[ffmpeg] 警告: 无法获取 dav1d 源码，跳过 AV1 解码"
    fi
  else
    echo "[ffmpeg] 提示: 未安装 meson/ninja，跳过 AV1 (dav1d) 解码"
  fi
fi
if [ -f "$DAV1D_PREFIX/lib/libdav1d.a" ]; then
  EXTRA_OPTIONS+=(--enable-gpl --enable-libdav1d)
  EXTRA_CFLAGS+=("-I$DAV1D_PREFIX/include")
  EXTRA_LDFLAGS+=("-L$DAV1D_PREFIX/lib")
  EXTRA_LIBS+=(-ldav1d)
  HAS_OPTIONAL=1
fi
if [ "$HAS_OPTIONAL" = "1" ]; then
  EXTRA_LIBS+=(-pthread -lm)
fi

run_configure() {
  make distclean >/dev/null 2>&1 || true
  ./configure \
    --prefix="$OUT_DIR" \
    --disable-everything \
    --disable-doc \
    --disable-debug \
    --disable-network \
    --disable-avdevice \
    --enable-swscale \
    --disable-shared \
    --enable-static \
    --enable-pthreads \
    --enable-zlib \
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
}

if [ "${SKIP_CONFIG:-0}" != "1" ]; then
  echo "[ffmpeg] configuring minimal build ..."
  if ! run_configure; then
    if [ "$HAS_OPTIONAL" = "1" ]; then
      echo "[ffmpeg] 警告: 带可选库 configure 失败，回退为纯内置 codec 再试 ..."
      EXTRA_OPTIONS=(); EXTRA_CFLAGS=(); EXTRA_LDFLAGS=(); EXTRA_LIBS=()
      run_configure
    else
      exit 1
    fi
  fi
fi

echo "[ffmpeg] compiling with $JOBS jobs ..."
make -j"$JOBS"

echo "[ffmpeg] installing binaries ..."
cp -f ffmpeg "$OUT_DIR/ffmpeg-x86_64-unknown-linux-gnu"
cp -f ffprobe "$OUT_DIR/ffprobe-x86_64-unknown-linux-gnu"
chmod +x "$OUT_DIR/ffmpeg-x86_64-unknown-linux-gnu" "$OUT_DIR/ffprobe-x86_64-unknown-linux-gnu"

ls -lh "$OUT_DIR"
echo "[ffmpeg] done."

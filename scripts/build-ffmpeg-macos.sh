#!/usr/bin/env bash
# Build a minimal static FFmpeg (ffmpeg + ffprobe) for macOS, targeting the
# host architecture (arm64 on Apple Silicon, x86_64 on Intel).
#
# macOS 的 WKWebView 原生支持 H.264/HEVC，ffmpeg 只承担容器重封装（stream
# copy）与"无法直出的音频 → WAV"回退，因此不需要 x264 / dav1d，配置最精简。
#
# Usage:
#   bash scripts/build-ffmpeg-macos.sh            # 本机架构
#   FFMPEG_VERSION=8.1.2 bash scripts/...         # 指定版本
set -euo pipefail

FFMPEG_VERSION="${FFMPEG_VERSION:-8.1.2}"
ARCH="$(uname -m)"
case "$ARCH" in
  arm64)  TRIPLE="aarch64-apple-darwin" ;;
  x86_64) TRIPLE="x86_64-apple-darwin" ;;
  *) echo "[ffmpeg-mac] 不支持的架构: $ARCH" >&2; exit 1 ;;
esac

BUILD_DIR="${BUILD_DIR:-${TMPDIR:-/tmp}/vinepalma-ffmpeg-build}"
OUT_DIR="${OUT_DIR:-$(cd "$(dirname "$0")/.." && pwd)/src-tauri/binaries}"
JOBS="${JOBS:-$(sysctl -n hw.ncpu 2>/dev/null || echo 4)}"
SRC_TARBALL="${SRC_TARBALL:-$BUILD_DIR/ffmpeg-$FFMPEG_VERSION.tar.xz}"

mkdir -p "$BUILD_DIR" "$OUT_DIR"

if [ ! -d "$BUILD_DIR/ffmpeg-$FFMPEG_VERSION" ]; then
  if [ ! -f "$SRC_TARBALL" ]; then
    echo "[ffmpeg-mac] downloading source ffmpeg-$FFMPEG_VERSION ..."
    curl -fL --retry 2 -o "$SRC_TARBALL" \
      "https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz"
  fi
  tar -C "$BUILD_DIR" -xf "$SRC_TARBALL"
fi

cd "$BUILD_DIR/ffmpeg-$FFMPEG_VERSION"

echo "[ffmpeg-mac] configuring minimal $TRIPLE build ..."
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
  --enable-muxer=wav,image2,mp4,mov,webm,matroska,mpegts,adts,mp3,ogg,webvtt

echo "[ffmpeg-mac] compiling with $JOBS jobs ..."
make -j"$JOBS"

echo "[ffmpeg-mac] installing binaries ..."
cp -f ffmpeg "$OUT_DIR/ffmpeg-$TRIPLE"
cp -f ffprobe "$OUT_DIR/ffprobe-$TRIPLE"
chmod +x "$OUT_DIR/ffmpeg-$TRIPLE" "$OUT_DIR/ffprobe-$TRIPLE"

ls -lh "$OUT_DIR"
echo "[ffmpeg-mac] done."

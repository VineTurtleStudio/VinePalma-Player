#!/usr/bin/env bash
# Build a minimal static FFmpeg (ffmpeg + ffprobe) for Windows x86_64,
# cross-compiled from Linux with mingw-w64. Same trimmed component set as the
# Linux build, so the bundled sidecars stay small.
#
# Optional: set ENABLE_X264=0 to skip the H.264 encoder. It is ON by default
# because the re-encode fallback (remux_video) must produce H.264: Chromium /
# WebView2 (Windows Edge) cannot decode MPEG-4 Part 2 (`mpeg4`), so without
# libx264 the fallback would only ever make "video shows no picture" on Windows.
set -euo pipefail

FFMPEG_VERSION="${FFMPEG_VERSION:-8.1.2}"
BUILD_DIR="${BUILD_DIR:-/tmp/opencode/ffmpeg-build}"
OUT_DIR="${OUT_DIR:-$(cd "$(dirname "$0")/.." && pwd)/src-tauri/binaries}"
JOBS="${JOBS:-$(nproc)}"
ENABLE_X264="${ENABLE_X264:-1}"
SRC_TARBALL="${SRC_TARBALL:-$BUILD_DIR/ffmpeg-$FFMPEG_VERSION.tar.xz}"
BUILD_WIN_DIR="$BUILD_DIR/ffmpeg-win-$FFMPEG_VERSION"

if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
  echo "[ffmpeg-win] 缺少 x86_64-w64-mingw32-gcc，请先安装 mingw-w64" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"

# Extract a fresh copy of the source for the Windows build.
if [ ! -d "$BUILD_WIN_DIR" ]; then
  if [ ! -f "$SRC_TARBALL" ]; then
    echo "[ffmpeg-win] downloading source ffmpeg-$FFMPEG_VERSION ..."
    curl -fL -o "$SRC_TARBALL" \
      "https://deb.debian.org/debian/pool/main/f/ffmpeg/ffmpeg_${FFMPEG_VERSION}.orig.tar.xz"
  fi
  mkdir -p "$BUILD_WIN_DIR"
  tar -C "$BUILD_WIN_DIR" --strip-components=1 -xf "$SRC_TARBALL"
fi

cd "$BUILD_WIN_DIR"

# ---- optional x264 (opt-in) ------------------------------------------------
EXTRA_OPTIONS=()
EXTRA_CFLAGS=(-I/tmp/opencode/mingw-zlib/include)
EXTRA_LDFLAGS=(-L/tmp/opencode/mingw-zlib/lib -static)
EXTRA_LIBS=(-lz)

X264_PREFIX="$BUILD_DIR/x264-win-install"
if [ "${ENABLE_X264:-0}" = "1" ] && [ ! -f "$X264_PREFIX/lib/libx264.a" ]; then
  echo "[ffmpeg-win] fetching x264 source ..."
  if [ ! -d "$BUILD_DIR/x264" ]; then
    git clone --depth 1 https://code.videolan.org/videolan/x264.git "$BUILD_DIR/x264" >/dev/null 2>&1 || true
  fi
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
  fi
fi
if [ -f "$X264_PREFIX/lib/libx264.a" ]; then
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

echo "[ffmpeg-win] compiling with $JOBS jobs ..."
make -j"$JOBS"

echo "[ffmpeg-win] installing binaries ..."
cp -f ffmpeg.exe "$OUT_DIR/ffmpeg-x86_64-pc-windows-gnu.exe"
cp -f ffprobe.exe "$OUT_DIR/ffprobe-x86_64-pc-windows-gnu.exe"

ls -lh "$OUT_DIR"
echo "[ffmpeg-win] done."

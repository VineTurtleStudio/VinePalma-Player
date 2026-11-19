//! Media probing, decoding, remuxing and cover-art extraction, built on a
//! minimal static FFmpeg (ffprobe + ffmpeg) shipped inside the app's resource
//! directory (a `.deb` install therefore keeps them out of `/usr/bin`).
//!
//! Only the formats/codecs needed by a local player are compiled into the
//! bundled ffmpeg, which is what keeps the installers small.
//!
//! Non-UTF8 (e.g. GBK-encoded) file names are supported by decoding through a
//! temporary symlink whose name is always valid ASCII: the sidecar CLI only
//! accepts `&str` arguments, so the mangled lossy string would otherwise make
//! ffmpeg fail to open the file.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Manager};

/// Every container/codec a local player is likely to meet.
pub const MEDIA_EXTS: &[&str] = &[
    // lossy & lossless audio
    "mp3", "wav", "flac", "m4a", "aac", "ogg", "oga", "opus", "wma", "ape", "aiff", "aif", "wv",
    "mpc", "tak", "tta", "alac", "amr", "au", "ac3", "eac3", "dts", "dtshd", "dsf", "caf", "m4b",
    "mka", // video containers
    "mp4", "m4v", "mkv", "webm", "mov", "avi", "wmv", "3gp", "3g2", "mpg", "mpeg", "ts", "m2ts",
    "flv", "rm", "rmvb", "asf",
];

/// Extensions used in the native "open file" dialog filter.
pub const DIALOG_EXTS: &[&str] = &[
    "mp3", "wav", "flac", "m4a", "aac", "ogg", "oga", "opus", "wma", "ape", "aiff", "aif", "wv",
    "mpc", "tak", "tta", "alac", "amr", "au", "ac3", "eac3", "dts", "dsf", "caf", "m4b", "mka",
    "mp4", "m4v", "mkv", "webm", "mov", "avi", "wmv", "3gp", "mpg", "mpeg", "ts", "m2ts", "flv",
    "rm", "rmvb", "asf",
];

/// Containers the webview's `<video>` element can play natively (served as-is).
const NATIVE_VIDEO_CONTAINERS: &[&str] = &["mp4", "m4v", "mov", "webm"];

/// True when an MP4-family file has its `moov` (index) atom before the media
/// data, so the browser can seek immediately. Videos whose index sits at the
/// end (the common non-faststart case) are remuxed so seeking works.
fn has_faststart(p: &Path) -> bool {
    use std::fs::File;
    use std::io::Read;
    let mut f = match File::open(p) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut buf = vec![0u8; 1_000_000];
    let n = f.read(&mut buf).unwrap_or(0);
    let bytes = &buf[..n];

    let mut moov: Option<usize> = None;
    let mut mdat: Option<usize> = None;
    for (i, w) in bytes.windows(4).enumerate() {
        if w == b"moov" && moov.is_none() {
            moov = Some(i);
        } else if w == b"mdat" && mdat.is_none() {
            mdat = Some(i);
        }
        if moov.is_some() && mdat.is_some() {
            break;
        }
    }
    match (moov, mdat) {
        (Some(_), None) => true, // index at the start, data stream beyond the scan window
        (Some(m), Some(d)) => m < d, // index before data -> faststart
        _ => false,              // no index visible (moov at end) -> not faststart
    }
}

/// True when a video file can be served to the browser directly *and* seeking
/// will work. WebM is always seekable; MP4-family files need a faststart index.
pub fn is_native_seekable(p: &Path) -> bool {
    match p.extension().and_then(|e| e.to_str()) {
        Some(e) if e.eq_ignore_ascii_case("webm") => true,
        Some(e)
            if NATIVE_VIDEO_CONTAINERS
                .iter()
                .any(|c| c.eq_ignore_ascii_case(e)) =>
        {
            has_faststart(p)
        }
        _ => false,
    }
}

/// Audio extensions the webview (GStreamer) can decode and seek natively, so
/// we can stream the *original* file instead of paying for a full, slow
/// transcode to an uncompressed WAV (which also makes seeking unreliable).
/// Other audio codecs still fall back to a decoded WAV cache.
///
/// The two platforms use different webview engines, so the set of formats that
/// can be *streamed as-is* differs:
/// - Linux: WebKitGTK + GStreamer (avdec_*) — very broad codec support.
/// - Windows: WebView2 / Chromium — narrower; no AIFF, AC3/EAC3 or Matroska
///   audio. Those are intentionally *omitted* from the Windows native list so
///   they go through the ffmpeg→WAV fallback instead (preserving format parity
///   with Linux, since ffmpeg decodes them identically on both platforms).
#[cfg(not(target_os = "windows"))]
const NATIVE_AUDIO_EXTS: &[&str] = &[
    // lossy / container audio most players meet
    "mp3", "m4a", "aac", "m4b", "mp4", "ogg", "oga", "opus", "flac", "wav", "aiff", "aif", "caf",
    "ac3", "eac3", "alac", "mka",
];

#[cfg(target_os = "windows")]
const NATIVE_AUDIO_EXTS: &[&str] = &[
    // Chromium-native: MP3, AAC, Ogg/Opus, FLAC, WAV, ALAC-in-CAF/MP4.
    // aiff/aif, ac3, eac3, mka 不在此列 → 走 ffmpeg→WAV 回退。
    "mp3", "m4a", "aac", "m4b", "mp4", "ogg", "oga", "opus", "flac", "wav", "caf", "alac",
];

pub fn is_native_audio(p: &Path) -> bool {
    match p.extension().and_then(|e| e.to_str()) {
        Some(ext) => NATIVE_AUDIO_EXTS.contains(&ext.to_lowercase().as_str()),
        None => false,
    }
}

/// Best-effort MIME type for a natively-streamed audio file. The webview mostly
/// sniffs content, so a coarse mapping is fine.
pub fn native_audio_content_type(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()) {
        Some(ext) => match ext.to_lowercase().as_str() {
            "mp3" | "m4a" | "m4b" | "aac" | "mp4" | "mka" => "audio/mpeg",
            "ogg" | "oga" | "opus" => "audio/ogg",
            "flac" => "audio/flac",
            "wav" | "aiff" | "aif" | "caf" => "audio/wav",
            "ac3" | "eac3" => "audio/ac3",
            _ => "audio/mpeg",
        },
        None => "audio/mpeg",
    }
}

pub fn is_media_file(p: &Path) -> bool {
    match p.extension().and_then(|e| e.to_str()) {
        Some(ext) => MEDIA_EXTS.contains(&ext.to_lowercase().as_str()),
        None => false,
    }
}

pub fn is_video_file(p: &Path) -> bool {
    match p.extension().and_then(|e| e.to_str()) {
        Some(ext) => NATIVE_VIDEO_CONTAINERS
            .iter()
            .any(|c| c.eq_ignore_ascii_case(ext)),
        None => false,
    }
}

/// Video codecs Chromium / WebView2 (Windows Edge) can decode in a `<video>`.
/// Linux (WebKitGTK + GStreamer avdec_*) plays almost anything, so the native
/// direct-serve / stream-copy path is fine there; on Windows these are the only
/// codecs that can be served without an H.264 re-encode.
#[cfg(target_os = "windows")]
const CHROMIUM_VIDEO_CODECS: &[&str] = &["h264", "avc1", "vp8", "vp9", "av1"];

/// True when the current platform's webview can natively decode `codec`.
#[cfg(target_os = "windows")]
pub fn webview_can_decode_video(codec: &str) -> bool {
    CHROMIUM_VIDEO_CODECS.contains(&codec)
}

#[cfg(not(target_os = "windows"))]
pub fn webview_can_decode_video(_codec: &str) -> bool {
    true
}

/// Probe the primary video stream's codec name (e.g. "h264", "hevc", "mpeg4").
/// Returns None when probing fails or the file has no real video stream.
pub fn video_codec(app: &AppHandle, cache_dir: &Path, path: &Path) -> Option<String> {
    let link = safe_link(cache_dir, path, "probe").ok()?;
    let s = link.to_str()?;
    let args = ["-v", "quiet", "-print_format", "json", "-show_streams", s];
    let (ok, out, _) = run_sidecar(app, "ffprobe", &args, PROBE_TIMEOUT_SECS).ok()?;
    if !ok {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&out).ok()?;
    json["streams"]
        .as_array()?
        .iter()
        .find(|st| {
            st["codec_type"].as_str() == Some("video")
                && st["disposition"]["attached_pic"].as_i64().unwrap_or(0) != 1
        })
        .and_then(|st| st["codec_name"].as_str().map(String::from))
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackInfo {
    pub path: String,
    pub name: String,
    pub duration: f64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub has_video: bool,
    pub is_native_video: bool,
}

/// Sidecar timeouts per call site (seconds). A corrupt/truncated file can make
/// ffmpeg hang forever; without a timeout that would freeze the whole
/// playback path, so every spawn is bounded and killed on expiry.
pub const PROBE_TIMEOUT_SECS: u64 = 30;
pub const COVER_TIMEOUT_SECS: u64 = 60;
pub const DECODE_TIMEOUT_SECS: u64 = 900;
pub const REMUX_TIMEOUT_SECS: u64 = 3600;

/// Run the bundled ffmpeg/ffprobe (shipped inside the app's resource dir, so a
/// `.deb` install never touches `/usr/bin` and cannot fight the distro's own
/// ffmpeg package) to completion and collect its output.
///
/// The child is killed when it exceeds `timeout_secs`; stdout/stderr are read
/// on dedicated threads so a chatty process can never deadlock the pipe.
fn run_sidecar(
    app: &AppHandle,
    bin: &str,
    args: &[&str],
    timeout_secs: u64,
) -> Result<(bool, Vec<u8>, String), String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let path = app
        .path()
        .resource_dir()
        .map_err(|e| format!("获取资源目录失败: {e}"))?
        .join(format!("{bin}{}", std::env::consts::EXE_SUFFIX));

    let mut cmd = Command::new(&path);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // ffmpeg/ffprobe 是 console 子系统二进制，GUI 应用直接 spawn 会弹黑窗。
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("执行 {bin} ({}) 失败: {e}", path.display()))?;

    let stdout_pipe = child.stdout.take().expect("stdout 已设置为 piped");
    let stderr_pipe = child.stderr.take().expect("stderr 已设置为 piped");
    let stdout_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut pipe = stdout_pipe;
        let _ = pipe.read_to_end(&mut buf);
        buf
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut pipe = stderr_pipe;
        let _ = pipe.read_to_end(&mut buf);
        buf
    });

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    crate::log::log(&format!(
                        "[sidecar] {bin} 超过 {timeout_secs}s 未退出，已强制终止"
                    ));
                    break None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(format!("等待 {bin} 退出失败: {e}")),
        }
    };

    let out = stdout_reader.join().unwrap_or_default();
    let err = String::from_utf8_lossy(&stderr_reader.join().unwrap_or_default()).into_owned();

    match status {
        Some(status) => Ok((status.success(), out, err)),
        None => Err(format!("{bin} 执行超时（>{timeout_secs}s），已终止")),
    }
}

/// Recursively collect every media file inside `dir`.
pub fn scan_media_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&d) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else if is_media_file(&p) {
                    out.push(p);
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Encoding of file paths into stream/cover query keys (base64url, no padding).
// ---------------------------------------------------------------------------

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

pub fn encode_key(path: &Path) -> String {
    URL_SAFE_NO_PAD.encode(path.as_os_str().as_encoded_bytes())
}

pub fn decode_key(key: &str) -> Option<PathBuf> {
    URL_SAFE_NO_PAD.decode(key).ok().and_then(|b| {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            Some(PathBuf::from(std::ffi::OsString::from_vec(b)))
        }
        #[cfg(not(unix))]
        {
            String::from_utf8(b).ok().map(PathBuf::from)
        }
    })
}

/// A short (16 hex chars) stable hash of a path, used for cache *file names*.
/// The full base64 key can exceed the 255-byte filename limit on long paths
/// (ENAMETOOLONG), so file names always use this hash instead.
pub fn short_key(path: &Path) -> String {
    let bytes = path.as_os_str().as_encoded_bytes();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a offset basis
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3); // FNV-1a prime
    }
    format!("{hash:016x}")
}

/// Create a temporary ASCII-named handle pointing at `src`, so the ffmpeg
/// sidecar can open files whose real name is not valid UTF-8.
/// On Unix this is a symlink; on Windows the original path is returned as-is
/// (Windows file names are UTF-16 and pass through process arguments intact,
/// so the mangled-name problem cannot happen there — and a full copy of the
/// media file just to run ffprobe would be far too expensive).
/// Returns the handle path (which lives inside `cache_dir` on Unix).
fn safe_link(cache_dir: &Path, src: &Path, tag: &str) -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        let _ = (cache_dir, tag);
        Ok(src.to_path_buf())
    }
    #[cfg(unix)]
    {
        std::fs::create_dir_all(cache_dir.join("src")).ok();
        let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("bin");
        let name = format!("{tag}-{}.{}", short_key(src), ext);
        let link = cache_dir.join("src").join(&name);
        if link.exists() {
            return Ok(link);
        }
        std::os::unix::fs::symlink(src, &link).map_err(|e| format!("无法创建符号链接: {e}"))?;
        Ok(link)
    }
}

fn get_json_str(v: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| v.get(*k).and_then(|x| x.as_str()).map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
}

/// Probe a single file with ffprobe and build its TrackInfo.
fn track_info(app: &AppHandle, cache_dir: &Path, path: &Path) -> TrackInfo {
    let name = path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());

    let mut duration = 0.0f64;
    let (mut title, mut artist, mut album) = (None, None, None);
    let mut has_video = false;

    if let Ok(link) = safe_link(cache_dir, path, "probe") {
        if let Some(s) = link.to_str() {
            let args = [
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                s,
            ];
            if let Ok((ok, out, _)) = run_sidecar(app, "ffprobe", &args, PROBE_TIMEOUT_SECS) {
                if ok {
                    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out) {
                        let format = &json["format"];
                        duration = format["duration"]
                            .as_str()
                            .and_then(|d| d.parse().ok())
                            .unwrap_or(0.0);
                        let tags = &format["tags"];
                        title = get_json_str(tags, &["title"]);
                        artist = get_json_str(tags, &["artist", "album_artist"]);
                        album = get_json_str(tags, &["album"]);
                        if let Some(streams) = json["streams"].as_array() {
                            for s in streams {
                                let ct = s["codec_type"].as_str().unwrap_or("");
                                // An attached picture (embedded cover) is not a video
                                // stream, so skip it when deciding video-ness.
                                if ct == "video"
                                    && s["disposition"]["attached_pic"].as_i64().unwrap_or(0) != 1
                                {
                                    has_video = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let title = title.unwrap_or_else(|| name.clone());
    let artist = artist.unwrap_or_else(|| "未知艺术家".to_string());
    let album = album.unwrap_or_default();

    TrackInfo {
        path: encode_key(path),
        name,
        duration,
        title,
        artist,
        album,
        has_video,
        is_native_video: is_video_file(path),
    }
}

/// Probe a list of files in parallel and preserve the given order.
pub fn probe_all(app: &AppHandle, cache_dir: &Path, paths: Vec<PathBuf>) -> Vec<TrackInfo> {
    let n = paths.len();
    if n == 0 {
        return Vec::new();
    }
    let workers = std::thread::available_parallelism()
        .map(|x| x.get())
        .unwrap_or(4)
        .min(8);

    let app = app.clone();
    let paths = Arc::new(paths);
    let cache = Arc::new(cache_dir.to_path_buf());
    let next = Arc::new(AtomicUsize::new(0));
    let out: Arc<Mutex<Vec<Option<TrackInfo>>>> =
        Arc::new(Mutex::new((0..n).map(|_| None).collect()));

    let mut handles = Vec::new();
    for _ in 0..workers {
        let (app, paths, cache, next, out) = (
            app.clone(),
            paths.clone(),
            cache.clone(),
            next.clone(),
            out.clone(),
        );
        handles.push(std::thread::spawn(move || loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            if i >= n {
                break;
            }
            let info = track_info(&app, &cache, &paths[i]);
            if let Some(slot) = out.lock().unwrap().get_mut(i) {
                *slot = Some(info);
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }

    Arc::try_unwrap(out)
        .ok()
        .map(|m| m.into_inner().unwrap())
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .collect()
}

/// Decode `src` to a PCM WAV file at `dst` (any supported input -> s16le WAV).
pub fn decode_to_wav(
    app: &AppHandle,
    cache_dir: &Path,
    src: &Path,
    dst: &Path,
) -> Result<(), String> {
    let link = safe_link(cache_dir, src, "dec")?;
    let link_str = link.to_string_lossy().into_owned();
    let dst_str = dst.to_string_lossy().into_owned();
    let args = [
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-i",
        link_str.as_str(),
        "-vn",
        "-acodec",
        "pcm_s16le",
        "-f",
        "wav",
        dst_str.as_str(),
    ];
    let (ok, _, err) = run_sidecar(app, "ffmpeg", &args, DECODE_TIMEOUT_SECS)?;
    if ok && dst.exists() {
        Ok(())
    } else {
        Err(format!("解码失败: {err}"))
    }
}

/// Extract the embedded cover art (or first video frame) as a JPEG.
/// Returns false when the file has no picture track.
pub fn extract_cover(
    app: &AppHandle,
    cache_dir: &Path,
    src: &Path,
    dst: &Path,
) -> Result<bool, String> {
    let link = safe_link(cache_dir, src, "cov")?;
    let link_str = link.to_string_lossy().into_owned();
    let dst_str = dst.to_string_lossy().into_owned();
    let args = [
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-i",
        link_str.as_str(),
        "-map",
        "0:v:0",
        "-frames:v",
        "1",
        "-c:v",
        "mjpeg",
        "-q:v",
        "3",
        dst_str.as_str(),
    ];
    let (ok, _, _) = run_sidecar(app, "ffmpeg", &args, COVER_TIMEOUT_SECS)?;
    Ok(ok && dst.exists())
}

// ---------------------------------------------------------------------------
// 字幕：列出可渲染的文本字幕轨，并按需提取为 WebVTT 供 <track> 播放。
// 位图字幕（pgssub/dvd_subtitle/dvb_subtitle）无法转成 VTT 文本，不列出。
// ---------------------------------------------------------------------------

pub const SUBTITLE_TIMEOUT_SECS: u64 = 120;

/// ffmpeg 能转成 WebVTT 的文本字幕编码。
const TEXT_SUBTITLE_CODECS: &[&str] = &["subrip", "srt", "ass", "ssa", "mov_text", "webvtt"];

#[derive(Debug, Clone, Serialize)]
pub struct SubtitleInfo {
    /// 相对字幕轨序号（`ffmpeg -map 0:s:<track>` 用）。注意它按所有字幕轨
    /// （含位图轨）计数，与 ffprobe 的绝对 stream index 不同。
    pub track: u32,
    /// ffprobe 的绝对 stream index（仅信息展示用）。
    pub stream_index: u32,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
}

/// 缓存文件名：`<short_key>-sub<track>.vtt`（命令生成与服务端读取共用）。
pub fn subtitle_cache_name(path: &Path, track: u32) -> String {
    format!("{}-sub{track}.vtt", short_key(path))
}

/// 列出 `path` 中可渲染的文本字幕轨（无可渲染字幕时返回空）。
pub fn list_subtitles(app: &AppHandle, cache_dir: &Path, path: &Path) -> Vec<SubtitleInfo> {
    let Ok(link) = safe_link(cache_dir, path, "probe") else {
        return Vec::new();
    };
    let Some(s) = link.to_str() else {
        return Vec::new();
    };
    let args = ["-v", "quiet", "-print_format", "json", "-show_streams", s];
    let Ok((ok, out, _)) = run_sidecar(app, "ffprobe", &args, PROBE_TIMEOUT_SECS) else {
        return Vec::new();
    };
    if !ok {
        return Vec::new();
    }
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out) else {
        return Vec::new();
    };
    let Some(streams) = json["streams"].as_array() else {
        return Vec::new();
    };

    let mut subs = Vec::new();
    let mut rel = 0u32;
    for st in streams {
        if st["codec_type"].as_str() != Some("subtitle") {
            continue;
        }
        let codec = st["codec_name"].as_str().unwrap_or("").to_string();
        let is_text = TEXT_SUBTITLE_CODECS
            .iter()
            .any(|c| codec.eq_ignore_ascii_case(c));
        let info = SubtitleInfo {
            track: rel,
            stream_index: st["index"].as_u64().unwrap_or(0) as u32,
            codec,
            language: st["tags"]["language"].as_str().map(String::from),
            title: get_json_str(&st["tags"], &["title"]),
        };
        rel += 1;
        if is_text {
            subs.push(info);
        }
    }
    subs
}

/// Extract subtitle stream `track`（相对序号）of `src` into a WebVTT file at
/// `dst`. Returns `Ok(true)` when a VTT with actual cues was produced and
/// `Ok(false)` when the track exists but has no dialogue (header-only output).
pub fn extract_subtitle(
    app: &AppHandle,
    cache_dir: &Path,
    src: &Path,
    dst: &Path,
    track: u32,
) -> Result<bool, String> {
    let link = safe_link(cache_dir, src, "sub")?;
    let link_str = link.to_string_lossy().into_owned();
    let dst_str = dst.to_string_lossy().into_owned();
    let args = [
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-i",
        link_str.as_str(),
        "-map",
        &format!("0:s:{track}"),
        "-f",
        "webvtt",
        dst_str.as_str(),
    ];
    let (ok, _, err) = run_sidecar(app, "ffmpeg", &args, SUBTITLE_TIMEOUT_SECS)?;
    if !ok {
        return Err(format!("字幕提取失败: {err}"));
    }
    // 只剩 "WEBVTT" 头的输出说明该轨没有可显示的对白。
    let non_empty = std::fs::metadata(dst)
        .map(|m| m.len() > 16)
        .unwrap_or(false);
    Ok(non_empty)
}

/// Remux `src` into a browser-friendly container.
///
/// Layered strategy, fastest to slowest:
/// 1. stream-copy (no re-encode) into MP4, index moved to the front;
/// 2. stream-copy into WebM (only possible with VP8/VP9 + Vorbis/Opus);
/// 3. re-encode into MP4 — H.264 + AAC when libx264 is compiled in, otherwise
///    the built-in MPEG-4 + AAC. Slower, but works for virtually any input,
///    which is what fixes "stream-copy fails -> video won't play" cases.
///
/// When `force_encode` is set (codec the webview can't decode, e.g. HEVC on
/// Windows), the stream-copy steps are skipped entirely and we go straight to
/// the H.264 re-encode, since a preserved incompatible codec would still fail.
///
/// Each attempt is logged so success/failure can be diagnosed.
/// Returns the path of the produced file on success.
pub fn remux_video(
    app: &AppHandle,
    cache_dir: &Path,
    src: &Path,
    dst: &Path,
    force_encode: bool,
) -> Result<PathBuf, String> {
    let link = safe_link(cache_dir, src, "vid")?;
    let link_str = link.to_string_lossy().into_owned();
    let mut last_err = String::new();

    // 1) 流复制（-c copy，不重编码）：MP4 优先，其次 WebM。
    if !force_encode {
        for container in ["mp4", "webm"] {
            let out = dst.with_extension(container);
            let out_str = out.to_string_lossy().into_owned();
            let args = [
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-i",
                link_str.as_str(),
                "-map",
                "0:v:0",
                "-map",
                "0:a?",
                "-c",
                "copy",
                "-movflags",
                "faststart",
                "-f",
                container,
                out_str.as_str(),
            ];
            match run_sidecar(app, "ffmpeg", &args, REMUX_TIMEOUT_SECS) {
                Ok((true, _, _)) if out.exists() => {
                    crate::log::log(&format!("[remux] {container} 流复制成功 -> {out_str}"));
                    return Ok(out);
                }
                Ok((_, _, err)) => {
                    crate::log::log(&format!("[remux] {container} 流复制失败: {err}"));
                    last_err = err;
                }
                Err(e) => {
                    crate::log::log(&format!("[remux] {container} 执行失败: {e}"));
                    last_err = e;
                }
            }
        }
    }

    // 2) 重编码：H.264+AAC 兼容性最好；本构建没带 libx264 时，Windows 直接
    //    报错（Chromium 不认 mpeg4），Linux 用内置 MPEG-4 编码器兜底。
    #[cfg(target_os = "windows")]
    let encoders: &[&str] = &["libx264"];
    #[cfg(not(target_os = "windows"))]
    let encoders: &[&str] = &["libx264", "mpeg4"];

    for vcodec in encoders {
        let out = dst.with_extension("mp4");
        let out_str = out.to_string_lossy().into_owned();
        let args = [
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-i",
            link_str.as_str(),
            "-map",
            "0:v:0",
            "-map",
            "0:a?",
            "-c:v",
            vcodec,
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "faststart",
            "-f",
            "mp4",
            out_str.as_str(),
        ];
        match run_sidecar(app, "ffmpeg", &args, REMUX_TIMEOUT_SECS) {
            Ok((true, _, _)) if out.exists() => {
                crate::log::log(&format!("[remux] mp4 重编码({vcodec})成功 -> {out_str}"));
                return Ok(out);
            }
            Ok((_, _, err)) => {
                crate::log::log(&format!("[remux] mp4 重编码({vcodec})失败: {err}"));
                last_err = err;
            }
            Err(e) => {
                crate::log::log(&format!("[remux] mp4 重编码({vcodec})执行失败: {e}"));
                last_err = e;
            }
        }
    }

    Err(format!(
        "该视频流复制与重编码均无法转为可播放格式，最后错误: {last_err}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(name: &str, bytes: &[u8]) -> PathBuf {
        let p = std::env::temp_dir().join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(bytes).unwrap();
        p
    }

    #[test]
    fn faststart_detection() {
        assert!(has_faststart(&write_tmp("vp_fast.mp4", b"moovmdatXXXX")));
        assert!(!has_faststart(&write_tmp("vp_late.mp4", b"mdatmoovXXXX")));
        assert!(!has_faststart(&write_tmp("vp_none.mp4", b"XXXXXXXXXXXX")));
        assert!(!has_faststart(&PathBuf::from("/nonexistent/file.mp4")));
    }

    #[test]
    fn faststart_scans_beyond_1mb() {
        // moov far past the 1 MB scan window => not faststart
        let mut bytes = Vec::new();
        bytes.extend(std::iter::repeat_n(b'x', 2_000_000));
        bytes.extend_from_slice(b"moov");
        let p = write_tmp("vp_big.mp4", &bytes);
        assert!(!has_faststart(&p));
        // moov near the start with a huge data section => faststart
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"moov");
        bytes.extend(std::iter::repeat_n(b'x', 2_000_000));
        let p = write_tmp("vp_big2.mp4", &bytes);
        assert!(has_faststart(&p));
    }
}

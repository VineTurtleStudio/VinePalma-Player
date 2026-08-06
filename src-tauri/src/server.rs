//! A tiny loopback HTTP server that streams decoded audio, remuxed video and
//! cover art to the webview's `<audio>` / `<video>` / `<img>` elements.
//!
//! Only `127.0.0.1` is ever bound, and requests are served from a private
//! cache directory. Range requests are honoured so seeking works.

use std::collections::HashMap;
use std::fs::File;
use std::io::{empty, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::media;
use tauri::AppHandle;

pub struct StreamState {
    app: AppHandle,
    pub port: u16,
    cache_dir: PathBuf,
    decodes: Mutex<HashMap<String, PathBuf>>,
    covers: Mutex<HashMap<String, Option<PathBuf>>>,
    videos: Mutex<HashMap<String, PathBuf>>,
}

impl StreamState {
    pub fn start(app: AppHandle, cache_dir: PathBuf) -> Arc<StreamState> {
        std::fs::create_dir_all(&cache_dir).ok();
        let server = Server::http("127.0.0.1:0").expect("无法启动本地流媒体服务器");
        let port = server.server_addr().to_ip().map(|i| i.port()).unwrap_or(0);

        let state = Arc::new(StreamState {
            app,
            port,
            cache_dir,
            decodes: Mutex::new(HashMap::new()),
            covers: Mutex::new(HashMap::new()),
            videos: Mutex::new(HashMap::new()),
        });

        let worker = state.clone();
        std::thread::spawn(move || {
            for request in server.incoming_requests() {
                let st = worker.clone();
                std::thread::spawn(move || handle(request, st));
            }
        });

        state
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn play_url(&self, path: &std::path::Path) -> String {
        format!("{}/play?p={}", self.base_url(), media::encode_key(path))
    }

    pub fn video_url(&self, path: &std::path::Path) -> String {
        format!("{}/video?p={}", self.base_url(), media::encode_key(path))
    }

    pub fn cover_url(&self, path: &std::path::Path) -> String {
        format!("{}/cover?p={}", self.base_url(), media::encode_key(path))
    }

    /// Decode a file to a cached WAV (idempotent).
    pub fn ensure_decoded(&self, path: &std::path::Path) -> Result<PathBuf, String> {
        let key = media::short_key(path);
        if let Some(p) = self.decodes.lock().unwrap().get(&key) {
            return Ok(p.clone());
        }
        let dst = self.cache_dir.join(format!("{key}.wav"));
        media::decode_to_wav(&self.app, &self.cache_dir, path, &dst)?;
        self.decodes.lock().unwrap().insert(key, dst.clone());
        Ok(dst)
    }

    /// Produce a streamable video file (idempotent): returns the original file
    /// when its container is already browser-native *and* seekable (faststart),
    /// otherwise remuxes a copy so the index sits at the front.
    pub fn ensure_video(&self, path: &std::path::Path) -> Result<PathBuf, String> {
        let key = media::short_key(path);
        if let Some(p) = self.videos.lock().unwrap().get(&key) {
            return Ok(p.clone());
        }

        let resolved = if media::is_native_seekable(path) {
            crate::log::log(&format!("VIDEO {} 原生容器可用，直接服务", path.display()));
            path.to_path_buf()
        } else {
            crate::log::log(&format!("VIDEO {} 需要 remux", path.display()));
            let dst = self.cache_dir.join(format!("{key}-remux"));
            let out = media::remux_video(&self.app, &self.cache_dir, path, &dst)?;
            crate::log::log(&format!("VIDEO remux 完成 -> {}", out.display()));
            out
        };

        self.videos.lock().unwrap().insert(key, resolved.clone());
        Ok(resolved)
    }

    /// Lazily extract and cache the cover art for a single file. Negative
    /// results are cached too, so files without covers are only probed once.
    /// Returns the cover URL when the file has an embedded picture.
    pub fn ensure_cover(&self, src: &std::path::Path) -> Option<PathBuf> {
        let key = media::short_key(src);
        {
            let map = self.covers.lock().unwrap();
            if let Some(v) = map.get(&key) {
                return v.clone();
            }
        }
        let dst = self.cache_dir.join(format!("{key}.jpg"));
        let result = match media::extract_cover(&self.app, &self.cache_dir, src, &dst) {
            Ok(true) => Some(dst),
            _ => None,
        };
        self.covers.lock().unwrap().insert(key, result.clone());
        result
    }
}

type Body = Box<dyn Read + Send>;

fn empty_resp(status: u16) -> Response<Body> {
    let reader: Body = Box::new(empty());
    Response::new(StatusCode(status), Vec::new(), reader, None, None)
}

fn get_header(req: &Request, name: &str) -> Option<String> {
    req.headers()
        .iter()
        .find(|h| h.field.as_str().to_string().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str().to_string())
}

fn handle(request: Request, state: Arc<StreamState>) {
    let head_only = request.method() == &Method::Head;
    if request.method() != &Method::Get && !head_only {
        let _ = request.respond(empty_resp(405));
        return;
    }

    let url = request.url().to_string();
    let (path, query) = match url.split_once('?') {
        Some((p, q)) => (p.trim_start_matches('/'), q),
        None => (url.trim_start_matches('/'), ""),
    };

    let range = get_header(&request, "Range");
    let key = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("p="))
        .map(|v| v.to_string());

    let response = match (path, key) {
        ("play", Some(key)) => handle_play(&state, &key, range.clone(), head_only),
        ("video", Some(key)) => handle_video(&state, &key, range.clone(), head_only),
        ("cover", Some(key)) => handle_cover(&state, &key, head_only),
        _ => Some(empty_resp(404)),
    };

    if let Some(resp) = response {
        let status = resp.status_code().0;
        crate::log::log(&format!(
            "HTTP {} {} range={:?} -> {}",
            if head_only { "HEAD" } else { "GET" },
            url,
            range,
            status
        ));
        let _ = request.respond(resp);
    }
}

/// Serve a static file with optional Range support.
///
/// Range semantics follow RFC 7233: a valid range gets 206 with a
/// `Content-Range` header, no `Range` header gets the full 200 body, and an
/// explicit-but-invalid range gets 416 (`Content-Range: bytes */<size>`).
/// Returning 200 for a bad range used to make the browser think seeking works
/// when it does not, which can stall playback mid-stream.
fn serve_file(
    file: &std::path::Path,
    content_type: &str,
    range_header: Option<String>,
    head_only: bool,
) -> Option<Response<Body>> {
    let size = std::fs::metadata(file).ok()?.len();

    crate::log::log(&format!("serve_file: path={:?}, size={}", file, size));

    let (status, start, len) = if let Some(h) = range_header {
        match parse_range(&h, size) {
            Some((s, e)) => (StatusCode(206), s, e - s + 1),
            None => {
                let headers = vec![
                    Header::from_bytes("Accept-Ranges", "bytes").ok()?,
                    Header::from_bytes(
                        "Content-Range",
                        format!("bytes */{size}").as_bytes(),
                    )
                    .ok()?,
                ];
                let reader: Body = Box::new(empty());
                return Some(Response::new(
                    StatusCode(416),
                    headers,
                    reader,
                    Some(0),
                    None,
                ));
            }
        }
    } else {
        (StatusCode(200), 0, size)
    };


    crate::log::log(&format!("serve_file: status={}, start={}, len={}, size={}", status.0, start, len, size)); // 新增
// ... 构造 headers，并打印 Content-Range
    let cr = format!("bytes {}-{}/{}", start, start + len - 1, size);
    crate::log::log(&format!("serve_file: Content-Range = {}", cr)); // 新增
let mut headers = vec![
    Header::from_bytes("Content-Type", content_type).ok()?,
    Header::from_bytes("Accept-Ranges", "bytes").ok()?,
    Header::from_bytes("Cache-Control", "no-store").ok()?,
    Header::from_bytes("Access-Control-Allow-Origin", "*").ok()?,
];
// 总是添加 Content-Length
headers.push(Header::from_bytes("Content-Length", len.to_string().as_bytes()).ok()?);

// 仅在 206 时添加 Content-Range
if status.0 == 206 {
    let cr = format!("bytes {}-{}/{}", start, start + len - 1, size);
    headers.push(Header::from_bytes("Content-Range", cr.as_bytes()).ok()?);
}

    let reader: Body = if head_only {
        Box::new(empty())
    } else {
        let mut f = File::open(file).ok()?;
        f.seek(SeekFrom::Start(start)).ok()?;
        Box::new(f.take(len))
    };

    // 关键：必须让响应带真实的 Content-Length，而不是 Transfer-Encoding: chunked。
    // tiny_http 默认对超过 32KB 的 body 用 chunked，而 WebKitGTK 把“无已知长度”
    // 的媒体流当直播流处理：duration=0/Infinity、seekable 只在已缓冲范围内，
    // 进度条无法映射、点击 seek 会落到流末尾触发 ended。抬高 chunked 阈值后
    // 媒体响应一律带 Content-Length，WebKit 才能获得完整时长并可全范围 seek。
    Some(
        Response::new(status, headers, reader, Some(len as usize), None)
            .with_chunked_threshold(usize::MAX),
    )
}

fn handle_play(
    state: &StreamState,
    key: &str,
    range: Option<String>,
    head_only: bool,
) -> Option<Response<Body>> {
    let src = media::decode_key(key)?;
    // 常见音频直接原样流式播放：浏览器（GStreamer）能原生解码并可靠 seek，
    // 免去整文件转 WAV 的等待（解码慢）与字节流定位错乱（进度条无效果）。
    // 只有原生不支持的编码（wma/ape/amr 等）才回退到缓存 WAV。
    if media::is_native_audio(&src) {
        let content_type = media::native_audio_content_type(&src);
        crate::log::log(&format!("AUDIO 原生直供: {}", src.display()));
        return serve_file(&src, content_type, range, head_only);
    }
    let wav = state.ensure_decoded(&src).ok()?;
    serve_file(&wav, "audio/wav", range, head_only)
}

fn handle_video(
    state: &StreamState,
    key: &str,
    range: Option<String>,
    head_only: bool,
) -> Option<Response<Body>> {
    let src = media::decode_key(key)?;
    let video = state.ensure_video(&src).ok()?;
    let content_type = match video.extension().and_then(|e| e.to_str()) {
        Some(e) if e.eq_ignore_ascii_case("webm") => "video/webm",
        Some(e) if e.eq_ignore_ascii_case("ogg") => "video/ogg",
        _ => "video/mp4",
    };
    serve_file(&video, content_type, range, head_only)
}

fn handle_cover(state: &StreamState, key: &str, head_only: bool) -> Option<Response<Body>> {
    let src = media::decode_key(key)?;
    let cover = state.ensure_cover(&src)?;
    let len = std::fs::metadata(&cover).ok()?.len();

    let headers = vec![
        Header::from_bytes("Content-Type", "image/jpeg").ok()?,
        Header::from_bytes("Cache-Control", "max-age=86400").ok()?,
    ];
    let reader: Body = if head_only {
        Box::new(empty())
    } else {
        Box::new(File::open(&cover).ok()?)
    };

    Some(
        Response::new(
            StatusCode(200),
            headers,
            reader,
            Some(len as usize),
            None,
        )
        .with_chunked_threshold(usize::MAX),
    )
}

/// Parse a `Range: bytes=...` header into an inclusive byte range.
fn parse_range(header: &str, size: u64) -> Option<(u64, u64)> {
    let v = header.strip_prefix("bytes=")?;
    let (a, b) = v.split_once('-')?;
    let start = if a.is_empty() {
        size.saturating_sub(b.parse::<u64>().ok()?)
    } else {
        a.parse().ok()?
    };
    // A suffix range (`bytes=-N`) is "the last N bytes", so its end is always
    // the final byte of the file, just like an open-ended (`bytes=N-`) range.
    let end = if b.is_empty() || a.is_empty() {
        size.saturating_sub(1)
    } else {
        b.parse::<u64>().ok()?.min(size.saturating_sub(1))
    };
    if start > end || start >= size {
        return None;
    }
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_http::HTTPVersion;

    fn write_tmp(name: &str, bytes: &[u8]) -> PathBuf {
        let p = std::env::temp_dir().join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    /// Render a full HTTP response into bytes (headers + body) so the exact
    /// bytes a browser / GStreamer would receive can be asserted.
    fn render(resp: Response<Body>, head_only: bool) -> Vec<u8> {
        let mut buf = Vec::new();
        let v = HTTPVersion::try_from((1, 1)).unwrap();
        resp.raw_print(&mut buf, v, &[], head_only, None).unwrap();
        buf
    }

    const N: usize = 1000;

    #[test]
    fn full_request_serves_every_byte_with_content_length() {
        let bytes: Vec<u8> = (0..N as u32).map(|i| (i % 251) as u8).collect();
        let f = write_tmp("vp_rng_full.bin", &bytes);

        let resp = serve_file(&f, "application/octet-stream", None, false).unwrap();
        assert_eq!(resp.status_code().0, 200);
        let out = render(resp, false);
        let head = String::from_utf8_lossy(&out);
        assert!(head.contains("Content-Length: 1000"), "headers: {head}");
        assert!(head.contains("Accept-Ranges: bytes"), "headers: {head}");
        // every byte of the file must reach the client
        assert_eq!(&out[out.len() - N..], &bytes[..]);
    }

    #[test]
    fn explicit_range_serves_only_the_requested_slice() {
        let bytes: Vec<u8> = (0..N as u32).map(|i| (i % 251) as u8).collect();
        let f = write_tmp("vp_rng_slice.bin", &bytes);

        let resp = serve_file(&f, "application/octet-stream", Some("bytes=100-199".into()), false).unwrap();
        assert_eq!(resp.status_code().0, 206);
        let out = render(resp, false);
        let head = String::from_utf8_lossy(&out);
        assert!(head.contains("Content-Range: bytes 100-199/1000"), "headers: {head}");
        assert!(head.contains("Content-Length: 100"), "headers: {head}");
        assert_eq!(&out[out.len() - 100..], &bytes[100..200]);
    }

    #[test]
    fn open_ended_and_suffix_ranges_are_correct() {
        let bytes: Vec<u8> = (0..N as u32).map(|i| (i % 251) as u8).collect();
        let f = write_tmp("vp_rng_suffix.bin", &bytes);

        // bytes=500-  => from offset 500 to end
        let resp = serve_file(&f, "application/octet-stream", Some("bytes=500-".into()), false).unwrap();
        assert_eq!(resp.status_code().0, 206);
        let out = render(resp, false);
        let head = String::from_utf8_lossy(&out);
        assert!(head.contains("Content-Range: bytes 500-999/1000"), "headers: {head}");
        assert_eq!(&out[out.len() - 500..], &bytes[500..]);

        // bytes=-100 => the last 100 bytes
        let resp = serve_file(&f, "application/octet-stream", Some("bytes=-100".into()), false).unwrap();
        assert_eq!(resp.status_code().0, 206);
        let out = render(resp, false);
        let head = String::from_utf8_lossy(&out);
        assert!(head.contains("Content-Range: bytes 900-999/1000"), "headers: {head}");
        assert_eq!(&out[out.len() - 100..], &bytes[900..]);
    }

    #[test]
    fn head_request_reports_length_without_a_body() {
        let bytes: Vec<u8> = (0..N as u32).map(|i| (i % 251) as u8).collect();
        let f = write_tmp("vp_rng_head.bin", &bytes);

        let resp = serve_file(&f, "application/octet-stream", None, true).unwrap();
        assert_eq!(resp.status_code().0, 200);
        let out = render(resp, true);
        let head = String::from_utf8_lossy(&out);
        assert!(head.contains("Content-Length: 1000"), "headers: {head}");
        // no body bytes may follow the header block
        let header_end = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        assert_eq!(out.len(), header_end, "HEAD must not send a body");
    }

    #[test]
    fn invalid_ranges_return_416_with_content_range() {
        let bytes: Vec<u8> = (0..N as u32).map(|i| (i % 251) as u8).collect();
        let f = write_tmp("vp_rng_invalid.bin", &bytes);

        for bad in ["bytes=2000-3000", "garbage", "bytes=abc-def", "bytes=5-2"] {
            let resp = serve_file(&f, "application/octet-stream", Some(bad.to_string()), false).unwrap();
            assert_eq!(resp.status_code().0, 416, "range {bad:?} should be rejected -> 416");
            let out = render(resp, false);
            let head = String::from_utf8_lossy(&out);
            assert!(head.contains("Content-Range: bytes */1000"), "headers: {head}");
        }
    }

    #[test]
    fn parse_range_edges() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=500-", 1000), Some((500, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=0-", 1000), Some((0, 999)));
        assert_eq!(parse_range("bytes=100-99", 1000), None);
        assert_eq!(parse_range("bytes=2000-3000", 1000), None);
        assert_eq!(parse_range("bytes=abc", 1000), None);
        assert_eq!(parse_range("no-range", 1000), None);
    }
}

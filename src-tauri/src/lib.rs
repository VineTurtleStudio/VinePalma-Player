use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

mod log;
mod media;
mod server;

use server::StreamState;

pub struct AppState {
    stream: std::sync::Arc<StreamState>,
    settings: Mutex<serde_json::Map<String, Value>>,
    settings_path: PathBuf,
}

impl AppState {
    fn save_settings(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&*self.settings.lock().unwrap()) {
            let _ = std::fs::write(&self.settings_path, json);
        }
    }
}

fn clean_cache(cache_dir: &std::path::Path) {
    const MAX_AGE_DAYS: u64 = 7;
    const MAX_SIZE_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2GB

    let now = std::time::SystemTime::now();
    let max_age = std::time::Duration::from_secs(MAX_AGE_DAYS * 24 * 3600);

    // 缓存根目录与一层子目录（如 Unix 上的 src/ 符号链接目录）都纳入清理范围。
    let mut files: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();
    let mut dirs = vec![cache_dir.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if let Ok(meta) = path.metadata() {
                    if let Ok(modified) = meta.modified() {
                        files.push((path, modified));
                    }
                }
            }
        }
    }

    files.sort_by_key(|(_, t)| *t);

    let mut total_size = 0u64;
    let mut to_delete = Vec::new();
    for (path, modified) in &files {
        if let Ok(age) = now.duration_since(*modified) {
            if age > max_age {
                to_delete.push(path.clone());
            } else {
                if let Ok(meta) = std::fs::metadata(path) {
                    total_size += meta.len();
                }
            }
        }
    }
    for path in to_delete {
        let _ = std::fs::remove_file(&path);
    }

    if total_size > MAX_SIZE_BYTES {
        let mut remaining_files: Vec<_> = files.into_iter().filter(|(p, _)| p.exists()).collect();
        remaining_files.sort_by_key(|(_, t)| *t);
        for (path, _) in remaining_files {
            if total_size <= MAX_SIZE_BYTES {
                break;
            }
            if let Ok(meta) = std::fs::metadata(&path) {
                total_size -= meta.len();
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

/// Pick one media file and return its metadata.
#[tauri::command]
async fn open_file(app: AppHandle) -> Result<Option<media::TrackInfo>, String> {
    let (tx, rx) = std::sync::mpsc::channel::<Option<tauri_plugin_dialog::FilePath>>();
    let dialog = app
        .dialog()
        .file()
        .set_title("选择媒体文件")
        .add_filter("媒体文件", media::DIALOG_EXTS)
        .add_filter("所有文件", &["*"]);
    dialog.pick_file(move |f| {
        let _ = tx.send(f);
    });

    let picked = tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten())
        .await
        .map_err(|e| e.to_string())?;

    let Some(fp) = picked else {
        return Ok(None);
    };
    let path = fp.into_path().map_err(|e| e.to_string())?;

    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    Ok(media::probe_all(&app, &cache_dir, vec![path])
        .into_iter()
        .next())
}

/// Scan a folder (recursively) and return metadata for every media file.
#[tauri::command]
async fn open_folder(app: AppHandle) -> Result<Vec<media::TrackInfo>, String> {
    let (tx, rx) = std::sync::mpsc::channel::<Option<tauri_plugin_dialog::FilePath>>();
    let dialog = app.dialog().file().set_title("导入文件夹");
    dialog.pick_folder(move |f| {
        let _ = tx.send(f);
    });

    let picked = tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten())
        .await
        .map_err(|e| e.to_string())?;

    let Some(fp) = picked else {
        return Ok(Vec::new());
    };
    let dir = fp.into_path().map_err(|e| e.to_string())?;

    let files = media::scan_media_files(&dir);
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    Ok(media::probe_all(&app, &cache_dir, files))
}

/// Return a streamable URL for an audio track.
/// `path` is a base64-encoded (raw bytes) file path.
/// Native formats are served lazily as-is by the stream server, so no upfront
/// decode is needed. Only exotic codecs (wma/ape/amr/…) must be decoded to a
/// cached WAV first, and that happens here so errors surface before playback.
#[tauri::command]
async fn stream_url(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let src = media::decode_key(&path).ok_or("无效的文件路径")?;
    if !media::is_native_audio(&src) {
        let stream = state.stream.clone();
        let src_for_task = src.clone();
        tauri::async_runtime::spawn_blocking(move || stream.ensure_decoded(&src_for_task))
            .await
            .map_err(|e| e.to_string())??;
    }
    Ok(state.stream.play_url(&src))
}

/// Return a streamable URL for a video track (native file or remuxed copy).
/// `path` is a base64-encoded (raw bytes) file path.
/// `mode`: "asset" (default) returns the resolved file path so the frontend
/// can turn it into an `http://asset.localhost/...` URL via `convertFileSrc`
/// (Tauri's asset protocol serves the file with Range support, so duration
/// and seeking work). "http" returns the loopback stream URL as a fallback.
#[tauri::command]
async fn video_url(
    state: State<'_, AppState>,
    path: String,
    mode: Option<String>,
) -> Result<String, String> {
    let src = media::decode_key(&path).ok_or("无效的文件路径")?;
    let stream = state.stream.clone();
    let src_for_task = src.clone();
    let resolved = tauri::async_runtime::spawn_blocking(move || stream.ensure_video(&src_for_task))
        .await
        .map_err(|e| e.to_string())??;
    // 🟢 默认返回 HTTP 环回地址（支持 Range），除非显式请求 asset
    let url = if mode.as_deref() == Some("asset") {
        resolved.to_string_lossy().into_owned()
    } else {
        state.stream.video_url(&src) // http://127.0.0.1:port/video?p=...
    };
    Ok(url)
}

/// Lazily extract the embedded cover of a single file and return its URL.
/// Called once per track when it starts playing, so a folder import never
/// loads every cover up front. Returns null when the file has no picture.
#[tauri::command]
async fn get_cover(state: State<'_, AppState>, path: String) -> Result<Option<String>, String> {
    let src = media::decode_key(&path).ok_or("无效的文件路径")?;
    let stream = state.stream.clone();
    let src_for_task = src.clone();
    let has = tauri::async_runtime::spawn_blocking(move || stream.ensure_cover(&src_for_task))
        .await
        .map_err(|e| e.to_string())?;
    Ok(has.map(|_| state.stream.cover_url(&src)))
}

/// Open the settings window (created here so it never depends on the webview's
/// ability to create windows from JavaScript). Calling this again while the
/// window is already open closes it, so the header button toggles the page.
/// Returns true when the window ended up open.
#[tauri::command]
fn open_settings(app: AppHandle) -> Result<bool, String> {
    let existing = app.get_webview_window("settings");
    if let Some(w) = existing {
        let _ = w.close();
        return Ok(false);
    }
    tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("settings/settings.html".into()),
    )
    .title("VinePalma 设置")
    .inner_size(560.0, 640.0)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .build()
    .map(|_| true)
    .map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    fix_transparent_click_through(&app, "settings");
    Ok(true)
}

/// Position the mini player near the bottom-right of the monitor that currently
/// holds the main window (falling back to the primary monitor).
/// `Monitor::position()/size()` are physical pixels; `WebviewWindowBuilder::position`
/// expects logical pixels, so convert with the monitor scale factor (otherwise on
/// Windows DPI scaling >100% the card is placed off-screen and never appears).
fn bottom_right(app: &AppHandle, w: f64, h: f64) -> (f64, f64) {
    let mon = app
        .get_webview_window("main")
        .and_then(|m| m.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    if let Some(mon) = mon {
        let scale = mon.scale_factor().max(1.0);
        let pos = mon.position();
        let size = mon.size();
        let margin = 24.0f64;
        let x = pos.x as f64 / scale + size.width as f64 / scale - w - margin;
        let y = pos.y as f64 / scale + size.height as f64 / scale - h - margin;
        (x.max(0.0), y.max(0.0))
    } else {
        (0.0, 0.0)
    }
}

/// Open the always-on-top mini player card (like NetEase Cloud Music's mini
/// player). NetEase-style: clicking the mini button minimizes the main window
/// so the desktop only shows the small card. Re-opening focuses the existing
/// window instead of creating a new one.
#[tauri::command]
fn open_mini(app: AppHandle) -> Result<bool, String> {
    // 网易云式：进入迷你播放器时最小化主窗口，桌面上只留小卡片
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.minimize();
    }
    if let Some(w) = app.get_webview_window("mini") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_always_on_top(true);
        let _ = w.set_focus();
        return Ok(true);
    }
    let (x, y) = bottom_right(&app, 340.0, 64.0);
    let window =
        tauri::WebviewWindowBuilder::new(&app, "mini", tauri::WebviewUrl::App("mini.html".into()))
            .title("VinePalma 迷你播放器")
            .inner_size(340.0, 64.0)
            .position(x, y)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .build()
            .map_err(|e| e.to_string())?;
    // 非 Linux 平台不需要取窗口句柄做 GTK 修复，显式消费避免 unused 警告。
    #[cfg(not(target_os = "linux"))]
    let _ = &window;
    // WebKitGTK 会给 WebView 窗口强加约 200px 的最小高度（gtk_window_resize
    // 默认受子控件自然尺寸约束），导致 64px 的小卡片实际被撑高。对 GTK 窗口
    // 本身设置 size request 可覆盖该下限，从而真正得到 340x64。
    #[cfg(target_os = "linux")]
    {
        use gtk::prelude::*;
        if let Ok(gtk_win) = window.gtk_window() {
            gtk_win.set_size_request(340, 64);
            gtk_win.resize(340, 64);
        }
        fix_transparent_click_through(&app, "mini");
    }
    Ok(true)
}

/// Close the mini player card and restore the main window.
#[tauri::command]
fn close_mini(app: AppHandle) -> Result<(), String> {
    // 必须“先恢复主窗口、最后再关闭卡片”：命令执行中途一旦销毁发起命令
    // 的 mini 窗口，剩余语句很可能被中断，导致“拉出主窗口”不生效。
    if let Some(main) = app.get_webview_window("main") {
        // 先取消最小化再 show：某些平台（X11）对已最小化的窗口先 show 后
        // unminimize 顺序不正确时不会真正恢复，所以统一先 unminimize。
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }
    // 双保险：即使上面的窗口操作被窗口管理器拦截，主窗口 JS 收到该事件后
    // 也会在自己的进程里再执行一遍恢复。
    let _ = app.emit_to("main", "mini:close-restore", ());
    if let Some(w) = app.get_webview_window("mini") {
        let _ = w.close();
    }
    Ok(())
}

/// Forward a control action from the mini player to the main window.
/// `action` is one of "toggle" | "next" | "prev" | "seek" (seek carries `value`).
#[tauri::command]
fn mini_control(app: AppHandle, action: String, value: Option<f64>) -> Result<(), String> {
    let payload = serde_json::json!({ "action": action, "value": value });
    app.emit_to("main", "mini:command", payload)
        .map_err(|e| e.to_string())
}

/// Bring the main window to the front (clicking the mini card opens the player).
#[tauri::command]
fn mini_open_main(app: AppHandle) -> Result<(), String> {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }
    Ok(())
}

/// Append a line from the frontend to the diagnostic log file.
#[tauri::command]
fn log_app(level: String, msg: String) {
    log::log(&format!("[{}] {}", level, msg));
}

#[tauri::command]
fn store_get(state: State<'_, AppState>, key: String) -> Result<Value, String> {
    Ok(state
        .settings
        .lock()
        .unwrap()
        .get(&key)
        .cloned()
        .unwrap_or(Value::Null))
}

#[tauri::command]
fn store_set(state: State<'_, AppState>, key: String, value: Value) -> Result<(), String> {
    state.settings.lock().unwrap().insert(key, value);
    state.save_settings();
    Ok(())
}

/// 修复 Linux 透明窗口“整个窗口鼠标穿透、无法点击”的问题。
///
/// 根因：透明窗口下 WebKitGTK 会按页面内容的 alpha 计算“输入形状区域”
/// （input shape）。`<body>` 背景透明时，窗口在首次渲染、尺寸变化或地图后
/// 都可能被 WebKit 重新判定为全部穿透。单纯在 `map` 时重置一次不够——若重置
/// 发生在 WebKit 首次绘制（第一帧透明）之前，之后 WebKit 又会把整窗设为
/// 点击穿透，且此后不再触发 `map`。这里在启动窗口内多次延时重置输入区域，
/// 覆盖 map → 首帧绘制这段竞态（即“低概率打开时鼠标点不中”的来源）。
#[cfg(target_os = "linux")]
fn fix_transparent_click_through(app: &tauri::AppHandle, label: &str) {
    use gtk::prelude::*;
    use tauri::Manager;

    let Some(w) = app.get_webview_window(label) else {
        return;
    };
    let Ok(gtk_win) = w.gtk_window() else { return };

    fn reset_input_region(win: &gtk::Window) {
        win.input_shape_combine_region(None::<&gtk::cairo::Region>);
        for child in win.children() {
            if let Ok(view) = child.downcast::<webkit2gtk::WebView>() {
                view.input_shape_combine_region(None::<&gtk::cairo::Region>);
            }
        }
    }

    reset_input_region(&gtk_win.clone().upcast::<gtk::Window>());
    // 在启动竞态窗口内反复重置（0/100/300/800ms），而不是只在 map 时一次。
    for delay_ms in [0u32, 100, 300, 800, 2000] {
        let win = gtk_win.clone();
        gtk::glib::timeout_add_local(
            std::time::Duration::from_millis(delay_ms as u64),
            move || {
                reset_input_region(&win.clone().upcast::<gtk::Window>());
                gtk::glib::ControlFlow::Break
            },
        );
    }
    gtk_win.connect_map(|win| reset_input_region(&win.clone().upcast::<gtk::Window>()));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if cfg!(target_os = "linux") {
        // 1. 优先使用 X11（保证置顶有效）
        std::env::set_var("GDK_BACKEND", "x11,wayland");
        // 2. 仅禁用 DMA-BUF（解决鼠标穿透），保留合成加速
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        // 不再强制禁用 WEBKIT_DISABLE_COMPOSITING_MODE
        // 3. 沙箱保留（可选）
        if std::env::var_os("WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS").is_none() {
            std::env::set_var("WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS", "1");
        }
        // 4. 解码器策略：优先使用稳定的软件多线程解码（avdec_*）
        std::env::set_var(
            "GST_PLUGIN_FEATURE_RANK",
            "avdec_h264:MAX,avdec_h265:MAX,avdec_vp8:MAX,avdec_vp9:MAX",
        );
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            crate::log::init(app.handle());
            let cache_dir = app.path().app_cache_dir()?;
            clean_cache(&cache_dir);
            let stream = StreamState::start(app.handle().clone(), cache_dir);

            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let settings = std::fs::read_to_string(&settings_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            app.manage(AppState {
                stream,
                settings: Mutex::new(settings),
                settings_path,
            });
            #[cfg(target_os = "linux")]
            fix_transparent_click_through(app.handle(), "main");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_file,
            open_folder,
            stream_url,
            video_url,
            get_cover,
            open_settings,
            open_mini,
            close_mini,
            mini_control,
            mini_open_main,
            store_get,
            store_set,
            log_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

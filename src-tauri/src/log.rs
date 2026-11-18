//! Tiny file logger used to diagnose playback issues on the user's machine.
//!
//! Everything (HTTP requests, remux decisions, frontend media events) is
//! appended to `<cache>/player.log` with a timestamp so a frozen playback can
//! be traced after the fact. The same line also goes to stderr for dev runs.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

use tauri::{AppHandle, Manager};

static STATE: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

/// (Re)create the log file in the app cache dir. Call once at startup.
/// The previous run's log is kept as `player.log.1` (one generation) so a
/// crash can still be diagnosed from the launch before the frozen one.
pub fn init(app: &AppHandle) {
    let path = app
        .path()
        .app_cache_dir()
        .map(|dir| {
            std::fs::create_dir_all(&dir).ok();
            dir.join("player.log")
        })
        .unwrap_or_default();
    if path.exists() {
        let old = path.with_extension("log.1");
        let _ = std::fs::remove_file(&old);
        let _ = std::fs::rename(&path, &old);
    }
    *STATE.lock().unwrap() = Some(path);
    log("=== VinePalma Player 启动 ===");
}

pub fn log(msg: &str) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    if let Some(path) = STATE.lock().unwrap().as_ref() {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "[{ts:.3}] {msg}");
        }
    }
    eprintln!("[log] {msg}");
}

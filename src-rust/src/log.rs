//! Debug logging system.
//!
//! When --debug is active, logs to stdout and to <exe_dir>\debug.log.

use std::sync::OnceLock;
use std::sync::Mutex;

struct Logger {
    debug: bool,
    log_path: Option<std::path::PathBuf>,
}

static LOGGER: OnceLock<Mutex<Logger>> = OnceLock::new();

/// Initialize the logger. Call once at startup.
pub fn init(debug: bool) {
    let log_path = if debug {
        let exe = std::env::current_exe().unwrap_or_default();
        let dir = exe.parent().unwrap_or(std::path::Path::new("."));
        let path = dir.join("debug.log");
        // Create/truncate with header
        let _ = std::fs::write(&path, "=== ToastWindow Debug Log ===\n");
        Some(path)
    } else {
        None
    };

    let _ = LOGGER.set(Mutex::new(Logger { debug, log_path }));
}

/// Log a message. Only outputs if --debug was specified.
pub fn log(msg: &str) {
    let Some(logger) = LOGGER.get() else { return };
    let Ok(logger) = logger.lock() else { return };

    if !logger.debug && logger.log_path.is_none() {
        return;
    }

    // Debug output goes ONLY to the log file, never to stdout/console.
    // AllocConsole() would create a visible CMD window for GUI subsystem apps,
    // which is unacceptable for a notification tool.

    if let Some(ref path) = logger.log_path {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(path) {
            let _ = writeln!(f, "{}", msg);
        }
    }
}

/// Convenience macro for formatted logging with [DEBUG] prefix.
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        $crate::log::log(&format!("[DEBUG] {}", format!($($arg)*)))
    };
}

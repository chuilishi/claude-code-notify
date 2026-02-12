//! State file save/load/delete.
//!
//! State file: %TEMP%\claude-notify-{session_id}.txt
//! Format: 4 lines (HWND, RuntimeId, caller exe path, user prompt)

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, IsWindow};

/// Data stored in and loaded from the state file.
pub struct State {
    pub target_hwnd: HWND,
    pub wt_hwnd: HWND,
    pub wt_runtime_id: String,
    pub icon_path: String,
    pub user_prompt: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            target_hwnd: HWND::default(),
            wt_hwnd: HWND::default(),
            wt_runtime_id: String::new(),
            icon_path: String::new(),
            user_prompt: String::new(),
        }
    }
}

/// Get the state file path for a session.
pub fn state_file_path(session_id: &str) -> std::path::PathBuf {
    let temp = std::env::temp_dir();
    temp.join(format!("claude-notify-{}.txt", session_id))
}

/// Save state to the state file (4 lines).
pub fn save_state(session_id: &str, hwnd: HWND, runtime_id: &str, icon_path: &str, prompt: &str) {
    let path = state_file_path(session_id);
    let hwnd_val = hwnd.0 as isize;
    let content = format!("{}\n{}\n{}\n{}", hwnd_val, runtime_id, icon_path, prompt);
    let _ = std::fs::write(&path, content);
}

/// Load state from the state file.
pub fn load_state(session_id: &str) -> State {
    let mut state = State::default();
    let path = state_file_path(session_id);

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return state,
    };

    let lines: Vec<&str> = content.lines().collect();

    // Line 1: HWND
    if let Some(line) = lines.first() {
        if let Ok(val) = line.trim().parse::<i64>() {
            let hwnd = HWND(val as *mut _);
            if unsafe { IsWindow(Some(hwnd)).as_bool() } {
                state.target_hwnd = hwnd;
                // Check if this is Windows Terminal
                let class = get_class_name(hwnd);
                if class == "CASCADIA_HOSTING_WINDOW_CLASS" {
                    state.wt_hwnd = hwnd;
                }
            }
        }
    }

    // Line 2: RuntimeId
    if let Some(line) = lines.get(1) {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            state.wt_runtime_id = trimmed.to_string();
        }
    }

    // Line 3: Caller exe path
    if let Some(line) = lines.get(2) {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            state.icon_path = trimmed.to_string();
        }
    }

    // Line 4: User prompt (may contain the rest of the file if there were newlines)
    if lines.len() > 3 {
        // Join remaining lines back (prompt may contain newlines)
        state.user_prompt = lines[3..].join("\n");
    }

    state
}

/// Delete the state file for a session.
pub fn delete_state(session_id: &str) {
    let path = state_file_path(session_id);
    let _ = std::fs::remove_file(&path);
}

fn get_class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..len as usize])
}

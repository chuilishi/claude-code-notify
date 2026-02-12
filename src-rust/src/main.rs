#![windows_subsystem = "windows"]

mod activate;
mod assets;
mod cli;
mod json;
mod log;
mod process;
mod spawn;
mod state;
mod toast;
mod uiautomation;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::*;
use windows::Win32::UI::WindowsAndMessaging::*;

fn print_usage() {
    unsafe {
        let _ = windows::Win32::System::Console::AllocConsole();
    }
    println!(
        "Usage:\n  \
         ToastWindow.exe --save      Save window state (UserPromptSubmit hook)\n  \
         ToastWindow.exe --notify    Show notification (Stop hook)\n  \
         ToastWindow.exe --input     Show input-required notification (Notification hook)\n\n\
         Both modes read session_id from stdin JSON for state file isolation."
    );
}

fn get_class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..len as usize])
}

fn exe_path() -> String {
    std::env::current_exe()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn run_save_mode(immediate_hwnd: HWND) -> i32 {
    let input = json::read_stdin_json();
    let session_id = json::extract_string(&input, "session_id");
    let prompt = json::extract_string(&input, "prompt");

    if session_id.is_empty() {
        debug_log!("No session_id, skipping save");
        return 0;
    }

    debug_log!("Session ID: {}", session_id);
    debug_log!("Prompt: {}", prompt);

    // Use immediate_hwnd, fall back to GetForegroundWindow if invalid (SPEC 3.2)
    let hwnd = if !immediate_hwnd.is_invalid()
        && immediate_hwnd != HWND::default()
        && unsafe { IsWindow(Some(immediate_hwnd)).as_bool() }
    {
        debug_log!("Using immediate HWND: {:?}", immediate_hwnd);
        immediate_hwnd
    } else {
        let fallback = unsafe { GetForegroundWindow() };
        debug_log!("Immediate HWND invalid, using fallback: {:?}", fallback);
        fallback
    };

    // Detect Windows Terminal and get RuntimeId
    let mut runtime_id = String::new();
    let class = get_class_name(hwnd);
    debug_log!("Window class: {}", class);

    if class == "CASCADIA_HOSTING_WINDOW_CLASS" {
        debug_log!("Detected Windows Terminal, capturing tab RuntimeId");
        runtime_id = uiautomation::get_selected_tab_runtime_id(hwnd);
        debug_log!("RuntimeId: {}", runtime_id);
    }

    // Find caller exe path for icon extraction
    let caller_path = process::find_caller_exe_path();
    debug_log!("Caller exe path: {}", caller_path);

    // Save state
    state::save_state(&session_id, hwnd, &runtime_id, &caller_path, &prompt);
    debug_log!("State saved to {:?}", state::state_file_path(&session_id));

    0
}

fn run_notify_mode(debug: bool) -> i32 {
    let input = json::read_stdin_json();
    let session_id = json::extract_string(&input, "session_id");

    if session_id.is_empty() {
        debug_log!("No session_id for notify mode");
        return 1;
    }

    debug_log!("Notify mode, session: {}", session_id);

    let mut cmd = format!("\"{}\" --notify-show --session \"{}\"", exe_path(), session_id);
    if debug {
        cmd.push_str(" --debug");
    }

    debug_log!("Spawning: {}", cmd);
    spawn::spawn_detached(&cmd);
    0
}

fn run_input_mode(debug: bool) -> i32 {
    let input = json::read_stdin_json();
    let session_id = json::extract_string(&input, "session_id");
    let message = json::extract_string(&input, "message");

    if session_id.is_empty() {
        debug_log!("No session_id for input mode");
        return 1;
    }

    debug_log!("Input mode, session: {}, message: {}", session_id, message);

    let mut cmd = format!(
        "\"{}\" --notify-show --input-mode --session \"{}\"",
        exe_path(),
        session_id
    );
    if !message.is_empty() {
        // Escape quotes in message (SPEC 16.2)
        let escaped = message.replace('"', "\\\"");
        cmd.push_str(&format!(" --message \"{}\"", escaped));
    }
    if debug {
        cmd.push_str(" --debug");
    }

    debug_log!("Spawning: {}", cmd);
    spawn::spawn_detached(&cmd);
    0
}

fn run_cleanup_mode() -> i32 {
    let input = json::read_stdin_json();
    let session_id = json::extract_string(&input, "session_id");

    if !session_id.is_empty() {
        debug_log!("Cleanup: deleting state for session {}", session_id);
        state::delete_state(&session_id);
    }
    0
}

fn run_notify_show_mode(args: &cli::Args) -> i32 {
    if args.session.is_empty() {
        debug_log!("No session ID for notify-show mode");
        return 1;
    }

    debug_log!("NotifyShow mode, session: {}", args.session);

    // 1. Load state from file
    let st = state::load_state(&args.session);
    debug_log!("Loaded state: HWND={:?}, RuntimeId={}, IconPath={}, Prompt={}",
        st.target_hwnd, st.wt_runtime_id, st.icon_path, st.user_prompt);

    // 2. Determine notification content (SPEC 14.1-14.2)
    let (title, message) = if args.input_mode {
        let msg = if !args.message.is_empty() {
            args.message.clone()
        } else {
            "Claude needs your input".to_string()
        };
        ("Input Required".to_string(), msg)
    } else {
        let msg = if !st.user_prompt.is_empty() {
            st.user_prompt.clone()
        } else {
            "Task completed".to_string()
        };
        ("Claude Code".to_string(), msg)
    };

    // 3. Sanitize message (SPEC 14.3)
    let message = sanitize_message(&message);
    debug_log!("Title: {}, Message: {}", title, message);

    // 4. Discover assets
    let discovered = assets::discover_assets();
    debug_log!("Sound: {:?}, Font: {:?}, Icon: {:?}",
        discovered.sound_file, discovered.font_file, discovered.default_icon_path);

    // 5. Extract icon from saved exe path
    let icon = assets::extract_icon(&st.icon_path);
    debug_log!("App icon: {:?}", icon);

    // 6. Load custom font
    let font_family = if let Some(ref font_path) = discovered.font_file {
        assets::load_font(font_path).unwrap_or_else(|| "Segoe UI".to_string())
    } else {
        "Segoe UI".to_string()
    };
    debug_log!("Font family: {}", font_family);

    // 7. Play sound
    assets::play_sound(&discovered.sound_file);

    // 8. Show toast (blocks until closed)
    toast::show_toast(toast::ToastParams {
        title,
        message,
        input_mode: args.input_mode,
        font_family,
        icon,
        default_icon_path: discovered.default_icon_path.unwrap_or_default(),
        target_hwnd: st.target_hwnd,
        wt_hwnd: st.wt_hwnd,
        wt_runtime_id: st.wt_runtime_id,
    });

    // 9. Cleanup
    if !icon.is_invalid() {
        unsafe { let _ = windows::Win32::UI::WindowsAndMessaging::DestroyIcon(icon); }
    }
    if let Some(ref font_path) = discovered.font_file {
        assets::unload_font(font_path);
    }

    0
}

fn sanitize_message(msg: &str) -> String {
    // Replace newlines with space
    let mut s: String = msg.chars().map(|c| {
        if c == '\n' || c == '\r' { ' ' } else { c }
    }).collect();

    // Truncate at 35 chars + "..."
    if s.chars().count() > 35 {
        s = s.chars().take(35).collect::<String>() + "...";
    }
    s
}

fn main() {
    // CRITICAL: Capture foreground window IMMEDIATELY (SPEC 3.1)
    let immediate_hwnd = unsafe { GetForegroundWindow() };

    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let args = cli::parse_args();
    log::init(args.debug);

    let exit_code = match args.mode {
        cli::Mode::Save => run_save_mode(immediate_hwnd),
        cli::Mode::Notify => run_notify_mode(args.debug),
        cli::Mode::Input => run_input_mode(args.debug),
        cli::Mode::NotifyShow => run_notify_show_mode(&args),
        cli::Mode::Cleanup => run_cleanup_mode(),
        cli::Mode::None => {
            print_usage();
            1
        }
    };

    unsafe {
        CoUninitialize();
    }
    std::process::exit(exit_code);
}

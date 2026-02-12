//! Toast window: rendering, WndProc, timers, mouse interaction, stacking.
//!
//! Implements the full toast notification window with GDI drawing,
//! fade-out animation, Telegram-style stacking, and click-to-activate.

use std::cell::RefCell;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TRACKMOUSEEVENT, TME_LEAVE};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// --- Constants (SPEC Sections 8.2, 8.3, 10.1, 10.2) ---

const WINDOW_WIDTH: i32 = 300;
const WINDOW_HEIGHT: i32 = 80;
const ICON_SIZE: i32 = 48;
const ICON_PADDING: i32 = 16;
const CLOSE_BUTTON_SIZE: i32 = 20;
const CLOSE_BUTTON_MARGIN: i32 = 6;
const BORDER_WIDTH: i32 = 2;

const COLOR_BG: u32 = 0x00333333;
const COLOR_BORDER_NORMAL: u32 = 0x004B64B2;
const COLOR_BORDER_INPUT: u32 = 0x0000CFCF;
const COLOR_TITLE: u32 = 0x00FFFFFF;
const COLOR_MESSAGE: u32 = 0x00CCCCCC;
const COLOR_CLOSE: u32 = 0x00888888;

const TIMER_FADE: usize = 1;
const TIMER_START_FADE: usize = 2;
const TIMER_REPOSITION: usize = 3;
const TIMER_CHECK_BOTTOM: usize = 4;

const DISPLAY_MS: u32 = 3000;
const FADE_MS: u32 = 1000;
const INITIAL_ALPHA: u8 = 230;

const TOAST_CLASS_NAME: &str = "ClaudeCodeToast";

const WM_TOAST_CHECK_POSITION: u32 = WM_USER + 101;
const WM_TOAST_PAUSE_TIMER: u32 = WM_USER + 102;
const WM_MOUSELEAVE: u32 = 0x02A3;

// --- Global state for the toast window (per-process, one toast per process) ---

struct ToastState {
    hwnd: HWND,
    title: String,
    message: String,
    input_mode: bool,
    font_family: String,
    icon: HICON,
    default_icon_path: String,
    // Activation targets
    target_hwnd: HWND,
    wt_hwnd: HWND,
    wt_runtime_id: String,
    // Fade state
    alpha: u8,
    fade_step: u8,
    is_fading: bool,
    // Mouse state
    mouse_inside: bool,
    // Stacking state
    target_y: i32,
    is_bottom_toast: bool,
    taskbar_edge: u32,
    // Clicked flag
    clicked: bool,
}

thread_local! {
    static TOAST: RefCell<Option<ToastState>> = const { RefCell::new(None) };
}

/// Execute a closure with an immutable reference to the toast state.
fn with_toast<R>(f: impl FnOnce(&ToastState) -> R) -> R {
    TOAST.with(|cell| {
        let borrow = cell.borrow();
        f(borrow.as_ref().unwrap())
    })
}

/// Execute a closure with a mutable reference to the toast state.
fn with_toast_mut<R>(f: impl FnOnce(&mut ToastState) -> R) -> R {
    TOAST.with(|cell| {
        let mut borrow = cell.borrow_mut();
        f(borrow.as_mut().unwrap())
    })
}

fn make_font(height: i32, bold: bool, family: &str) -> HFONT {
    let face = crate::util::encode_wide(family);
    unsafe {
        CreateFontW(
            height, 0, 0, 0,
            if bold { FW_BOLD.0 as i32 } else { FW_NORMAL.0 as i32 },
            0, 0, 0,
            FONT_CHARSET(0),
            FONT_OUTPUT_PRECISION(0),
            FONT_CLIP_PRECISION(0),
            FONT_QUALITY(0),
            0,
            PCWSTR(face.as_ptr()),
        )
    }
}

fn is_point_in_close_button(x: i32, y: i32) -> bool {
    let btn_left = WINDOW_WIDTH - CLOSE_BUTTON_MARGIN - CLOSE_BUTTON_SIZE;
    let btn_top = CLOSE_BUTTON_MARGIN;
    x >= btn_left && x <= btn_left + CLOSE_BUTTON_SIZE
        && y >= btn_top && y <= btn_top + CLOSE_BUTTON_SIZE
}

// --- Stacking helpers ---

struct ToastInfo {
    hwnd: HWND,
    rect: RECT,
}

fn enum_other_toasts() -> Vec<ToastInfo> {
    let mut toasts: Vec<ToastInfo> = Vec::new();

    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let toasts = &mut *(lparam.0 as *mut Vec<ToastInfo>);
        let my_hwnd = TOAST.with(|cell| {
            cell.borrow().as_ref().map(|t| t.hwnd).unwrap_or_default()
        });
        if hwnd == my_hwnd {
            return TRUE;
        }
        let mut class_buf = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut class_buf);
        let class = String::from_utf16_lossy(&class_buf[..len as usize]);
        if class == TOAST_CLASS_NAME && IsWindowVisible(hwnd).as_bool() {
            let mut rect = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rect);
            toasts.push(ToastInfo { hwnd, rect });
        }
        TRUE
    }

    unsafe {
        let _ = EnumWindows(
            Some(callback),
            LPARAM(&mut toasts as *mut Vec<ToastInfo> as isize),
        );
    }

    toasts
}

fn calculate_position(work_area: &RECT, taskbar_edge: u32) -> (i32, i32) {
    let other_toasts = enum_other_toasts();

    // X position
    let x = if taskbar_edge == ABE_LEFT as u32 {
        work_area.left
    } else {
        work_area.right - WINDOW_WIDTH
    };

    // Y position
    let y = if other_toasts.is_empty() {
        // First toast
        if taskbar_edge == ABE_TOP as u32 {
            work_area.top
        } else {
            work_area.bottom - WINDOW_HEIGHT
        }
    } else {
        if taskbar_edge == ABE_TOP as u32 {
            // Stack below: find lowest bottom
            let lowest_bottom = other_toasts.iter().map(|t| t.rect.bottom).max().unwrap_or(work_area.top);
            lowest_bottom
        } else {
            // Stack above: find highest top
            let highest_top = other_toasts.iter().map(|t| t.rect.top).min().unwrap_or(work_area.bottom);
            highest_top - WINDOW_HEIGHT
        }
    };

    (x, y)
}

fn is_bottom_toast_check(hwnd: HWND, _taskbar_edge: u32) -> bool {
    let other_toasts = enum_other_toasts();
    if other_toasts.is_empty() {
        return true;
    }

    // Bottom toast = the one with the lowest HWND value (created earliest, closest to taskbar)
    for t in &other_toasts {
        if (t.hwnd.0 as usize) < (hwnd.0 as usize) {
            return false;
        }
    }
    true
}

fn notify_other_toasts_closing(my_hwnd: HWND) {
    let mut my_rect = RECT::default();
    unsafe { let _ = GetWindowRect(my_hwnd, &mut my_rect); }

    let others = enum_other_toasts();
    for t in &others {
        unsafe {
            let _ = SendMessageW(
                t.hwnd,
                WM_TOAST_CHECK_POSITION,
                Some(WPARAM(my_rect.top as usize)),
                Some(LPARAM(0)),
            );
        }
    }
}

fn notify_all_toasts_pause_timer(pause: bool) {
    let my_hwnd = with_toast(|t| t.hwnd);
    // Send to self
    unsafe {
        let _ = SendMessageW(
            my_hwnd,
            WM_TOAST_PAUSE_TIMER,
            Some(WPARAM(if pause { 1 } else { 0 })),
            Some(LPARAM(0)),
        );
    }
    // Send to others
    let others = enum_other_toasts();
    for t in &others {
        unsafe {
            let _ = SendMessageW(
                t.hwnd,
                WM_TOAST_PAUSE_TIMER,
                Some(WPARAM(if pause { 1 } else { 0 })),
                Some(LPARAM(0)),
            );
        }
    }
}

fn animate_to_position(hwnd: HWND) {
    let mut rect = RECT::default();
    unsafe { let _ = GetWindowRect(hwnd, &mut rect); }

    let (target_y, done) = with_toast_mut(|state| {
        let current_y = rect.top;
        let diff = state.target_y - current_y;

        if diff == 0 {
            return (0, true);
        }

        let mut step = diff * 2 / 5;
        if step == 0 {
            step = if diff > 0 { 2 } else { -2 };
        }

        let mut new_y = current_y + step;
        if (state.target_y - new_y).abs() < 4 {
            new_y = state.target_y;
        }

        (new_y, new_y == state.target_y)
    });

    if done && target_y == 0 {
        // diff was 0, just kill timer
        unsafe { let _ = KillTimer(Some(hwnd), TIMER_REPOSITION); }
        return;
    }

    unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            rect.left, target_y, 0, 0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }

    if done {
        unsafe { let _ = KillTimer(Some(hwnd), TIMER_REPOSITION); }
    }
}

// --- WndProc ---

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }

        WM_TIMER => {
            match wparam.0 {
                TIMER_START_FADE => {
                    let _ = KillTimer(Some(hwnd), TIMER_START_FADE);
                    with_toast_mut(|state| state.is_fading = true);
                    SetTimer(Some(hwnd), TIMER_FADE, 16, None);
                }
                TIMER_FADE => {
                    let should_destroy = with_toast_mut(|state| {
                        if state.alpha > state.fade_step {
                            state.alpha -= state.fade_step;
                            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), state.alpha, LWA_ALPHA);
                            false
                        } else {
                            state.is_fading = false;
                            true
                        }
                    });
                    if should_destroy {
                        let _ = KillTimer(Some(hwnd), TIMER_FADE);
                        notify_other_toasts_closing(hwnd);
                        let _ = DestroyWindow(hwnd);
                    }
                }
                TIMER_REPOSITION => {
                    animate_to_position(hwnd);
                }
                TIMER_CHECK_BOTTOM => {
                    let taskbar_edge = with_toast(|s| s.taskbar_edge);
                    if is_bottom_toast_check(hwnd, taskbar_edge) {
                        with_toast_mut(|state| {
                            state.is_bottom_toast = true;
                        });
                        let _ = KillTimer(Some(hwnd), TIMER_CHECK_BOTTOM);
                        SetTimer(Some(hwnd), TIMER_START_FADE, DISPLAY_MS, None);
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

            if is_point_in_close_button(x, y) {
                // Close button click
                let _ = KillTimer(Some(hwnd), TIMER_START_FADE);
                let _ = KillTimer(Some(hwnd), TIMER_FADE);
                notify_other_toasts_closing(hwnd);
                let _ = DestroyWindow(hwnd);
            } else {
                // Body click: activate window
                let _ = KillTimer(Some(hwnd), TIMER_START_FADE);
                let _ = KillTimer(Some(hwnd), TIMER_FADE);
                notify_other_toasts_closing(hwnd);
                let _ = ShowWindow(hwnd, SW_HIDE);

                let (target, wt, rid) = with_toast_mut(|state| {
                    state.clicked = true;
                    (state.target_hwnd, state.wt_hwnd, state.wt_runtime_id.clone())
                });
                crate::activate::activate_window(target, wt, &rid);

                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }

        WM_RBUTTONUP => {
            // Right click: close without activation
            let _ = KillTimer(Some(hwnd), TIMER_START_FADE);
            let _ = KillTimer(Some(hwnd), TIMER_FADE);
            notify_other_toasts_closing(hwnd);
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }

        WM_MOUSEMOVE => {
            let was_inside = with_toast(|s| s.mouse_inside);
            if !was_inside {
                with_toast_mut(|state| state.mouse_inside = true);
                // Track mouse leave
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut tme);

                // Pause all toasts
                notify_all_toasts_pause_timer(true);
            }
            LRESULT(0)
        }

        WM_MOUSELEAVE => {
            with_toast_mut(|state| state.mouse_inside = false);
            // Resume all toasts
            notify_all_toasts_pause_timer(false);
            LRESULT(0)
        }

        x if x == WM_TOAST_CHECK_POSITION => {
            let closed_toast_y = wparam.0 as i32;
            let mut my_rect = RECT::default();
            let _ = GetWindowRect(hwnd, &mut my_rect);

            let taskbar_edge = with_toast(|s| s.taskbar_edge);

            // Update position target (borrow released before is_bottom_toast_check)
            with_toast_mut(|state| {
                if state.taskbar_edge == ABE_TOP as u32 {
                    // Top taskbar: if we're below the closed toast, move up
                    if my_rect.top > closed_toast_y {
                        state.target_y = my_rect.top - WINDOW_HEIGHT;
                        SetTimer(Some(hwnd), TIMER_REPOSITION, 16, None);
                    }
                } else {
                    // Bottom taskbar: if we're above the closed toast, move down
                    if my_rect.top < closed_toast_y {
                        state.target_y = my_rect.top + WINDOW_HEIGHT;
                        SetTimer(Some(hwnd), TIMER_REPOSITION, 16, None);
                    }
                }
            });

            // Check bottom toast status OUTSIDE the borrow to avoid double-borrow
            // (is_bottom_toast_check -> enum_other_toasts -> callback borrows TOAST)
            let is_already_bottom = with_toast(|s| s.is_bottom_toast);
            if !is_already_bottom && is_bottom_toast_check(hwnd, taskbar_edge) {
                with_toast_mut(|state| {
                    state.is_bottom_toast = true;
                    let _ = KillTimer(Some(hwnd), TIMER_CHECK_BOTTOM);
                    if !state.mouse_inside {
                        SetTimer(Some(hwnd), TIMER_START_FADE, DISPLAY_MS, None);
                    }
                });
            }

            LRESULT(0)
        }

        x if x == WM_TOAST_PAUSE_TIMER => {
            let pause = wparam.0 == 1;

            with_toast_mut(|state| {
                if pause {
                    if state.is_fading {
                        let _ = KillTimer(Some(hwnd), TIMER_FADE);
                        state.is_fading = false;
                        state.alpha = INITIAL_ALPHA;
                        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), INITIAL_ALPHA, LWA_ALPHA);
                    }
                    let _ = KillTimer(Some(hwnd), TIMER_START_FADE);
                } else {
                    // Resume: only start fade timer if bottom toast and mouse not inside
                    if state.is_bottom_toast && !state.mouse_inside {
                        SetTimer(Some(hwnd), TIMER_START_FADE, DISPLAY_MS, None);
                    }
                }
            });
            LRESULT(0)
        }

        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// --- Paint ---

unsafe fn paint(hwnd: HWND) {
    let (title, message, input_mode, font_family, icon, default_icon_path) = with_toast(|state| {
        (
            state.title.clone(),
            state.message.clone(),
            state.input_mode,
            state.font_family.clone(),
            state.icon,
            state.default_icon_path.clone(),
        )
    });

    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);

    // Background
    let bg = CreateSolidBrush(COLORREF(COLOR_BG));
    let rect = RECT { left: 0, top: 0, right: WINDOW_WIDTH, bottom: WINDOW_HEIGHT };
    FillRect(hdc, &rect, bg);
    let _ = DeleteObject(HGDIOBJ(bg.0));

    // Border (color depends on input mode)
    let border_color = if input_mode { COLOR_BORDER_INPUT } else { COLOR_BORDER_NORMAL };
    let border = CreateSolidBrush(COLORREF(border_color));
    let borders = [
        RECT { left: 0, top: 0, right: WINDOW_WIDTH, bottom: BORDER_WIDTH },
        RECT { left: 0, top: WINDOW_HEIGHT - BORDER_WIDTH, right: WINDOW_WIDTH, bottom: WINDOW_HEIGHT },
        RECT { left: 0, top: 0, right: BORDER_WIDTH, bottom: WINDOW_HEIGHT },
        RECT { left: WINDOW_WIDTH - BORDER_WIDTH, top: 0, right: WINDOW_WIDTH, bottom: WINDOW_HEIGHT },
    ];
    for b in &borders {
        FillRect(hdc, b, border);
    }
    let _ = DeleteObject(HGDIOBJ(border.0));

    // Icon
    let icon_x = ICON_PADDING;
    let icon_y = (WINDOW_HEIGHT - ICON_SIZE) / 2;
    if !icon.is_invalid() {
        let _ = DrawIconEx(
            hdc, icon_x, icon_y,
            icon,
            ICON_SIZE, ICON_SIZE,
            0, None, DI_NORMAL,
        );
    } else if !default_icon_path.is_empty() {
        let path_wide = crate::util::encode_wide(&default_icon_path);
        let result = LoadImageW(
            None,
            PCWSTR(path_wide.as_ptr()),
            IMAGE_ICON,
            ICON_SIZE, ICON_SIZE,
            LR_LOADFROMFILE,
        );
        if let Ok(handle) = result {
            let h_icon = HICON(handle.0);
            if !h_icon.is_invalid() {
                let _ = DrawIconEx(hdc, icon_x, icon_y, h_icon, ICON_SIZE, ICON_SIZE, 0, None, DI_NORMAL);
                let _ = DestroyIcon(h_icon);
            }
        }
    }

    // Text setup
    SetBkMode(hdc, TRANSPARENT);

    let text_left = icon_x + ICON_SIZE + ICON_PADDING;

    // Title
    SetTextColor(hdc, COLORREF(COLOR_TITLE));
    let title_font = make_font(18, true, &font_family);
    let old = SelectObject(hdc, HGDIOBJ(title_font.0));
    let mut title_rect = RECT { left: text_left, top: 15, right: WINDOW_WIDTH - 10, bottom: 40 };
    let mut title_buf = crate::util::encode_wide(&title);
    let title_len = title_buf.len() - 1; // exclude null terminator
    DrawTextW(hdc, &mut title_buf[..title_len], &mut title_rect, DRAW_TEXT_FORMAT(0));
    SelectObject(hdc, old);
    let _ = DeleteObject(HGDIOBJ(title_font.0));

    // Message
    SetTextColor(hdc, COLORREF(COLOR_MESSAGE));
    let msg_font = make_font(14, false, &font_family);
    let old = SelectObject(hdc, HGDIOBJ(msg_font.0));
    let mut msg_rect = RECT { left: text_left, top: 42, right: WINDOW_WIDTH - 10, bottom: WINDOW_HEIGHT - 10 };
    let mut msg_buf = crate::util::encode_wide(&message);
    let msg_len = msg_buf.len() - 1; // exclude null terminator
    DrawTextW(hdc, &mut msg_buf[..msg_len], &mut msg_rect, DRAW_TEXT_FORMAT(0));
    SelectObject(hdc, old);
    let _ = DeleteObject(HGDIOBJ(msg_font.0));

    // Close button (always Segoe UI)
    SetTextColor(hdc, COLORREF(COLOR_CLOSE));
    let close_font = make_font(16, true, "Segoe UI");
    let old = SelectObject(hdc, HGDIOBJ(close_font.0));
    let btn_left = WINDOW_WIDTH - CLOSE_BUTTON_MARGIN - CLOSE_BUTTON_SIZE;
    let mut close_rect = RECT {
        left: btn_left,
        top: CLOSE_BUTTON_MARGIN,
        right: btn_left + CLOSE_BUTTON_SIZE,
        bottom: CLOSE_BUTTON_MARGIN + CLOSE_BUTTON_SIZE,
    };
    let mut close_buf = crate::util::encode_wide("\u{00D7}");
    let close_len = close_buf.len() - 1;
    DrawTextW(
        hdc,
        &mut close_buf[..close_len],
        &mut close_rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    SelectObject(hdc, old);
    let _ = DeleteObject(HGDIOBJ(close_font.0));

    let _ = EndPaint(hwnd, &ps);
}

// --- Public API ---

pub struct ToastParams {
    pub title: String,
    pub message: String,
    pub input_mode: bool,
    pub font_family: String,
    pub icon: HICON,
    pub default_icon_path: String,
    pub target_hwnd: HWND,
    pub wt_hwnd: HWND,
    pub wt_runtime_id: String,
}

/// Show the toast notification window. Blocks until the window is closed.
pub fn show_toast(params: ToastParams) {
    // Calculate fade step (SPEC 10.3)
    let fade_ticks = (FADE_MS / 16).max(1);
    let fade_step = ((INITIAL_ALPHA as u32 / fade_ticks) + 1).min(255) as u8;

    // Detect taskbar position
    let taskbar_edge = detect_taskbar_edge();

    // Get work area from cursor's monitor
    let (work_area, _monitor) = get_cursor_monitor_work_area();

    TOAST.with(|cell| {
        *cell.borrow_mut() = Some(ToastState {
            hwnd: HWND::default(),
            title: params.title,
            message: params.message,
            input_mode: params.input_mode,
            font_family: params.font_family,
            icon: params.icon,
            default_icon_path: params.default_icon_path,
            target_hwnd: params.target_hwnd,
            wt_hwnd: params.wt_hwnd,
            wt_runtime_id: params.wt_runtime_id,
            alpha: INITIAL_ALPHA,
            fade_step,
            is_fading: false,
            mouse_inside: false,
            target_y: 0,
            is_bottom_toast: false,
            taskbar_edge,
            clicked: false,
        });
    });

    unsafe {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        let class_wide = crate::util::encode_wide(TOAST_CLASS_NAME);

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_wide.as_ptr()),
            hCursor: LoadCursorW(None, IDC_HAND).unwrap_or_default(),
            ..Default::default()
        };

        // OK if already registered by another toast instance
        let _ = RegisterClassExW(&wc);

        let (x, y) = calculate_position(&work_area, taskbar_edge);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE,
            PCWSTR(class_wide.as_ptr()),
            w!("Toast"),
            WS_POPUP,
            x, y, WINDOW_WIDTH, WINDOW_HEIGHT,
            None, None, Some(instance.into()), None,
        ).unwrap_or_default();

        if hwnd.is_invalid() || hwnd == HWND::default() {
            crate::debug_log!("CreateWindowExW failed");
            return;
        }

        with_toast_mut(|state| state.hwnd = hwnd);

        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), INITIAL_ALPHA, LWA_ALPHA);

        // Determine if bottom toast and start appropriate timer
        if is_bottom_toast_check(hwnd, taskbar_edge) {
            with_toast_mut(|state| state.is_bottom_toast = true);
            SetTimer(Some(hwnd), TIMER_START_FADE, DISPLAY_MS, None);
        } else {
            with_toast_mut(|state| state.is_bottom_toast = false);
            SetTimer(Some(hwnd), TIMER_CHECK_BOTTOM, 200, None);
        }

        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = UpdateWindow(hwnd);

        // Message loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn detect_taskbar_edge() -> u32 {
    let mut abd = APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        ..Default::default()
    };

    let result = unsafe { SHAppBarMessage(ABM_GETTASKBARPOS, &mut abd) };
    if result != 0 {
        abd.uEdge
    } else {
        ABE_BOTTOM as u32
    }
}

fn get_cursor_monitor_work_area() -> (RECT, HMONITOR) {
    unsafe {
        let mut cursor_pos = POINT::default();
        let _ = GetCursorPos(&mut cursor_pos);

        let monitor = MonitorFromPoint(cursor_pos, MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(monitor, &mut mi);

        (mi.rcWork, monitor)
    }
}

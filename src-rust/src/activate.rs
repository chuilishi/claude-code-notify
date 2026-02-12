//! Window activation with focus-stealing workaround.
//!
//! Implements the ALT key trick and full 12-step activation sequence
//! for both regular windows and Windows Terminal tabs.

use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::uiautomation;

/// Activate the saved window. If it's a WT window with a saved RuntimeId,
/// switch to the correct tab.
pub fn activate_window(
    target: HWND,
    wt_hwnd: HWND,
    wt_runtime_id: &str,
) {
    if !wt_hwnd.is_invalid()
        && wt_hwnd != HWND::default()
        && !wt_runtime_id.is_empty()
    {
        crate::debug_log!("Activating WT window with tab switch");
        switch_to_wt_tab(wt_hwnd, wt_runtime_id);
    } else if !target.is_invalid()
        && target != HWND::default()
        && unsafe { IsWindow(Some(target)).as_bool() }
    {
        crate::debug_log!("Activating regular window: {:?}", target);
        activate_hwnd(target);
    } else {
        crate::debug_log!("No valid target window to activate");
    }
}

fn switch_to_wt_tab(wt_hwnd: HWND, runtime_id: &str) {
    if !unsafe { IsWindow(Some(wt_hwnd)).as_bool() } {
        crate::debug_log!("WT window no longer valid");
        return;
    }

    // Restore if minimized
    if unsafe { IsIconic(wt_hwnd).as_bool() } {
        unsafe { let _ = ShowWindow(wt_hwnd, SW_RESTORE); }
    }

    // Bring WT window to foreground
    activate_hwnd(wt_hwnd);

    // Switch to the correct tab via UI Automation
    if uiautomation::select_tab_by_runtime_id(wt_hwnd, runtime_id) {
        crate::debug_log!("WT tab selected successfully");
    } else {
        crate::debug_log!("WT tab not found (may have been closed)");
    }
}

/// Full 12-step activation sequence (SPEC 7.2).
fn activate_hwnd(target: HWND) {
    unsafe {
        // Step 1: Allow any process to set foreground
        let _ = AllowSetForegroundWindow(ASFW_ANY);

        // Step 2: Restore if minimized
        if IsIconic(target).as_bool() {
            let _ = ShowWindow(target, SW_RESTORE);
        }

        // Step 3: ALT key trick
        try_alt_key_trick();

        // Steps 4-6: Get thread IDs
        let fg_hwnd = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg_hwnd, None);
        let cur_thread = GetCurrentThreadId();
        let target_thread = GetWindowThreadProcessId(target, None);

        // Steps 7-8: Attach thread input
        let _ = AttachThreadInput(cur_thread, fg_thread, true);
        let _ = AttachThreadInput(cur_thread, target_thread, true);

        // Step 9: Set window position to top
        let _ = SetWindowPos(
            target,
            Some(HWND_TOP),
            0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );

        // Step 10: Bring to top
        let _ = BringWindowToTop(target);

        // Step 11: SwitchToThisWindow (undocumented but effective)
        SwitchToThisWindow(target, true);

        // Step 12: Set foreground
        let _ = SetForegroundWindow(target);

        // Detach thread input
        let _ = AttachThreadInput(cur_thread, target_thread, false);
        let _ = AttachThreadInput(cur_thread, fg_thread, false);
    }
}

/// Simulate ALT key press/release to trick Windows into allowing
/// foreground window changes (SPEC 7.1).
fn try_alt_key_trick() {
    unsafe {
        keybd_event(
            VK_MENU.0 as u8,
            0,
            KEYEVENTF_EXTENDEDKEY,
            0,
        );
        keybd_event(
            VK_MENU.0 as u8,
            0,
            KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP,
            0,
        );
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
}

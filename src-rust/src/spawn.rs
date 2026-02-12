//! Detached child process spawning.
//!
//! Uses CreateProcessW with CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS
//! to spawn a child that outlives the parent.

use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::PWSTR;

/// Spawn a detached child process with the given command line.
/// Returns true on success.
pub fn spawn_detached(cmd_line: &str) -> bool {
    let mut cmd_wide: Vec<u16> = cmd_line.encode_utf16().chain(std::iter::once(0)).collect();

    let si = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        dwFlags: STARTF_USESHOWWINDOW,
        wShowWindow: SW_HIDE.0 as u16,
        ..Default::default()
    };

    let mut pi = PROCESS_INFORMATION::default();

    let result = unsafe {
        CreateProcessW(
            None,
            Some(PWSTR(cmd_wide.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS,
            None,
            None,
            &si,
            &mut pi,
        )
    };

    match result {
        Ok(_) => {
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(pi.hProcess);
                let _ = windows::Win32::Foundation::CloseHandle(pi.hThread);
            }
            true
        }
        Err(_) => false,
    }
}

//! Shared utility functions.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;

/// Encode a Rust string as a null-terminated wide (UTF-16) string.
pub fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Get the window class name for a given HWND.
pub fn get_class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..len as usize])
}

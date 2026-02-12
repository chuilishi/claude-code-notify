//! Asset discovery, font loading, icon extraction, and sound playback.

use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::*;

fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Find the first file matching a pattern in a directory.
/// e.g., find_first_file("C:\\dir", "*.wav")
pub fn find_first_file(dir: &str, pattern: &str) -> Option<String> {
    let search = format!("{}\\{}", dir, pattern);
    let search_wide = encode_wide(&search);

    unsafe {
        let mut fd = WIN32_FIND_DATAW::default();
        let handle = FindFirstFileW(PCWSTR(search_wide.as_ptr()), &mut fd);
        match handle {
            Ok(h) => {
                let name_len = fd.cFileName.iter().position(|&c| c == 0).unwrap_or(fd.cFileName.len());
                let name = String::from_utf16_lossy(&fd.cFileName[..name_len]);
                let _ = FindClose(h);
                Some(format!("{}\\{}", dir, name))
            }
            Err(_) => None,
        }
    }
}

/// Get the directory containing the current executable.
pub fn exe_dir() -> String {
    std::env::current_exe()
        .unwrap_or_default()
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_string_lossy()
        .into_owned()
}

/// Discover asset paths relative to the exe directory.
pub struct Assets {
    pub sound_file: Option<String>,
    pub font_file: Option<String>,
    pub default_icon_path: Option<String>,
}

pub fn discover_assets() -> Assets {
    let dir = exe_dir();
    let sound_dir = format!("{}\\assets\\sound", dir);
    let font_dir = format!("{}\\assets\\fonts", dir);
    let img_dir = format!("{}\\assets\\img", dir);

    Assets {
        sound_file: find_first_file(&sound_dir, "*.wav"),
        font_file: find_first_file(&font_dir, "*.ttf")
            .or_else(|| find_first_file(&font_dir, "*.otf")),
        default_icon_path: find_first_file(&img_dir, "*.ico"),
    }
}

/// Load a custom font file as a private font. Returns the derived font family name.
pub fn load_font(font_path: &str) -> Option<String> {
    let path_wide = encode_wide(font_path);
    let result = unsafe {
        AddFontResourceExW(PCWSTR(path_wide.as_ptr()), FONT_RESOURCE_CHARACTERISTICS(0x10), None)
    };
    if result > 0 {
        Some(derive_font_family(font_path))
    } else {
        None
    }
}

/// Remove a previously loaded private font.
pub fn unload_font(font_path: &str) {
    let path_wide = encode_wide(font_path);
    unsafe {
        let _ = RemoveFontResourceExW(PCWSTR(path_wide.as_ptr()), 0x10, None);
    }
}

/// Derive font family name from filename (SPEC 13.3).
fn derive_font_family(path: &str) -> String {
    // Extract filename without directory
    let name = path
        .rsplit(|c| c == '\\' || c == '/')
        .next()
        .unwrap_or(path);

    // Remove extension
    let name = match name.rfind('.') {
        Some(pos) => &name[..pos],
        None => name,
    };

    // Remove known suffixes
    let suffixes = ["-Regular", "-Bold", "-Italic", "-Light", "-Medium"];
    let mut name = name.to_string();
    for suffix in &suffixes {
        if let Some(pos) = name.find(suffix) {
            name = name[..pos].to_string();
            break;
        }
    }

    // CamelCase split: insert space before uppercase that follows lowercase
    let mut result = String::new();
    let chars: Vec<char> = name.chars().collect();
    for i in 0..chars.len() {
        if i > 0 && chars[i].is_uppercase() && chars[i - 1].is_lowercase() {
            result.push(' ');
        }
        result.push(chars[i]);
    }

    result
}

/// Extract the large icon from an exe file (index 0).
pub fn extract_icon(exe_path: &str) -> HICON {
    if exe_path.is_empty() {
        return HICON::default();
    }

    let path_wide = encode_wide(exe_path);
    let mut large = HICON::default();
    let mut small = HICON::default();

    unsafe {
        let count = ExtractIconExW(
            PCWSTR(path_wide.as_ptr()),
            0,
            Some(&mut large),
            Some(&mut small),
            1,
        );

        if count > 0 {
            if !small.is_invalid() {
                let _ = DestroyIcon(small);
            }
        }
    }

    large
}

/// Play a notification sound (SPEC 12.2).
pub fn play_sound(wav_path: &Option<String>) {
    use windows::Win32::Media::Audio::*;

    if let Some(path) = wav_path {
        let path_wide = encode_wide(path);
        // Check file exists
        let attrs = unsafe { GetFileAttributesW(PCWSTR(path_wide.as_ptr())) };
        if attrs != INVALID_FILE_ATTRIBUTES {
            unsafe {
                let _ = PlaySoundW(
                    PCWSTR(path_wide.as_ptr()),
                    None,
                    SND_FILENAME | SND_ASYNC,
                );
            }
            return;
        }
    }

    // Fallback: system beep
    #[link(name = "user32")]
    extern "system" {
        fn MessageBeep(utype: u32) -> i32;
    }
    unsafe {
        MessageBeep(0x40); // MB_ICONASTERISK
    }
}

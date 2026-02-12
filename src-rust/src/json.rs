//! Stdin JSON reading and field extraction.
//!
//! Reads stdin in binary mode, parses as JSON via serde_json,
//! and extracts string fields.

use std::io::Read;

/// Read all of stdin into a String.
/// Mirrors the C++ ReadStdinJson() which reads in binary mode with fread in 4096 chunks.
pub fn read_stdin_json() -> String {
    let mut buf = Vec::new();
    let _ = std::io::stdin().lock().read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

/// Extract a string field from a JSON string.
/// Returns empty string if the field is not found or not a string.
pub fn extract_string(json: &str, key: &str) -> String {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    v.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

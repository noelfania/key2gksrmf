// JSON 설정 파일 읽기/쓰기 모듈
// 실행 파일과 같은 디렉터리의 config.json을 사용한다

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSize {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    #[serde(default = "default_window_width")]
    pub width: i32,
    #[serde(default = "default_window_height")]
    pub height: i32,
}

fn default_window_width() -> i32 {
    500
}

fn default_window_height() -> i32 {
    500
}

impl Default for WindowSize {
    fn default() -> Self {
        WindowSize {
            x: None,
            y: None,
            width: default_window_width(),
            height: default_window_height(),
        }
    }
}

fn default_word_wrap() -> bool {
    true
}

/// UI 색상 설정 ("#RRGGBB"). 없거나 파싱 실패 시 시스템 기본색을 사용한다.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ColorConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gutter_background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_bar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_background: Option<String>,
}

/// "#RRGGBB" 형식을 COLORREF 값(0x00BBGGRR)으로 파싱한다. 실패 시 None.
pub fn parse_hex_color(s: &str) -> Option<u32> {
    let hex = s.trim();
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u32::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u32::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u32::from_str_radix(&hex[4..6], 16).ok()?;
    Some((b << 16) | (g << 8) | r)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub always_on_top: bool,
    #[serde(default)]
    pub start_with_windows: bool,
    #[serde(default = "default_word_wrap")]
    pub word_wrap: bool,
    #[serde(default)]
    pub global_hotkey_open: bool,
    #[serde(default)]
    pub colors: ColorConfig,
    #[serde(default)]
    pub window: WindowSize,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            always_on_top: false,
            start_with_windows: false,
            word_wrap: default_word_wrap(),
            global_hotkey_open: false,
            colors: ColorConfig::default(),
            window: WindowSize::default(),
        }
    }
}

fn config_path() -> PathBuf {
    // 실행 파일 위치 기준 config.json
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return dir.join("config.json");
        }
    }
    PathBuf::from("config.json")
}

/// 설정 파일을 로드한다. 파일이 없거나 파싱 실패 시 기본값을 반환한다.
pub fn load() -> AppConfig {
    let path = config_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return AppConfig::default(),
    };
    match serde_json::from_str::<AppConfig>(&text) {
        Ok(cfg) => cfg,
        Err(_) => {
            // 파손된 설정 파일을 백업 후 기본값으로 복구
            let backup = path.with_extension("json.bak");
            let _ = std::fs::rename(&path, &backup);
            AppConfig::default()
        }
    }
}

/// 설정을 파일에 저장한다. 실패 시 false 반환.
pub fn save(cfg: &AppConfig) -> bool {
    let path = config_path();
    match serde_json::to_string_pretty(cfg) {
        Ok(text) => std::fs::write(&path, text).is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color_valid() {
        // COLORREF는 0x00BBGGRR 순서
        assert_eq!(parse_hex_color("#FFFFFF"), Some(0x00FF_FFFF));
        assert_eq!(parse_hex_color("#FF0000"), Some(0x0000_00FF));
        assert_eq!(parse_hex_color("#0000FF"), Some(0x00FF_0000));
        assert_eq!(parse_hex_color("f0f0f0"), Some(0x00F0_F0F0));
        assert_eq!(parse_hex_color(" #6E6E6E "), Some(0x006E_6E6E));
    }

    #[test]
    fn test_parse_hex_color_invalid_falls_back_to_none() {
        assert_eq!(parse_hex_color(""), None);
        assert_eq!(parse_hex_color("#FFF"), None);
        assert_eq!(parse_hex_color("#GGGGGG"), None);
        assert_eq!(parse_hex_color("#FFFFFF00"), None);
        assert_eq!(parse_hex_color("white"), None);
    }
}

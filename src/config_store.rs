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

pub const DEFAULT_GUTTER_BACKGROUND: &str = "#F3F3F3";
pub const DEFAULT_LINE_NUMBER: &str = "#8A8A8A";
pub const DEFAULT_STATUS_BAR: &str = "#F3F3F3";
pub const DEFAULT_EDITOR_BACKGROUND: &str = "#FFFFFF";

// 이전 버전 기본값. 사용자가 바꾸지 않은 값으로 보고 새 기본값으로 옮긴다.
const LEGACY_GUTTER_BACKGROUND: &str = "#F0F0F0";
const LEGACY_LINE_NUMBER: &str = "#6E6E6E";
const LEGACY_STATUS_BAR: &str = "#F0F0F0";

/// 라이트 모드 UI 색상 ("#RRGGBB"). 없거나 파싱 실패 시 내장 팔레트를 사용한다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorConfig {
    #[serde(default = "default_gutter_background")]
    pub gutter_background: Option<String>,
    #[serde(default = "default_line_number")]
    pub line_number: Option<String>,
    #[serde(default = "default_status_bar")]
    pub status_bar: Option<String>,
    #[serde(default = "default_editor_background")]
    pub editor_background: Option<String>,
}

fn default_gutter_background() -> Option<String> {
    Some(DEFAULT_GUTTER_BACKGROUND.to_string())
}

fn default_line_number() -> Option<String> {
    Some(DEFAULT_LINE_NUMBER.to_string())
}

fn default_status_bar() -> Option<String> {
    Some(DEFAULT_STATUS_BAR.to_string())
}

fn default_editor_background() -> Option<String> {
    Some(DEFAULT_EDITOR_BACKGROUND.to_string())
}

impl Default for ColorConfig {
    fn default() -> Self {
        ColorConfig {
            gutter_background: default_gutter_background(),
            line_number: default_line_number(),
            status_bar: default_status_bar(),
            editor_background: default_editor_background(),
        }
    }
}

/// 다크 모드 UI 색상 덮어쓰기 ("#RRGGBB"). 기본은 모두 비어 있고 내장 다크 팔레트를 쓴다.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DarkColorConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gutter_background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_bar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_background: Option<String>,
}

impl ColorConfig {
    /// 이전 버전 기본값을 새 기본값으로 바꾼다. 바뀐 게 있으면 true.
    pub fn migrate_legacy_defaults(&mut self) -> bool {
        let mut changed = false;
        let mut swap = |field: &mut Option<String>, legacy: &str, current: &str| {
            if field.as_deref().is_some_and(|v| v.eq_ignore_ascii_case(legacy)) {
                *field = Some(current.to_string());
                changed = true;
            }
        };
        swap(&mut self.gutter_background, LEGACY_GUTTER_BACKGROUND, DEFAULT_GUTTER_BACKGROUND);
        swap(&mut self.line_number, LEGACY_LINE_NUMBER, DEFAULT_LINE_NUMBER);
        swap(&mut self.status_bar, LEGACY_STATUS_BAR, DEFAULT_STATUS_BAR);
        changed
    }

    /// 누락된 색상 키에 기본 HEX를 채운다 (기존 `"colors": {}` 호환).
    pub fn fill_defaults(&mut self) {
        if self.gutter_background.is_none() {
            self.gutter_background = default_gutter_background();
        }
        if self.line_number.is_none() {
            self.line_number = default_line_number();
        }
        if self.status_bar.is_none() {
            self.status_bar = default_status_bar();
        }
        if self.editor_background.is_none() {
            self.editor_background = default_editor_background();
        }
    }
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

/// 창 투명도 상한(%). 불투명도 20%까지만 내려간다.
pub const MAX_TRANSPARENCY: u8 = 80;

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
    /// 창 투명도(%). 0 = 불투명, 최대 `MAX_TRANSPARENCY`.
    #[serde(default)]
    pub transparency: u8,
    #[serde(default)]
    pub colors: ColorConfig,
    #[serde(default)]
    pub colors_dark: DarkColorConfig,
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
            transparency: 0,
            colors: ColorConfig::default(),
            colors_dark: DarkColorConfig::default(),
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
/// 기본값/복구 시에는 바로 저장해 colors 키가 파일에 보이도록 한다.
pub fn load() -> AppConfig {
    let path = config_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => {
            let cfg = AppConfig::default();
            let _ = save(&cfg);
            return cfg;
        }
    };
    match serde_json::from_str::<AppConfig>(&text) {
        Ok(mut cfg) => {
            cfg.transparency = cfg.transparency.min(MAX_TRANSPARENCY);
            // 구버전 `"colors": {}` 등 누락 키를 기본 HEX로 채우고 저장
            let before = cfg.colors.clone();
            cfg.colors.fill_defaults();
            cfg.colors.migrate_legacy_defaults();
            if cfg.colors != before {
                let _ = save(&cfg);
            }
            cfg
        }
        Err(_) => {
            // 파손된 설정 파일을 백업 후 기본값으로 복구
            let backup = path.with_extension("json.bak");
            let _ = std::fs::rename(&path, &backup);
            let cfg = AppConfig::default();
            let _ = save(&cfg);
            cfg
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

    #[test]
    fn test_color_config_default_serializes_hex_keys() {
        let json = serde_json::to_string_pretty(&ColorConfig::default()).unwrap();
        assert!(json.contains(&format!("\"gutter_background\": \"{DEFAULT_GUTTER_BACKGROUND}\"")));
        assert!(json.contains(&format!("\"line_number\": \"{DEFAULT_LINE_NUMBER}\"")));
        assert!(json.contains(&format!("\"status_bar\": \"{DEFAULT_STATUS_BAR}\"")));
        assert!(json.contains(&format!("\"editor_background\": \"{DEFAULT_EDITOR_BACKGROUND}\"")));
    }

    #[test]
    fn test_legacy_default_colors_migrate_but_custom_colors_stay() {
        let mut colors = ColorConfig {
            gutter_background: Some("#f0f0f0".into()),
            line_number: Some("#6E6E6E".into()),
            status_bar: Some("#123456".into()),
            editor_background: Some("#FFFFFF".into()),
        };
        assert!(colors.migrate_legacy_defaults());
        assert_eq!(colors.gutter_background.as_deref(), Some(DEFAULT_GUTTER_BACKGROUND));
        assert_eq!(colors.line_number.as_deref(), Some(DEFAULT_LINE_NUMBER));
        assert_eq!(colors.status_bar.as_deref(), Some("#123456"));
        assert!(!colors.migrate_legacy_defaults());
    }

    #[test]
    fn test_colors_dark_defaults_to_empty_object() {
        let cfg: AppConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg.colors_dark, DarkColorConfig::default());
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("\"colors_dark\":{}"));
    }

    #[test]
    fn test_fill_defaults_fills_empty_colors() {
        let mut colors = ColorConfig {
            gutter_background: None,
            line_number: None,
            status_bar: None,
            editor_background: None,
        };
        colors.fill_defaults();
        assert_eq!(
            colors.gutter_background.as_deref(),
            Some(DEFAULT_GUTTER_BACKGROUND)
        );
        assert_eq!(colors.line_number.as_deref(), Some(DEFAULT_LINE_NUMBER));
        assert_eq!(colors.status_bar.as_deref(), Some(DEFAULT_STATUS_BAR));
        assert_eq!(
            colors.editor_background.as_deref(),
            Some(DEFAULT_EDITOR_BACKGROUND)
        );
    }

    #[test]
    fn test_deserialize_empty_colors_object_gets_defaults() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{ "always_on_top": false, "colors": {} }"#,
        )
        .unwrap();
        // serde default = 함수가 각 필드에 적용된다
        assert_eq!(cfg.transparency, 0);
        assert_eq!(
            cfg.colors.gutter_background.as_deref(),
            Some(DEFAULT_GUTTER_BACKGROUND)
        );
        assert_eq!(
            cfg.colors.editor_background.as_deref(),
            Some(DEFAULT_EDITOR_BACKGROUND)
        );
    }
}

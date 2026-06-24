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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub always_on_top: bool,
    #[serde(default)]
    pub start_with_windows: bool,
    #[serde(default)]
    pub window: WindowSize,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            always_on_top: false,
            start_with_windows: false,
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

pub const APP_NAME: &str = "key2gksrmf";
// 디버그 빌드는 별도 인스턴스로 떠서 실행 중인 릴리스 앱과 함께 띄워 볼 수 있다
#[cfg(not(debug_assertions))]
pub const APP_MUTEX_NAME: &str = "Local\\key2gksrmf_single_instance";
#[cfg(debug_assertions)]
pub const APP_MUTEX_NAME: &str = "Local\\key2gksrmf_single_instance_debug";
#[cfg(not(debug_assertions))]
pub const APP_WINDOW_CLASS: &str = "key2gksrmf_wnd";
#[cfg(debug_assertions)]
pub const APP_WINDOW_CLASS: &str = "key2gksrmf_wnd_debug";
pub const APP_OWNER_WINDOW_CLASS: &str = "key2gksrmf_owner_wnd";
pub const APP_HOMEPAGE: &str = "https://github.com/noelfania/key2gksrmf";

// 단축키 표시 문자열 (상단바 배지·툴팁·트레이 메뉴 공용)
pub const SHORTCUT_TOGGLE_MODE: &str = "F1";
pub const SHORTCUT_WORD_WRAP: &str = "Alt+Z";
pub const SHORTCUT_OPEN_WINDOW: &str = "Ctrl+Alt+Shift+G";

#[cfg(target_arch = "x86_64")]
const BITNESS_LABEL: &str = "64bit";
#[cfg(target_arch = "x86")]
const BITNESS_LABEL: &str = "32bit";
#[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
const BITNESS_LABEL: &str = "unknown";

pub fn window_title() -> String {
    format!("{APP_NAME} {BITNESS_LABEL} {}", env!("CARGO_PKG_VERSION"))
}

pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

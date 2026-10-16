// Windows 라이트/다크 테마·강조색 감지 모듈 (HKCU 레지스트리 조회)

use windows::{
    core::*,
    Win32::{
        Foundation::*,
        System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_BINARY, RRF_RT_REG_DWORD},
    },
};

const PERSONALIZE_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
const DWM_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\DWM");
const ACCENT_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Accent");

// Windows 11 기본 강조색 (레지스트리 값이 없을 때)
const DEFAULT_ACCENT_LIGHT: COLORREF = COLORREF(0x00B8_5F00); // #005FB8
const DEFAULT_ACCENT_DARK: COLORREF = COLORREF(0x00FF_CD60); // #60CDFF

fn read_dword(key: PCWSTR, name: PCWSTR) -> Option<u32> {
    let mut data = 0u32;
    let mut len = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            name,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut core::ffi::c_void),
            Some(&mut len),
        )
    };
    (status == ERROR_SUCCESS).then_some(data)
}

/// 작업 표시줄·트레이가 라이트 테마인지 ("Windows 모드")
pub fn taskbar_is_light() -> bool {
    read_dword(PERSONALIZE_KEY, w!("SystemUsesLightTheme")).map_or(true, |v| v != 0)
}

/// 앱 창이 라이트 테마인지 ("앱 모드")
pub fn apps_are_light() -> bool {
    // 디버그 빌드 확인용: KEY2GKSRMF_THEME=dark|light 로 시스템 설정 대신 강제
    #[cfg(debug_assertions)]
    if let Ok(forced) = std::env::var("KEY2GKSRMF_THEME") {
        return forced != "dark";
    }
    read_dword(PERSONALIZE_KEY, w!("AppsUseLightTheme")).map_or(true, |v| v != 0)
}

/// "제목 표시줄 및 창 테두리에 강조색 표시"가 켜져 있는지
pub fn caption_uses_accent() -> bool {
    read_dword(DWM_KEY, w!("ColorPrevalence")).map_or(false, |v| v != 0)
}

/// 제목 표시줄에 칠해지는 강조색 (DWM AccentColor, 0xAABBGGRR)
pub fn caption_accent_color() -> Option<COLORREF> {
    read_dword(DWM_KEY, w!("AccentColor")).map(|v| COLORREF(v & 0x00FF_FFFF))
}

/// 컨트롤 강조색. Windows 11처럼 라이트는 Accent Dark 1, 다크는 Accent Light 2를 쓴다.
pub fn control_accent_color(dark: bool) -> COLORREF {
    // AccentPalette: RGBA 8색 (Light3, Light2, Light1, Base, Dark1, Dark2, Dark3, 예비)
    let mut palette = [0u8; 32];
    let mut len = palette.len() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            ACCENT_KEY,
            w!("AccentPalette"),
            RRF_RT_REG_BINARY,
            None,
            Some(palette.as_mut_ptr() as *mut core::ffi::c_void),
            Some(&mut len),
        )
    };
    if status != ERROR_SUCCESS || len < 32 {
        return if dark { DEFAULT_ACCENT_DARK } else { DEFAULT_ACCENT_LIGHT };
    }
    let i = if dark { 1 } else { 4 } * 4;
    let (r, g, b) = (palette[i] as u32, palette[i + 1] as u32, palette[i + 2] as u32);
    COLORREF((b << 16) | (g << 8) | r)
}

/// 상대 휘도 기준으로 어두운 색인지
pub fn is_dark_color(c: COLORREF) -> bool {
    let r = (c.0 & 0xFF) as f32;
    let g = ((c.0 >> 8) & 0xFF) as f32;
    let b = ((c.0 >> 16) & 0xFF) as f32;
    (0.299 * r + 0.587 * g + 0.114 * b) < 140.0
}

/// 창 제목 표시줄 배경이 어두운지 (창 아이콘을 흰 글자로 쓸지 결정)
pub fn caption_is_dark() -> bool {
    if !apps_are_light() {
        return true;
    }
    if caption_uses_accent() {
        if let Some(accent) = caption_accent_color() {
            return is_dark_color(accent);
        }
    }
    false
}

/// WM_SETTINGCHANGE의 lparam이 테마 변경("ImmersiveColorSet")인지
pub unsafe fn is_theme_change(lparam: LPARAM) -> bool {
    if lparam.0 == 0 {
        return false;
    }
    let area = PCWSTR(lparam.0 as *const u16);
    area.to_string().map_or(false, |s| s == "ImmersiveColorSet")
}

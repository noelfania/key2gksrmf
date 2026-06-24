// 트레이 아이콘 등록/해제 및 컨텍스트 메뉴 관리 모듈

use windows::{
    core::*,
    Win32::{
        Foundation::*,
        System::Registry::{
            RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, REG_DWORD,
        },
        UI::{
            Input::KeyboardAndMouse::SetFocus,
            Shell::*,
            WindowsAndMessaging::*,
        },
    },
};

use crate::config_store;

// 트레이 아이콘 콜백 메시지 ID
pub const WM_TRAY: u32 = WM_APP + 1;

// 컨텍스트 메뉴 항목 ID
pub const IDM_ALWAYS_ON_TOP: u32 = 1001;
pub const IDM_START_WITH_WINDOWS: u32 = 1002;
pub const IDM_EXIT: u32 = 1003;

const IDI_GKSRMF_LIGHT: usize = 101;
const IDI_GKSRMF_DARK: usize = 102;
const PERSONALIZE_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
const SYSTEM_USES_LIGHT_THEME: &str = "SystemUsesLightTheme";

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn resource_id(id: usize) -> PCWSTR {
    PCWSTR(id as *const u16)
}

unsafe fn system_uses_light_theme() -> bool {
    let key_wide = to_wide(PERSONALIZE_KEY);
    let value_wide = to_wide(SYSTEM_USES_LIGHT_THEME);

    let mut hkey = Default::default();
    if RegOpenKeyExW(
        HKEY_CURRENT_USER,
        PCWSTR(key_wide.as_ptr()),
        0,
        KEY_READ,
        &mut hkey,
    )
    .is_err()
    {
        return true;
    }

    let mut value_type = REG_DWORD;
    let mut data = [0u8; 4];
    let mut data_len = data.len() as u32;
    let result = RegQueryValueExW(
        hkey,
        PCWSTR(value_wide.as_ptr()),
        None,
        Some(&mut value_type),
        Some(data.as_mut_ptr()),
        Some(&mut data_len),
    );
    let _ = RegCloseKey(hkey);

    if result.is_err() || value_type != REG_DWORD || data_len < 4 {
        return true;
    }

    u32::from_le_bytes(data) != 0
}

pub unsafe fn load_themed_icon(hinstance: HINSTANCE) -> HICON {
    let icon_id = if system_uses_light_theme() {
        IDI_GKSRMF_LIGHT
    } else {
        IDI_GKSRMF_DARK
    };

    LoadIconW(hinstance, resource_id(icon_id))
        .or_else(|_| LoadIconW(hinstance, IDI_APPLICATION))
        .unwrap_or_default()
}

pub unsafe fn register_tray_icon(hwnd: HWND, hinstance: HINSTANCE) -> NOTIFYICONDATAW {
    let icon = load_themed_icon(hinstance);

    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAY,
        hIcon: icon,
        ..Default::default()
    };

    let tip = "gksrmf";
    let tip_wide: Vec<u16> = tip.encode_utf16().collect();
    let len = tip_wide.len().min(nid.szTip.len() - 1);
    nid.szTip[..len].copy_from_slice(&tip_wide[..len]);

    let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    nid
}

pub unsafe fn refresh_tray_icon(nid: &mut NOTIFYICONDATAW, hinstance: HINSTANCE) {
    nid.hIcon = load_themed_icon(hinstance);
    nid.uFlags = NIF_ICON;
    let _ = Shell_NotifyIconW(NIM_MODIFY, nid);
}

pub unsafe fn remove_tray_icon(nid: &NOTIFYICONDATAW) {
    let _ = Shell_NotifyIconW(NIM_DELETE, nid);
}

pub unsafe fn show_context_menu(hwnd: HWND, cfg: &config_store::AppConfig) {
    let hmenu = CreatePopupMenu().unwrap();

    let always_on_top_flag = if cfg.always_on_top {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING
    };
    AppendMenuW(hmenu, always_on_top_flag, IDM_ALWAYS_ON_TOP as usize, w!("항상 위")).ok();

    let startup_flag = if cfg.start_with_windows {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING
    };
    AppendMenuW(hmenu, startup_flag, IDM_START_WITH_WINDOWS as usize, w!("Windows 시작 시 기동")).ok();

    AppendMenuW(hmenu, MF_SEPARATOR, 0, None).ok();
    AppendMenuW(hmenu, MF_STRING, IDM_EXIT as usize, w!("종료")).ok();

    let mut pt = POINT::default();
    GetCursorPos(&mut pt).ok();

    // 포커스 설정 없이 팝업을 띄우면 메뉴 바깥 클릭으로 닫히지 않으므로 foreground 설정
    let _ = SetForegroundWindow(hwnd);
    let _ = TrackPopupMenu(hmenu, TPM_BOTTOMALIGN | TPM_LEFTALIGN, pt.x, pt.y, 0, hwnd, None);
    DestroyMenu(hmenu).ok();
}

/// 트레이 아이콘 더블클릭 시 보통의 트레이 앱처럼 창을 표시하고 포커스한다.
pub unsafe fn show_or_focus_window(hwnd: HWND, hwnd_edit: HWND) {
    // SW_SHOW 대신 SW_RESTORE를 사용하여 비정상적으로 최소화된 경우도 복구
    let _ = ShowWindow(hwnd, SW_RESTORE);
    let _ = SetForegroundWindow(hwnd);
    if hwnd_edit != HWND(0 as _) {
        let _ = SetFocus(hwnd_edit);
    }
}

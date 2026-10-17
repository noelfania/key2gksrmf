// 트레이 아이콘 등록/해제 및 컨텍스트 메뉴 관리 모듈

use windows::{
    core::*,
    Win32::{
        Foundation::*,
        UI::{
            Controls::{LoadIconMetric, LIM_SMALL},
            HiDpi::GetSystemMetricsForDpi,
            Input::KeyboardAndMouse::SetFocus,
            Shell::*,
            WindowsAndMessaging::*,
        },
    },
};

use crate::{app_info, config_store, system_theme};

// 트레이 아이콘 콜백 메시지 ID
pub const WM_TRAY: u32 = WM_APP + 1;

// 컨텍스트 메뉴 항목 ID
pub const IDM_ALWAYS_ON_TOP: u32 = 1001;
pub const IDM_START_WITH_WINDOWS: u32 = 1002;
pub const IDM_EXIT: u32 = 1003;
pub const IDM_GLOBAL_HOTKEY: u32 = 1004;
pub const IDM_TOGGLE_WINDOW: u32 = 1005;
pub const IDM_WORD_WRAP: u32 = 1006;
pub const IDM_ABOUT: u32 = 1007;

// NOTIFYICON_VERSION_4 키보드 선택 (NIN_SELECT | NINF_KEY)
pub const NIN_KEYSELECT: u32 = NIN_SELECT | 0x1;

// 아이콘 리소스: 101 = 검은 '나'(밝은 배경용), 102 = 흰 '나'(어두운 배경용)
pub const IDI_GKSRMF_LIGHT: usize = 101;
pub const IDI_GKSRMF_DARK: usize = 102;
const TRAY_ICON_UID: u32 = 1;

fn resource_id(id: usize) -> PCWSTR {
    PCWSTR(id as *const u16)
}

fn icon_for_dark_background(dark: bool) -> usize {
    if dark {
        IDI_GKSRMF_DARK
    } else {
        IDI_GKSRMF_LIGHT
    }
}

/// 작업 표시줄 테마에 맞는 트레이 아이콘 (DPI에 맞는 소형 크기). 호출자가 DestroyIcon 한다.
unsafe fn load_tray_icon(hinstance: HINSTANCE) -> HICON {
    let id = icon_for_dark_background(!system_theme::taskbar_is_light());
    LoadIconMetric(hinstance, resource_id(id), LIM_SMALL)
        .or_else(|_| LoadIconW(None, IDI_APPLICATION))
        .unwrap_or_default()
}

/// 제목 표시줄 배경에 맞는 창 아이콘 (작은/큰). 호출자가 DestroyIcon 한다.
pub unsafe fn load_window_icons(hinstance: HINSTANCE, dpi: u32) -> (HICON, HICON) {
    let id = icon_for_dark_background(system_theme::caption_is_dark());
    let load = |metric_x: SYSTEM_METRICS_INDEX, metric_y: SYSTEM_METRICS_INDEX| {
        LoadImageW(
            hinstance,
            resource_id(id),
            IMAGE_ICON,
            GetSystemMetricsForDpi(metric_x, dpi),
            GetSystemMetricsForDpi(metric_y, dpi),
            LR_DEFAULTCOLOR,
        )
        .map(|h| HICON(h.0))
        .unwrap_or_default()
    };
    (load(SM_CXSMICON, SM_CYSMICON), load(SM_CXICON, SM_CYICON))
}

fn copy_to_wide_buf(dst: &mut [u16], text: &str) {
    let wide: Vec<u16> = text.encode_utf16().collect();
    let len = wide.len().min(dst.len() - 1);
    dst[..len].copy_from_slice(&wide[..len]);
    dst[len] = 0;
}

pub unsafe fn register_tray_icon(hwnd: HWND, hinstance: HINSTANCE, tip: &str) -> NOTIFYICONDATAW {
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ICON_UID,
        // VERSION_4에서는 NIF_SHOWTIP이 있어야 기본 툴팁이 뜬다
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP,
        uCallbackMessage: WM_TRAY,
        hIcon: load_tray_icon(hinstance),
        ..Default::default()
    };
    copy_to_wide_buf(&mut nid.szTip, tip);

    let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    // 키보드 접근(Win+B)·WM_CONTEXTMENU 좌표를 쓰기 위해 최신 동작으로
    nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    let _ = Shell_NotifyIconW(NIM_SETVERSION, &nid);
    nid
}

/// 탐색기 재시작(TaskbarCreated) 후 아이콘을 다시 등록한다.
pub unsafe fn re_add_tray_icon(nid: &NOTIFYICONDATAW) {
    let mut add = *nid;
    add.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
    let _ = Shell_NotifyIconW(NIM_ADD, &add);
    add.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    let _ = Shell_NotifyIconW(NIM_SETVERSION, &add);
}

pub unsafe fn refresh_tray_icon(nid: &mut NOTIFYICONDATAW, hinstance: HINSTANCE) {
    let old = nid.hIcon;
    nid.hIcon = load_tray_icon(hinstance);
    nid.uFlags = NIF_ICON;
    let _ = Shell_NotifyIconW(NIM_MODIFY, nid);
    if old != nid.hIcon {
        let _ = DestroyIcon(old);
    }
}

pub unsafe fn remove_tray_icon(nid: &NOTIFYICONDATAW) {
    let _ = Shell_NotifyIconW(NIM_DELETE, nid);
    let _ = DestroyIcon(nid.hIcon);
}

unsafe fn append_item(hmenu: HMENU, id: u32, text: &str, checked: bool) {
    let flags = if checked { MF_STRING | MF_CHECKED } else { MF_STRING };
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    AppendMenuW(hmenu, flags, id as usize, PCWSTR(wide.as_ptr())).ok();
}

/// 트레이 우클릭 메뉴. 단축키는 탭(\t) 뒤에 써서 오른쪽 정렬한다.
pub unsafe fn show_context_menu(
    hwnd: HWND,
    cfg: &config_store::AppConfig,
    window_visible: bool,
    anchor: POINT,
) {
    let Ok(hmenu) = CreatePopupMenu() else {
        return;
    };

    let toggle_text = match (window_visible, cfg.global_hotkey_open) {
        (true, _) => format!("Hide {}", app_info::APP_NAME),
        (false, true) => format!("Open {}\t{}", app_info::APP_NAME, app_info::SHORTCUT_OPEN_WINDOW),
        (false, false) => format!("Open {}", app_info::APP_NAME),
    };
    append_item(hmenu, IDM_TOGGLE_WINDOW, &toggle_text, false);
    // 더블클릭 동작과 같은 항목을 굵게 표시
    let _ = SetMenuDefaultItem(hmenu, IDM_TOGGLE_WINDOW, 0);
    AppendMenuW(hmenu, MF_SEPARATOR, 0, None).ok();

    append_item(hmenu, IDM_ALWAYS_ON_TOP, "Always on Top", cfg.always_on_top);
    append_item(
        hmenu,
        IDM_WORD_WRAP,
        &format!("Word Wrap\t{}", app_info::SHORTCUT_WORD_WRAP),
        cfg.word_wrap,
    );
    AppendMenuW(hmenu, MF_SEPARATOR, 0, None).ok();

    append_item(
        hmenu,
        IDM_GLOBAL_HOTKEY,
        &format!("Global Hotkey ({})", app_info::SHORTCUT_OPEN_WINDOW),
        cfg.global_hotkey_open,
    );
    append_item(hmenu, IDM_START_WITH_WINDOWS, "Start with Windows", cfg.start_with_windows);
    AppendMenuW(hmenu, MF_SEPARATOR, 0, None).ok();

    append_item(hmenu, IDM_ABOUT, "GitHub", false);
    append_item(hmenu, IDM_EXIT, "Exit", false);

    // 메뉴를 띄우기 전 전경 창이 되어야 바깥 클릭 시 메뉴가 닫힌다
    let _ = SetForegroundWindow(hwnd);
    // 트레이는 보통 화면 우측 하단 — 메뉴가 기준점 왼쪽 위로 펼쳐지게 한다
    let _ = TrackPopupMenu(
        hmenu,
        TPM_BOTTOMALIGN | TPM_RIGHTALIGN | TPM_RIGHTBUTTON,
        anchor.x,
        anchor.y,
        0,
        hwnd,
        None,
    );
    // 메뉴가 닫힌 뒤 전경 전환이 꼬이지 않도록 (TrackPopupMenu 문서의 권장 패턴)
    let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
    DestroyMenu(hmenu).ok();
}

/// 프로젝트 페이지를 기본 브라우저로 연다.
pub unsafe fn open_homepage() {
    let url: Vec<u16> = app_info::APP_HOMEPAGE.encode_utf16().chain(std::iter::once(0)).collect();
    ShellExecuteW(None, w!("open"), PCWSTR(url.as_ptr()), None, None, SW_SHOWNORMAL);
}

/// 창을 표시하고 포커스한다.
pub unsafe fn show_or_focus_window(hwnd: HWND, hwnd_edit: HWND) {
    // SW_SHOW 대신 SW_RESTORE를 사용하여 비정상적으로 최소화된 경우도 복구
    let _ = ShowWindow(hwnd, SW_RESTORE);
    let _ = SetForegroundWindow(hwnd);
    if hwnd_edit != HWND(0 as _) {
        let _ = SetFocus(hwnd_edit);
    }
}

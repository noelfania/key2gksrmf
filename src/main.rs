// key2gksrmf — Win32 트레이 앱 진입점
// 트레이 아이콘 상주, 에디터 창, 실시간 한글 변환

#![windows_subsystem = "windows"]
// 단일 스레드 Win32 앱 특성상 전역 mutable static은 unsafe 블록 안에서만 접근한다
#![allow(static_mut_refs)]

mod app_info;
mod config_store;
mod editor_window;
mod font_manager;
mod hangul_engine;
mod input_controller;
mod startup_registry;
mod system_theme;
mod tray_manager;

use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::{
            Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE},
            Gdi::*,
        },
        System::{
            LibraryLoader::GetModuleHandleW, SystemInformation::GetTickCount,
            Threading::CreateMutexW,
        },
        UI::{
            HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow},
            Input::KeyboardAndMouse::{
                GetDoubleClickTime, GetKeyState, RegisterHotKey, SetFocus, UnregisterHotKey,
                MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, VIRTUAL_KEY, VK_CONTROL, VK_SHIFT,
            },
            Shell::*,
            WindowsAndMessaging::*,
        },
    },
};

use input_controller::{InputController, InputMode, KeyEvent, KeyResult};
use tray_manager::{
    IDM_ABOUT, IDM_ALWAYS_ON_TOP, IDM_EXIT, IDM_GLOBAL_HOTKEY, IDM_START_WITH_WINDOWS,
    IDM_TOGGLE_WINDOW, IDM_WORD_WRAP, NIN_KEYSELECT, WM_TRAY,
};

// EditControl 메시지 상수 (windows 크레이트 미노출)
const EM_GETSEL: u32 = 0x00B0;
const EM_SETSEL: u32 = 0x00B1;
const EM_SCROLLCARET: u32 = 0x00B7;
const VK_F1: u32 = 0x70;
const EN_VSCROLL_RAW: u16 = 0x0602;
const WM_DWMCOLORIZATIONCOLORCHANGED_RAW: u32 = 0x0320;
const DWMWA_USE_IMMERSIVE_DARK_MODE: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(20);
// 전역 단축키 (Ctrl+Alt+Shift+G 창 열기)
const HOTKEY_ID_OPEN: i32 = 1;
// 트레이 클릭 직전까지 창이 활성 상태였다고 볼 시간 (ms)
const TRAY_TOGGLE_ACTIVE_WINDOW_MS: u32 = 500;

// 전역 상태 (Win32 콜백에서 접근하기 위해 전역으로 관리)
static mut G_HWND_MAIN: HWND = HWND(0 as _);
static mut G_HWND_EDIT: HWND = HWND(0 as _);
static mut G_HWND_GUTTER: HWND = HWND(0 as _);
static mut G_HWND_TOOLBAR: HWND = HWND(0 as _);
static mut G_CONFIG: Option<config_store::AppConfig> = None;
static mut G_NOTIFY_ICON_DATA: Option<NOTIFYICONDATAW> = None;
static mut G_EMBEDDED_FONT: Option<font_manager::EmbeddedFont> = None;
static mut G_SINGLE_INSTANCE_MUTEX: Option<HANDLE> = None;
// WM_SETICON으로 넣은 창 아이콘 (작은, 큰). 교체 시 이전 것을 해제한다.
static mut G_WINDOW_ICONS: (HICON, HICON) = (HICON(0 as _), HICON(0 as _));
// 탐색기 재시작 알림 메시지 ID
static mut G_WM_TASKBAR_CREATED: u32 = 0;
// 트레이 클릭 토글 판정용 시각 (GetTickCount)
static mut G_LAST_DEACTIVATE_TICK: u32 = 0;
static mut G_LAST_TRAY_SHOW_TICK: u32 = 0;

// 물리키 기반 조합 상태 (모드·조합 추적·WM_CHAR 억제)
static mut G_INPUT: Option<InputController> = None;

// 에디터 컨트롤의 원본 WndProc 보관용
static mut G_ORIG_EDIT_PROC: WNDPROC = None;

unsafe fn input() -> &'static mut InputController {
    G_INPUT.get_or_insert_with(InputController::new)
}

unsafe fn get_selection(hwnd: HWND) -> (u32, u32) {
    let mut start = 0u32;
    let mut end = 0u32;
    SendMessageW(
        hwnd,
        EM_GETSEL,
        WPARAM(&mut start as *mut _ as usize),
        LPARAM(&mut end as *mut _ as isize),
    );
    (start, end)
}

unsafe fn set_selection(hwnd: HWND, start: u32, end: u32) {
    SendMessageW(hwnd, EM_SETSEL, WPARAM(start as usize), LPARAM(end as isize));
}

unsafe fn commit_composition() {
    input().commit();
}

fn tray_tip(mode: InputMode) -> String {
    let mode_label = match mode {
        InputMode::Han => "한",
        InputMode::Eng => "A",
    };
    format!("{} — {}", app_info::APP_NAME, mode_label)
}

/// 입력 모드·Wrap·항상 위 상태를 상단바와 트레이 툴팁에 반영한다.
unsafe fn sync_toolbar() {
    let mode = input().mode();
    let (wrap, topmost) = G_CONFIG
        .as_ref()
        .map_or((true, false), |c| (c.word_wrap, c.always_on_top));
    editor_window::toolbar_set_state(mode == InputMode::Han, wrap, topmost);
    if let Some(nid) = G_NOTIFY_ICON_DATA.as_mut() {
        tray_manager::set_tray_tip(nid, &tray_tip(mode));
    }
}

unsafe fn get_edit_text(hwnd: HWND) -> String {
    let len = GetWindowTextLengthW(hwnd) as usize;
    if len == 0 {
        return String::new();
    }
    let mut buf: Vec<u16> = vec![0u16; len + 1];
    GetWindowTextW(hwnd, &mut buf);
    buf.truncate(len);
    String::from_utf16_lossy(&buf)
}

unsafe fn set_edit_text(hwnd: HWND, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = SetWindowTextW(hwnd, PCWSTR(wide.as_ptr()));
}

unsafe fn subclass_edit_control(hwnd: HWND) {
    G_ORIG_EDIT_PROC = std::mem::transmute(SetWindowLongPtrW(
        hwnd,
        GWLP_WNDPROC,
        edit_proc as *const () as isize,
    ));
}

unsafe fn apply_font_to_edit(hwnd: HWND) {
    if let Some(font) = G_EMBEDDED_FONT.as_ref() {
        font_manager::apply_font_to_control(hwnd, font.hfont());
    }
}

unsafe fn relayout(hwnd: HWND) {
    let mut rect = RECT::default();
    GetClientRect(hwnd, &mut rect).ok();
    editor_window::resize_editor_controls(
        G_HWND_TOOLBAR,
        G_HWND_GUTTER,
        G_HWND_EDIT,
        rect.right,
        rect.bottom,
    );
}

/// Wrap 토글: ES_AUTOHSCROLL은 생성 후 변경 불가하므로 EDIT를 재생성한다.
unsafe fn toggle_word_wrap(hwnd: HWND) {
    commit_composition();

    let Some(cfg) = G_CONFIG.as_mut() else {
        return;
    };
    cfg.word_wrap = !cfg.word_wrap;
    let word_wrap = cfg.word_wrap;
    config_store::save(cfg);
    sync_toolbar();

    if G_HWND_EDIT == HWND(0 as _) {
        return;
    }

    let old_text = get_edit_text(G_HWND_EDIT);
    let (sel_start, sel_end) = get_selection(G_HWND_EDIT);

    let _ = DestroyWindow(G_HWND_EDIT);
    G_HWND_EDIT = HWND(0 as _);
    G_ORIG_EDIT_PROC = None;

    let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
    let mut rect = RECT::default();
    GetClientRect(hwnd, &mut rect).ok();
    let edit_w = (rect.right - editor_window::gutter_width()).max(1);
    let edit_h = (rect.bottom - editor_window::toolbar_height()).max(1);

    G_HWND_EDIT = editor_window::create_edit_control(hwnd, hinstance, word_wrap, edit_w, edit_h);
    subclass_edit_control(G_HWND_EDIT);
    apply_font_to_edit(G_HWND_EDIT);
    set_edit_text(G_HWND_EDIT, &old_text);
    set_selection(G_HWND_EDIT, sel_start, sel_end);
    // 재생성 직후 스크롤이 맨 위로 가므로 캐럿이 보이는 위치로 복원
    SendMessageW(G_HWND_EDIT, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
    editor_window::set_gutter_target(G_HWND_EDIT);

    relayout(hwnd);
    focus_editor();
}

unsafe fn handle_alt_z_word_wrap(hwnd: HWND) -> bool {
    toggle_word_wrap(hwnd);
    input().suppress_next_char();
    true
}

unsafe fn register_open_hotkey(hwnd: HWND) -> bool {
    RegisterHotKey(
        hwnd,
        HOTKEY_ID_OPEN,
        MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_NOREPEAT,
        'G' as u32,
    )
    .is_ok()
}

unsafe fn toggle_global_hotkey(hwnd: HWND) {
    let cfg = G_CONFIG.as_mut().unwrap();
    if cfg.global_hotkey_open {
        let _ = UnregisterHotKey(hwnd, HOTKEY_ID_OPEN);
        cfg.global_hotkey_open = false;
        config_store::save(cfg);
    } else if register_open_hotkey(hwnd) {
        cfg.global_hotkey_open = true;
        config_store::save(cfg);
    } else {
        // 다른 앱이 같은 조합을 선점한 경우
        let app_name = app_info::to_wide(app_info::APP_NAME);
        let message = app_info::to_wide(&format!(
            "전역 단축키({})를 등록하지 못했습니다.\n다른 프로그램이 이미 사용 중일 수 있습니다.",
            app_info::SHORTCUT_OPEN_WINDOW
        ));
        MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            PCWSTR(app_name.as_ptr()),
            MB_ICONWARNING | MB_OK,
        );
    }
}

unsafe fn set_input_mode(mode: InputMode) {
    input().set_mode(mode);
    sync_toolbar();
}

unsafe fn focus_editor() {
    if G_HWND_EDIT != HWND(0 as _) {
        let _ = SetFocus(G_HWND_EDIT);
    }
}

unsafe fn toggle_input_mode_and_focus_editor() {
    input().toggle_mode();
    sync_toolbar();
    focus_editor();
}

unsafe fn cleanup_embedded_font() {
    if let Some(font) = G_EMBEDDED_FONT.take() {
        font_manager::cleanup_embedded_font(font);
    }
}

unsafe fn focus_existing_instance() {
    let class_name = app_info::to_wide(app_info::APP_WINDOW_CLASS);
    if let Ok(hwnd) = FindWindowW(PCWSTR(class_name.as_ptr()), None) {
        if hwnd != HWND(0 as _) {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

unsafe fn cleanup_single_instance_mutex() {
    if let Some(mutex) = G_SINGLE_INSTANCE_MUTEX.take() {
        let _ = CloseHandle(mutex);
    }
}

unsafe fn acquire_single_instance() -> bool {
    let mutex_name = app_info::to_wide(app_info::APP_MUTEX_NAME);
    let mutex = match CreateMutexW(None, false, PCWSTR(mutex_name.as_ptr())) {
        Ok(mutex) => mutex,
        Err(_) => {
            // mutex 생성 실패 시 앱 사용성 저하를 막기 위해 단일 실행 가드는 건너뛴다.
            return true;
        }
    };

    let already_exists = GetLastError() == ERROR_ALREADY_EXISTS;
    G_SINGLE_INSTANCE_MUTEX = Some(mutex);

    if already_exists {
        focus_existing_instance();
        cleanup_single_instance_mutex();
        return false;
    }
    true
}

// ---------- 테마·아이콘 ----------

unsafe fn set_dark_title_bar(hwnd: HWND, dark: bool) {
    let value = BOOL::from(dark);
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
        &value as *const BOOL as *const core::ffi::c_void,
        std::mem::size_of::<BOOL>() as u32,
    );
}

/// 제목 표시줄 배경에 맞는 '나' 아이콘(작은·큰)을 현재 DPI 크기로 설정한다.
unsafe fn apply_window_icons(hwnd: HWND) {
    let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
    let (small, big) = tray_manager::load_window_icons(hinstance, editor_window::dpi());
    SendMessageW(hwnd, WM_SETICON, WPARAM(ICON_SMALL as usize), LPARAM(small.0 as isize));
    SendMessageW(hwnd, WM_SETICON, WPARAM(ICON_BIG as usize), LPARAM(big.0 as isize));
    let (old_small, old_big) = std::mem::replace(&mut G_WINDOW_ICONS, (small, big));
    for icon in [old_small, old_big] {
        if icon != HICON(0 as _) {
            let _ = DestroyIcon(icon);
        }
    }
}

/// Windows 라이트/다크·강조색에 맞춰 창 전체를 다시 칠한다.
unsafe fn apply_theme(hwnd: HWND) {
    let dark = !system_theme::apps_are_light();
    if let Some(cfg) = G_CONFIG.as_ref() {
        editor_window::init_theme(cfg, dark, system_theme::control_accent_color(dark));
    }
    set_dark_title_bar(hwnd, dark);
    apply_window_icons(hwnd);
    if let Some(nid) = G_NOTIFY_ICON_DATA.as_mut() {
        let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
        tray_manager::refresh_tray_icon(nid, hinstance);
    }
    editor_window::apply_control_theme(G_HWND_EDIT);
    editor_window::refresh_toolbar_theme();
    system_theme::flush_menu_themes();
    let _ = RedrawWindow(
        hwnd,
        None,
        None,
        RDW_ERASE | RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_FRAME,
    );
}

/// 모니터 DPI가 바뀌면 폰트·아이콘·치수를 새 배율로 다시 만든다.
unsafe fn apply_dpi_change(hwnd: HWND, dpi: u32, suggested: &RECT) {
    editor_window::set_dpi(dpi);
    if let Some(font) = G_EMBEDDED_FONT.as_mut() {
        if let Some(old) = font.recreate_for_dpi(dpi) {
            apply_font_to_edit(G_HWND_EDIT);
            let _ = DeleteObject(HGDIOBJ(old.0));
        }
    }
    editor_window::refresh_toolbar_fonts();
    apply_window_icons(hwnd);
    let _ = SetWindowPos(
        hwnd,
        None,
        suggested.left,
        suggested.top,
        suggested.right - suggested.left,
        suggested.bottom - suggested.top,
        SWP_NOZORDER | SWP_NOACTIVATE,
    );
    relayout(hwnd);
    editor_window::invalidate_gutter();
}

// ---------- 트레이 ----------

unsafe fn hide_window(hwnd: HWND) {
    save_window_bounds(hwnd);
    let _ = ShowWindow(hwnd, SW_HIDE);
}

unsafe fn show_window(hwnd: HWND) {
    tray_manager::show_or_focus_window(hwnd, G_HWND_EDIT);
    G_LAST_TRAY_SHOW_TICK = GetTickCount();
}

/// 트레이 아이콘 클릭: 창이 앞에 있었으면 숨기고, 아니면 앞으로 가져온다.
unsafe fn tray_toggle_window(hwnd: HWND) {
    let now = GetTickCount();
    let visible = IsWindowVisible(hwnd).as_bool();
    // 트레이를 누르는 순간 작업 표시줄로 활성이 넘어가므로 직전 비활성 시각으로 판단한다
    let was_active = GetForegroundWindow() == hwnd
        || now.wrapping_sub(G_LAST_DEACTIVATE_TICK) < TRAY_TOGGLE_ACTIVE_WINDOW_MS;
    // 더블클릭의 두 번째 클릭이 방금 연 창을 다시 숨기지 않도록
    let just_shown = now.wrapping_sub(G_LAST_TRAY_SHOW_TICK) < GetDoubleClickTime();
    if visible && was_active && !just_shown {
        hide_window(hwnd);
    } else {
        show_window(hwnd);
    }
}

fn main() {
    unsafe { run() }
}

unsafe extern "system" fn owner_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn run() {
    if !acquire_single_instance() {
        return;
    }

    let cfg = config_store::load();
    startup_registry::sync_on_startup(cfg.start_with_windows);
    G_CONFIG = Some(cfg.clone());
    let dark = !system_theme::apps_are_light();
    editor_window::init_theme(&cfg, dark, system_theme::control_accent_color(dark));
    G_WM_TASKBAR_CREATED = RegisterWindowMessageW(w!("TaskbarCreated"));
    system_theme::allow_dark_menus();

    let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();

    let owner_class_name = app_info::to_wide(app_info::APP_OWNER_WINDOW_CLASS);
    let owner_title = app_info::to_wide(app_info::APP_NAME);
    let owner_wc = WNDCLASSW {
        lpfnWndProc: Some(owner_wnd_proc),
        hInstance: hinstance,
        lpszClassName: PCWSTR(owner_class_name.as_ptr()),
        ..Default::default()
    };
    RegisterClassW(&owner_wc);

    // 작업 표시줄 숨김을 위해 실제 창의 owner로만 쓰는 보이지 않는 창
    let hwnd_owner = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        PCWSTR(owner_class_name.as_ptr()),
        PCWSTR(owner_title.as_ptr()),
        WS_OVERLAPPED,
        0,
        0,
        0,
        0,
        None,
        None,
        hinstance,
        None,
    )
    .unwrap();

    let class_name = app_info::to_wide(app_info::APP_WINDOW_CLASS);
    let window_title = app_info::to_wide(&app_info::window_title());
    let wc = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        // 창 아이콘은 생성 후 테마·DPI에 맞춰 WM_SETICON으로 다시 넣는다
        hIcon: LoadIconW(hinstance, PCWSTR(tray_manager::IDI_GKSRMF_LIGHT as *const u16))
            .unwrap_or_default(),
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
        ..Default::default()
    };
    RegisterClassW(&wc);

    // WS_VSCROLL은 에디터 컨트롤에 내장되어 있으므로 메인 창에는 불필요
    // 표준 캡션 레이아웃은 유지하되 최소화/최대화 버튼만 제거한다
    // WS_CLIPCHILDREN: 부모 WM_PAINT가 자식 컨트롤을 덮어 그리지 않도록
    let style = (WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN) & !WS_MINIMIZEBOX & !WS_MAXIMIZEBOX;
    // WS_EX_TOOLWINDOW을 쓰면 도구창용 작은 캡션이 되므로 일반 네이티브 타이틀바를 유지한다
    let ex_style = if cfg.always_on_top {
        WS_EX_TOPMOST
    } else {
        WINDOW_EX_STYLE(0)
    };

    let mut start_x = cfg.window.x.unwrap_or(CW_USEDEFAULT);
    let mut start_y = cfg.window.y.unwrap_or(CW_USEDEFAULT);

    // 저장된 좌표가 유효한 모니터 화면 내에 있는지 확인 (듀얼 모니터 해제 시 화면 이탈 방지)
    if start_x != CW_USEDEFAULT && start_y != CW_USEDEFAULT {
        let pt = POINT { x: start_x, y: start_y };
        let hmonitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONULL);
        if hmonitor.is_invalid() {
            start_x = CW_USEDEFAULT;
            start_y = CW_USEDEFAULT;
        }
    }

    // 상단바 컨트롤이 들어갈 수 있는 최소 외곽 크기로 클램프
    let (min_win_w, min_win_h) = min_window_size(style, ex_style);
    let start_w = cfg.window.width.max(min_win_w);
    let start_h = cfg.window.height.max(min_win_h);

    let hwnd = CreateWindowExW(
        ex_style,
        PCWSTR(class_name.as_ptr()),
        PCWSTR(window_title.as_ptr()),
        style,
        start_x,
        start_y,
        start_w,
        start_h,
        hwnd_owner,
        None,
        hinstance,
        None,
    )
    .unwrap();

    G_HWND_MAIN = hwnd;
    set_dark_title_bar(hwnd, dark);
    apply_window_icons(hwnd);

    // 설정에 따라 전역 단축키 등록 (실패 시 조용히 설정 원복)
    if cfg.global_hotkey_open && !register_open_hotkey(hwnd) {
        if let Some(c) = G_CONFIG.as_mut() {
            c.global_hotkey_open = false;
            config_store::save(c);
        }
    }

    G_NOTIFY_ICON_DATA = Some(tray_manager::register_tray_icon(
        hwnd,
        hinstance,
        &tray_tip(input().mode()),
    ));

    // 최초 실행 시에도 바로 입력할 수 있게 창을 띄운다
    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = SetForegroundWindow(hwnd);
    if G_HWND_EDIT != HWND(0 as _) {
        let _ = SetFocus(G_HWND_EDIT);
    }

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        // F1(한/영 전환)은 포커스 위치와 상관없이 여기서 한 번만 처리한다
        if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_F1 {
            toggle_input_mode_and_focus_editor();
            continue;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    cleanup_single_instance_mutex();
}

/// 상단바가 깨지지 않는 최소 외곽 창 크기 (클라이언트 min + 프레임).
unsafe fn min_window_size(style: WINDOW_STYLE, ex_style: WINDOW_EX_STYLE) -> (i32, i32) {
    let mut rc = RECT {
        left: 0,
        top: 0,
        right: editor_window::min_client_width(),
        bottom: editor_window::min_client_height(),
    };
    let _ = AdjustWindowRectExForDpi(&mut rc, style, false, ex_style, editor_window::dpi());
    ((rc.right - rc.left).max(1), (rc.bottom - rc.top).max(1))
}

unsafe fn save_window_bounds(hwnd: HWND) {
    let mut wp = WINDOWPLACEMENT {
        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
        ..Default::default()
    };
    if GetWindowPlacement(hwnd, &mut wp).is_ok() {
        if let Some(cfg) = G_CONFIG.as_mut() {
            let rect = wp.rcNormalPosition;
            cfg.window.x = Some(rect.left);
            cfg.window.y = Some(rect.top);
            cfg.window.width = rect.right - rect.left;
            cfg.window.height = rect.bottom - rect.top;
            config_store::save(cfg);
        }
    }
}

unsafe fn toggle_always_on_top(hwnd: HWND) {
    let cfg = G_CONFIG.as_mut().unwrap();
    cfg.always_on_top = !cfg.always_on_top;
    config_store::save(cfg);
    editor_window::apply_always_on_top(hwnd, cfg.always_on_top);
    sync_toolbar();
}

unsafe fn toggle_start_with_windows() {
    let cfg = G_CONFIG.as_mut().unwrap();
    cfg.start_with_windows = !cfg.start_with_windows;

    let ok = if cfg.start_with_windows {
        startup_registry::register()
    } else {
        startup_registry::unregister()
    };

    if ok {
        config_store::save(cfg);
    } else {
        // 실패 시 상태 원복
        cfg.start_with_windows = !cfg.start_with_windows;
        let app_name = app_info::to_wide(app_info::APP_NAME);
        MessageBoxW(
            None,
            w!("시작 프로그램 레지스트리 설정에 실패했습니다."),
            PCWSTR(app_name.as_ptr()),
            MB_ICONERROR | MB_OK,
        );
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // 탐색기가 다시 시작되면 트레이 아이콘을 다시 등록한다
    if G_WM_TASKBAR_CREATED != 0 && msg == G_WM_TASKBAR_CREATED {
        if let Some(nid) = G_NOTIFY_ICON_DATA.as_ref() {
            tray_manager::re_add_tray_icon(nid);
        }
        return LRESULT(0);
    }

    match msg {
        WM_SYSKEYDOWN => {
            if wparam.0 as u32 == 'Z' as u32 {
                handle_alt_z_word_wrap(hwnd);
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        WM_SYSCHAR => {
            // Alt+Z 메뉴 비프음 억제
            if wparam.0 as u32 == 'z' as u32 && input().take_suppress() {
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        WM_ACTIVATE => {
            if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE {
                G_LAST_DEACTIVATE_TICK = GetTickCount();
            } else {
                // Alt+Tab 등으로 창이 활성화되면 바로 타이핑할 수 있게 에디터에 포커스
                focus_editor();
            }
            LRESULT(0)
        }

        WM_CTLCOLOREDIT => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            let theme = editor_window::theme();
            let _ = SetTextColor(hdc, theme.text);
            let _ = SetBkColor(hdc, theme.editor_bg);
            LRESULT(editor_window::editor_bg_brush().0 as isize)
        }

        WM_PAINT => {
            // 자식(상단바·거터·에디터)이 클라이언트를 모두 덮는다. 틈이 보일 때만 에디터 배경으로 칠한다.
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            FillRect(hdc, &ps.rcPaint, editor_window::editor_bg_brush());
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }

        WM_ERASEBKGND => LRESULT(1),

        WM_CREATE => {
            editor_window::set_dpi(GetDpiForWindow(hwnd));
            // 상단바가 Pretendard로 '한'을 그리므로 컨트롤보다 먼저 내장 폰트를 등록한다
            G_EMBEDDED_FONT = font_manager::load_embedded_font(editor_window::dpi());

            let word_wrap = G_CONFIG
                .as_ref()
                .map(|cfg| cfg.word_wrap)
                .unwrap_or(true);
            let controls = editor_window::create_editor_controls(hwnd, word_wrap);
            G_HWND_TOOLBAR = controls.hwnd_toolbar;
            G_HWND_GUTTER = controls.hwnd_gutter;
            G_HWND_EDIT = controls.hwnd_edit;

            // Edit 컨트롤 Subclassing (물리키 → 한글 조합)
            subclass_edit_control(G_HWND_EDIT);

            input().set_mode(InputMode::Han);
            sync_toolbar();

            if let Some(cfg) = G_CONFIG.as_ref() {
                if cfg.always_on_top {
                    editor_window::apply_always_on_top(hwnd, true);
                }
            }

            apply_font_to_edit(G_HWND_EDIT);
            editor_window::apply_editor_padding(G_HWND_EDIT);
            editor_window::invalidate_gutter();
            LRESULT(0)
        }

        WM_GETMINMAXINFO => {
            // 상단바(한/A + Wrap + 항상 위)가 겹치지 않는 최소 트랙 크기
            let style = WINDOW_STYLE(GetWindowLongW(hwnd, GWL_STYLE) as u32);
            let ex_style = WINDOW_EX_STYLE(GetWindowLongW(hwnd, GWL_EXSTYLE) as u32);
            let (min_w, min_h) = min_window_size(style, ex_style);
            let mmi = &mut *(lparam.0 as *mut MINMAXINFO);
            mmi.ptMinTrackSize.x = min_w;
            mmi.ptMinTrackSize.y = min_h;
            LRESULT(0)
        }

        WM_SIZE => {
            // 최소화 중에는 레이아웃하지 않는다
            if wparam.0 as u32 == SIZE_MINIMIZED {
                return LRESULT(0);
            }
            let w = (lparam.0 & 0xFFFF) as i32;
            let h = ((lparam.0 >> 16) & 0xFFFF) as i32;
            editor_window::resize_editor_controls(G_HWND_TOOLBAR, G_HWND_GUTTER, G_HWND_EDIT, w, h);
            LRESULT(0)
        }

        WM_DPICHANGED => {
            let dpi = (wparam.0 & 0xFFFF) as u32;
            let suggested = &*(lparam.0 as *const RECT);
            apply_dpi_change(hwnd, dpi, suggested);
            LRESULT(0)
        }

        WM_HOTKEY => {
            if wparam.0 as i32 == HOTKEY_ID_OPEN {
                show_window(hwnd);
            }
            LRESULT(0)
        }

        WM_COMMAND => {
            let notif = (wparam.0 >> 16) as u16;
            let ctrl_id = (wparam.0 & 0xFFFF) as u16;

            if lparam.0 as isize == G_HWND_EDIT.0 as isize {
                // 텍스트 변경·세로 스크롤 시 행 번호 거터 갱신
                if notif == EN_CHANGE as u16 || notif == EN_VSCROLL_RAW {
                    editor_window::invalidate_gutter();
                }
                // 물리키 기반 입력으로 전환했으므로 EN_CHANGE 후처리는 수행하지 않는다.
                if notif == EN_CHANGE as u16 {
                    return LRESULT(0);
                }
            }

            match ctrl_id as u32 {
                editor_window::IDC_MODE_HAN => {
                    if notif == BN_CLICKED as u16 {
                        set_input_mode(InputMode::Han);
                        focus_editor();
                    }
                }
                editor_window::IDC_MODE_ENG => {
                    if notif == BN_CLICKED as u16 {
                        set_input_mode(InputMode::Eng);
                        focus_editor();
                    }
                }
                editor_window::IDC_ALWAYS_ON_TOP_CHECK => {
                    if notif == BN_CLICKED as u16 {
                        toggle_always_on_top(hwnd);
                        focus_editor();
                    }
                }
                editor_window::IDC_WORD_WRAP_CHECK => {
                    if notif == BN_CLICKED as u16 {
                        toggle_word_wrap(hwnd);
                    }
                }
                IDM_TOGGLE_WINDOW => {
                    if IsWindowVisible(hwnd).as_bool() {
                        hide_window(hwnd);
                    } else {
                        show_window(hwnd);
                    }
                }
                IDM_ALWAYS_ON_TOP => toggle_always_on_top(hwnd),
                IDM_WORD_WRAP => toggle_word_wrap(hwnd),
                IDM_START_WITH_WINDOWS => toggle_start_with_windows(),
                IDM_GLOBAL_HOTKEY => toggle_global_hotkey(hwnd),
                IDM_ABOUT => tray_manager::open_homepage(),
                IDM_EXIT => {
                    // 정리는 WM_DESTROY 한 곳에서 한다
                    let _ = DestroyWindow(hwnd);
                }
                _ => {}
            }
            LRESULT(0)
        }

        WM_TRAY => {
            // NOTIFYICON_VERSION_4: LOWORD(lparam)=이벤트, wparam=기준 좌표
            let event = (lparam.0 & 0xFFFF) as u32;
            match event {
                NIN_SELECT | NIN_KEYSELECT => tray_toggle_window(hwnd),
                WM_LBUTTONDBLCLK => show_window(hwnd),
                WM_CONTEXTMENU => {
                    let anchor = POINT {
                        x: (wparam.0 & 0xFFFF) as i16 as i32,
                        y: ((wparam.0 >> 16) & 0xFFFF) as i16 as i32,
                    };
                    let visible = IsWindowVisible(hwnd).as_bool();
                    if let Some(cfg) = G_CONFIG.as_ref() {
                        tray_manager::show_context_menu(hwnd, cfg, visible, anchor);
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }

        WM_CLOSE => {
            // 닫기 버튼은 종료 대신 숨김 처리하되 마지막 창 위치/크기는 저장
            hide_window(hwnd);
            LRESULT(0)
        }

        WM_SYSCOMMAND => {
            let cmd = (wparam.0 & 0xFFF0) as u32;
            if cmd == SC_MINIMIZE {
                // 최소화 명령 가로채기: 닫기 버튼과 동일하게 트레이로 숨김 처리
                hide_window(hwnd);
                LRESULT(0)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }

        WM_SETTINGCHANGE => {
            // Windows 라이트/다크 테마 변경
            if system_theme::is_theme_change(lparam) {
                apply_theme(hwnd);
            }
            LRESULT(0)
        }

        WM_DWMCOLORIZATIONCOLORCHANGED_RAW => {
            // 강조색 변경
            apply_theme(hwnd);
            LRESULT(0)
        }

        WM_DESTROY => {
            save_window_bounds(hwnd);
            cleanup_embedded_font();
            editor_window::cleanup();
            if let Some(nid) = &G_NOTIFY_ICON_DATA {
                tray_manager::remove_tray_icon(nid);
            }
            let (small, big) = std::mem::take(&mut G_WINDOW_ICONS);
            for icon in [small, big] {
                if icon != HICON(0 as _) {
                    let _ = DestroyIcon(icon);
                }
            }
            PostQuitMessage(0);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let result = edit_proc_inner(hwnd, msg, wparam, lparam);
    // 커서 이동·스크롤·편집 후 행 번호 거터 갱신
    match msg {
        WM_KEYDOWN | WM_SYSKEYDOWN | WM_CHAR | WM_MOUSEWHEEL | WM_VSCROLL | WM_HSCROLL
        | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_PASTE | WM_CUT | WM_SETFOCUS | WM_SIZE => {
            editor_window::invalidate_gutter();
        }
        _ => {}
    }
    result
}

// 메시지가 만들어질 당시의 키 상태 (GetAsyncKeyState는 처리 시점 상태라 빠른 입력에서 어긋난다)
unsafe fn key_down(vk: VIRTUAL_KEY) -> bool {
    (GetKeyState(vk.0 as i32) as u16 & 0x8000) != 0
}

unsafe fn edit_proc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_CHAR || msg == WM_SYSCHAR {
        if !input().filter_char(wparam.0 as u32) {
            return LRESULT(0);
        }
    }

    if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
        let key = wparam.0 as u32;

        if msg == WM_SYSKEYDOWN && key == 'Z' as u32 {
            handle_alt_z_word_wrap(G_HWND_MAIN);
            return LRESULT(0);
        }

        let ev = KeyEvent {
            vk: key,
            shift: key_down(VK_SHIFT),
            ctrl: key_down(VK_CONTROL),
            alt: msg == WM_SYSKEYDOWN,
        };
        let mut buf = editor_window::EditBuffer(hwnd);
        if input().handle_key(&mut buf, ev) == KeyResult::Handled {
            return LRESULT(0);
        }
    }

    if msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN {
        input().on_mouse_down();
    }

    CallWindowProcW(G_ORIG_EDIT_PROC, hwnd, msg, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composer_backspace_syllable_in_main_flow() {
        let mut composer = hangul_engine::HangulComposer::new();
        let mut committed = String::new();
        let mut composing = String::new();
        for ch in "gksrmf".chars() {
            let step = composer.feed_key(ch);
            committed.push_str(&step.committed);
            composing = step.composing;
        }
        assert_eq!(format!("{committed}{composing}"), "한글");
        assert_eq!(composer.pop_key(), "그");
    }
}

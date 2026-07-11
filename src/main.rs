// key2gksrmf — Win32 트레이 앱 진입점
// 트레이 아이콘 상주, 에디터 창, 실시간 한글 변환

#![windows_subsystem = "windows"]
// 단일 스레드 Win32 앱 특성상 전역 mutable static은 unsafe 블록 안에서만 접근한다
#![allow(static_mut_refs)]

mod config_store;
mod editor_window;
mod font_manager;
mod hangul_engine;
mod startup_registry;
mod tray_manager;
mod app_info;

use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{LibraryLoader::GetModuleHandleW, Threading::CreateMutexW},
        UI::{
            Input::KeyboardAndMouse::{
                GetAsyncKeyState, RegisterHotKey, SetFocus, UnregisterHotKey, MOD_ALT,
                MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, VK_CONTROL, VK_SHIFT,
            },
            Shell::*,
            WindowsAndMessaging::*,
        },
    },
};

use tray_manager::{
    IDM_ALWAYS_ON_TOP, IDM_EXIT, IDM_GLOBAL_HOTKEY, IDM_START_WITH_WINDOWS, WM_TRAY,
};

// EditControl 메시지 상수 (windows 크레이트 미노출)
const EM_GETSEL: u32 = 0x00B0;
const EM_REPLACESEL: u32 = 0x00C2;
const EM_SETSEL: u32 = 0x00B1;
const VK_BACKSPACE: u32 = 0x08;
const VK_F1: u32 = 0x70;
const BST_UNCHECKED_RAW: usize = 0;
const BST_CHECKED_RAW: usize = 1;
const EN_VSCROLL_RAW: u16 = 0x0602;
// 전역 단축키 (Ctrl+Alt+Shift+G 창 열기)
const HOTKEY_ID_OPEN: i32 = 1;

// 전역 상태 (Win32 콜백에서 접근하기 위해 전역으로 관리)
static mut G_HWND_MAIN: HWND = HWND(0 as _);
static mut G_HWND_EDIT: HWND = HWND(0 as _);
static mut G_HWND_STATUS_BAR: HWND = HWND(0 as _);
static mut G_HWND_GUTTER: HWND = HWND(0 as _);
static mut G_HWND_MODE_HAN: HWND = HWND(0 as _);
static mut G_HWND_MODE_ENG: HWND = HWND(0 as _);
static mut G_HWND_TOPMOST_CHECK: HWND = HWND(0 as _);
static mut G_HWND_WRAP_CHECK: HWND = HWND(0 as _);
static mut G_CONFIG: Option<config_store::AppConfig> = None;
static mut G_NOTIFY_ICON_DATA: Option<NOTIFYICONDATAW> = None;
static mut G_EMBEDDED_FONT: Option<font_manager::EmbeddedFont> = None;
static mut G_SINGLE_INSTANCE_MUTEX: Option<HANDLE> = None;

// 물리키 기반 조합 상태
static mut G_COMPOSER: Option<hangul_engine::HangulComposer> = None;
static mut G_COMPOSE_START: Option<u32> = None;
static mut G_COMPOSE_LEN_UTF16: u32 = 0;
static mut G_SUPPRESS_NEXT_WM_CHAR: bool = false;

// 에디터 컨트롤의 원본 WndProc 보관용
static mut G_ORIG_EDIT_PROC: WNDPROC = None;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Han,
    Eng,
}

static mut G_INPUT_MODE: InputMode = InputMode::Han;

fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

fn map_physical_key_to_eng(vk: u32, shift: bool) -> Option<char> {
    if !(u32::from(b'A')..=u32::from(b'Z')).contains(&vk) {
        return None;
    }

    let base = char::from_u32(vk)?.to_ascii_lowercase();
    let mapped = if shift {
        match base {
            'r' | 'e' | 'q' | 't' | 'w' | 'o' | 'p' => base.to_ascii_uppercase(),
            _ => base,
        }
    } else {
        base
    };

    Some(mapped)
}

fn is_modifier_key(vk: u32) -> bool {
    vk == VK_SHIFT.0 as u32 || vk == VK_CONTROL.0 as u32
}

fn is_word_boundary(ch: char) -> bool {
    ch.is_whitespace()
        || ch.is_ascii_punctuation()
        || matches!(ch, '·' | '…' | '—' | '–' | '「' | '」' | '『' | '』')
}

fn is_punctuation_boundary(ch: char) -> bool {
    is_word_boundary(ch) && !ch.is_whitespace()
}

fn find_ctrl_backspace_delete_from(chars: &[char], cursor: usize) -> usize {
    let mut delete_from = cursor.min(chars.len());

    while delete_from > 0 && chars[delete_from - 1].is_whitespace() {
        delete_from -= 1;
    }
    if delete_from > 0 && is_punctuation_boundary(chars[delete_from - 1]) {
        return delete_from - 1;
    }
    while delete_from > 0 && !is_word_boundary(chars[delete_from - 1]) {
        delete_from -= 1;
    }
    delete_from
}

unsafe fn composer_mut() -> &'static mut hangul_engine::HangulComposer {
    if G_COMPOSER.is_none() {
        G_COMPOSER = Some(hangul_engine::HangulComposer::new());
    }
    G_COMPOSER.as_mut().unwrap()
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

unsafe fn replace_selection(hwnd: HWND, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    SendMessageW(
        hwnd,
        EM_REPLACESEL,
        WPARAM(1),
        LPARAM(wide.as_ptr() as isize),
    );
}

unsafe fn clear_composition_tracking() {
    G_COMPOSE_START = None;
    G_COMPOSE_LEN_UTF16 = 0;
    composer_mut().clear();
}

unsafe fn commit_composition() {
    if composer_mut().has_pending() {
        clear_composition_tracking();
    }
}

unsafe fn sync_mode_radio(mode: InputMode) {
    if G_HWND_MODE_HAN == HWND(0 as _) || G_HWND_MODE_ENG == HWND(0 as _) {
        return;
    }
    let (han_checked, eng_checked) = match mode {
        InputMode::Han => (BST_CHECKED_RAW, BST_UNCHECKED_RAW),
        InputMode::Eng => (BST_UNCHECKED_RAW, BST_CHECKED_RAW),
    };
    SendMessageW(
        G_HWND_MODE_HAN,
        BM_SETCHECK,
        WPARAM(han_checked),
        LPARAM(0),
    );
    SendMessageW(
        G_HWND_MODE_ENG,
        BM_SETCHECK,
        WPARAM(eng_checked),
        LPARAM(0),
    );
}

unsafe fn sync_topmost_checkbox(is_on: bool) {
    if G_HWND_TOPMOST_CHECK == HWND(0 as _) {
        return;
    }
    let check_state = if is_on { BST_CHECKED_RAW } else { BST_UNCHECKED_RAW };
    SendMessageW(
        G_HWND_TOPMOST_CHECK,
        BM_SETCHECK,
        WPARAM(check_state),
        LPARAM(0),
    );
}

unsafe fn sync_wrap_checkbox(is_on: bool) {
    if G_HWND_WRAP_CHECK == HWND(0 as _) {
        return;
    }
    let check_state = if is_on { BST_CHECKED_RAW } else { BST_UNCHECKED_RAW };
    SendMessageW(
        G_HWND_WRAP_CHECK,
        BM_SETCHECK,
        WPARAM(check_state),
        LPARAM(0),
    );
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

/// Wrap 토글: ES_AUTOHSCROLL은 생성 후 변경 불가하므로 EDIT를 재생성한다.
unsafe fn toggle_word_wrap(hwnd: HWND) {
    commit_composition();

    let Some(cfg) = G_CONFIG.as_mut() else {
        return;
    };
    cfg.word_wrap = !cfg.word_wrap;
    let word_wrap = cfg.word_wrap;
    config_store::save(cfg);
    sync_wrap_checkbox(word_wrap);

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
    let edit_w =
        rect.right - editor_window::EDIT_OUTER_MARGIN * 2 - editor_window::GUTTER_WIDTH;
    let edit_h = rect.bottom
        - editor_window::STATUS_BAR_HEIGHT
        - editor_window::EDIT_OUTER_MARGIN * 2;

    G_HWND_EDIT = editor_window::create_edit_control(hwnd, hinstance, word_wrap, edit_w, edit_h);
    subclass_edit_control(G_HWND_EDIT);
    apply_font_to_edit(G_HWND_EDIT);
    set_edit_text(G_HWND_EDIT, &old_text);
    set_selection(G_HWND_EDIT, sel_start, sel_end);
    editor_window::set_gutter_target(G_HWND_EDIT);

    editor_window::resize_editor_controls(
        G_HWND_STATUS_BAR,
        G_HWND_MODE_HAN,
        G_HWND_MODE_ENG,
        G_HWND_WRAP_CHECK,
        G_HWND_TOPMOST_CHECK,
        G_HWND_GUTTER,
        G_HWND_EDIT,
        rect.right,
        rect.bottom,
    );

    focus_editor();
}

unsafe fn handle_alt_z_word_wrap(hwnd: HWND) -> bool {
    toggle_word_wrap(hwnd);
    G_SUPPRESS_NEXT_WM_CHAR = true;
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
        MessageBoxW(
            None,
            w!("전역 단축키(Ctrl+Alt+Shift+G) 등록에 실패했습니다.\n다른 프로그램이 이미 사용 중일 수 있습니다."),
            PCWSTR(app_name.as_ptr()),
            MB_ICONWARNING | MB_OK,
        );
    }
}

unsafe fn toggle_input_mode() {
    commit_composition();
    G_INPUT_MODE = match G_INPUT_MODE {
        InputMode::Han => InputMode::Eng,
        InputMode::Eng => InputMode::Han,
    };
    sync_mode_radio(G_INPUT_MODE);
}

unsafe fn focus_editor() {
    if G_HWND_EDIT != HWND(0 as _) {
        let _ = SetFocus(G_HWND_EDIT);
    }
}

unsafe fn toggle_input_mode_and_focus_editor() {
    toggle_input_mode();
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

unsafe fn render_composition_text(hwnd: HWND, text: &str) {
    let start = G_COMPOSE_START.unwrap_or(0);
    let end = start + G_COMPOSE_LEN_UTF16;
    set_selection(hwnd, start, end);
    replace_selection(hwnd, text);
    let next_pos = start + utf16_len(text);
    set_selection(hwnd, next_pos, next_pos);
    G_COMPOSE_LEN_UTF16 = utf16_len(text);
}

unsafe fn append_committed_text(hwnd: HWND, text: &str) {
    if text.is_empty() {
        return;
    }
    let start = G_COMPOSE_START.unwrap_or(0);
    let end = start + G_COMPOSE_LEN_UTF16;
    set_selection(hwnd, start, end);
    replace_selection(hwnd, text);
    let next = start + utf16_len(text);
    set_selection(hwnd, next, next);
    G_COMPOSE_START = Some(next);
    G_COMPOSE_LEN_UTF16 = 0;
}

unsafe fn start_composition_at_caret(hwnd: HWND) {
    let (start, end) = get_selection(hwnd);
    if start != end {
        set_selection(hwnd, start, end);
        replace_selection(hwnd, "");
        set_selection(hwnd, start, start);
    }
    G_COMPOSE_START = Some(start);
    G_COMPOSE_LEN_UTF16 = 0;
    composer_mut().clear();
}

unsafe fn handle_ctrl_backspace(hwnd: HWND) {
    if composer_mut().has_pending() {
        let text = composer_mut().pop_key();
        render_composition_text(hwnd, &text);
        if text.is_empty() {
            clear_composition_tracking();
        }
        return;
    }

    let (start, end) = get_selection(hwnd);
    if start != end {
        set_selection(hwnd, start, end);
        replace_selection(hwnd, "");
        set_selection(hwnd, start, start);
        return;
    }

    let len = GetWindowTextLengthW(hwnd) as usize;
    if len == 0 || start == 0 {
        return;
    }
    let mut buf: Vec<u16> = vec![0u16; len + 1];
    GetWindowTextW(hwnd, &mut buf);
    buf.truncate(len);
    let text = String::from_utf16_lossy(&buf);
    let chars: Vec<char> = text.chars().collect();
    let mut cursor = start as usize;
    if cursor > chars.len() {
        cursor = chars.len();
    }
    let delete_from = find_ctrl_backspace_delete_from(&chars, cursor);
    set_selection(hwnd, delete_from as u32, cursor as u32);
    replace_selection(hwnd, "");
    set_selection(hwnd, delete_from as u32, delete_from as u32);
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
    editor_window::init_theme(&cfg.colors);

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
        hIcon: tray_manager::load_themed_icon(hinstance),
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as _),
        ..Default::default()
    };
    RegisterClassW(&wc);

    // WS_VSCROLL은 에디터 컨트롤에 내장되어 있으므로 메인 창에는 불필요
    // 표준 캡션 레이아웃은 유지하되 최소화/최대화 버튼만 제거한다
    let style = WS_OVERLAPPEDWINDOW & !WS_MINIMIZEBOX & !WS_MAXIMIZEBOX;
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

    let hwnd = CreateWindowExW(
        ex_style,
        PCWSTR(class_name.as_ptr()),
        PCWSTR(window_title.as_ptr()),
        style,
        start_x,
        start_y,
        cfg.window.width,
        cfg.window.height,
        hwnd_owner,
        None,
        hinstance,
        None,
    )
    .unwrap();

    G_HWND_MAIN = hwnd;

    // 설정에 따라 전역 단축키 등록 (실패 시 조용히 설정 원복)
    if cfg.global_hotkey_open && !register_open_hotkey(hwnd) {
        if let Some(c) = G_CONFIG.as_mut() {
            c.global_hotkey_open = false;
            config_store::save(c);
        }
    }

    G_NOTIFY_ICON_DATA = Some(tray_manager::register_tray_icon(hwnd, hinstance));

    // 최초 실행 시에도 바로 입력할 수 있게 창을 띄운다
    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = SetForegroundWindow(hwnd);
    if G_HWND_EDIT != HWND(0 as _) {
        let _ = SetFocus(G_HWND_EDIT);
    }

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_F1 {
            toggle_input_mode_and_focus_editor();
            continue;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    cleanup_single_instance_mutex();
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
    sync_topmost_checkbox(cfg.always_on_top);
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
    match msg {
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if wparam.0 as u32 == VK_F1 {
                toggle_input_mode_and_focus_editor();
                return LRESULT(0);
            }
            if msg == WM_SYSKEYDOWN && wparam.0 as u32 == 'Z' as u32 {
                handle_alt_z_word_wrap(hwnd);
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        WM_SYSCHAR => {
            // Alt+Z 메뉴 비프음 억제
            if G_SUPPRESS_NEXT_WM_CHAR && wparam.0 as u32 == 'z' as u32 {
                G_SUPPRESS_NEXT_WM_CHAR = false;
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        WM_ACTIVATE => {
            // Alt+Tab 등으로 창이 활성화되면 바로 타이핑할 수 있게 에디터에 포커스
            if (wparam.0 & 0xFFFF) as u32 != WA_INACTIVE {
                focus_editor();
            }
            LRESULT(0)
        }

        WM_CTLCOLORBTN | WM_CTLCOLORSTATIC => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            let _ = SetBkMode(hdc, OPAQUE);
            let _ = SetBkColor(hdc, editor_window::theme().status_bar);
            LRESULT(editor_window::status_bar_brush().0 as isize)
        }

        WM_CTLCOLOREDIT => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            let _ = SetBkColor(hdc, editor_window::theme().editor_bg);
            LRESULT(editor_window::editor_bg_brush().0 as isize)
        }

        WM_PAINT => {
            // 타이틀바↔상태바, 상태바↔에디터 구분선 (상태바 위아래 1px)
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rect = RECT::default();
            GetClientRect(hwnd, &mut rect).ok();
            let top_line = RECT {
                left: 0,
                top: 0,
                right: rect.right,
                bottom: 1,
            };
            let bottom_line = RECT {
                left: 0,
                top: editor_window::STATUS_BAR_HEIGHT - 1,
                right: rect.right,
                bottom: editor_window::STATUS_BAR_HEIGHT,
            };
            FillRect(hdc, &top_line, editor_window::separator_brush());
            FillRect(hdc, &bottom_line, editor_window::separator_brush());
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }

        WM_CREATE => {
            let word_wrap = G_CONFIG
                .as_ref()
                .map(|cfg| cfg.word_wrap)
                .unwrap_or(true);
            let controls = editor_window::create_editor_controls(hwnd, word_wrap);
            G_HWND_STATUS_BAR = controls.hwnd_status_bar;
            G_HWND_GUTTER = controls.hwnd_gutter;
            G_HWND_MODE_HAN = controls.hwnd_mode_han;
            G_HWND_MODE_ENG = controls.hwnd_mode_eng;
            G_HWND_WRAP_CHECK = controls.hwnd_wrap_check;
            G_HWND_TOPMOST_CHECK = controls.hwnd_topmost_check;
            G_HWND_EDIT = controls.hwnd_edit;

            // 단축키 안내 툴팁 (라디오: F1, Wrap: Alt+Z)
            editor_window::setup_tooltips(hwnd, &controls);

            // Edit 컨트롤 Subclassing (Ctrl+Z, Ctrl+Y 가로채기)
            subclass_edit_control(G_HWND_EDIT);

            G_INPUT_MODE = InputMode::Han;
            sync_mode_radio(G_INPUT_MODE);

            if let Some(cfg) = G_CONFIG.as_ref() {
                sync_topmost_checkbox(cfg.always_on_top);
                sync_wrap_checkbox(cfg.word_wrap);
                if cfg.always_on_top {
                    editor_window::apply_always_on_top(hwnd, true);
                }
            }

            if let Some(font) = font_manager::load_embedded_font() {
                let hfont = font.hfont();
                let small_hfont = font.small_hfont();
                font_manager::apply_font_to_control(G_HWND_EDIT, hfont);
                font_manager::apply_font_to_control(G_HWND_MODE_HAN, hfont);
                font_manager::apply_font_to_control(G_HWND_MODE_ENG, hfont);
                font_manager::apply_font_to_control(G_HWND_WRAP_CHECK, small_hfont);
                font_manager::apply_font_to_control(G_HWND_TOPMOST_CHECK, small_hfont);
                G_EMBEDDED_FONT = Some(font);
                // WM_SETFONT가 포맷 영역을 재계산하므로 여백 재적용
                editor_window::apply_editor_padding(G_HWND_EDIT);
                editor_window::invalidate_gutter();
            }
            LRESULT(0)
        }

        WM_SIZE => {
            let w = (lparam.0 & 0xFFFF) as i32;
            let h = ((lparam.0 >> 16) & 0xFFFF) as i32;
            editor_window::resize_editor_controls(
                G_HWND_STATUS_BAR,
                G_HWND_MODE_HAN,
                G_HWND_MODE_ENG,
                G_HWND_WRAP_CHECK,
                G_HWND_TOPMOST_CHECK,
                G_HWND_GUTTER,
                G_HWND_EDIT,
                w,
                h,
            );
            LRESULT(0)
        }

        WM_HOTKEY => {
            if wparam.0 as i32 == HOTKEY_ID_OPEN {
                tray_manager::show_or_focus_window(hwnd, G_HWND_EDIT);
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
                        commit_composition();
                        G_INPUT_MODE = InputMode::Han;
                        sync_mode_radio(G_INPUT_MODE);
                        if G_HWND_EDIT != HWND(0 as _) {
                            let _ = SetFocus(G_HWND_EDIT);
                        }
                    }
                }
                editor_window::IDC_MODE_ENG => {
                    if notif == BN_CLICKED as u16 {
                        commit_composition();
                        G_INPUT_MODE = InputMode::Eng;
                        sync_mode_radio(G_INPUT_MODE);
                        if G_HWND_EDIT != HWND(0 as _) {
                            let _ = SetFocus(G_HWND_EDIT);
                        }
                    }
                }
                editor_window::IDC_ALWAYS_ON_TOP_CHECK => {
                    if notif == BN_CLICKED as u16 {
                        toggle_always_on_top(hwnd);
                    }
                }
                editor_window::IDC_WORD_WRAP_CHECK => {
                    if notif == BN_CLICKED as u16 {
                        toggle_word_wrap(hwnd);
                    }
                }
                IDM_ALWAYS_ON_TOP => toggle_always_on_top(hwnd),
                IDM_START_WITH_WINDOWS => toggle_start_with_windows(),
                IDM_GLOBAL_HOTKEY => toggle_global_hotkey(hwnd),
                IDM_EXIT => {
                    save_window_bounds(hwnd);
                    cleanup_embedded_font();
                    editor_window::cleanup_theme();
                    cleanup_single_instance_mutex();
                    if let Some(nid) = &G_NOTIFY_ICON_DATA {
                        tray_manager::remove_tray_icon(nid);
                    }
                    PostQuitMessage(0);
                }
                _ => {}
            }
            LRESULT(0)
        }

        WM_TRAY => {
            let event = (lparam.0 & 0xFFFF) as u32;
            match event {
                // 싱글클릭은 보통의 트레이 앱처럼 동작 없음
                WM_LBUTTONUP => {}
                // 더블클릭으로 창 표시/포커스
                WM_LBUTTONDBLCLK => {
                    tray_manager::show_or_focus_window(hwnd, G_HWND_EDIT);
                }
                WM_RBUTTONUP => {
                    let cfg = G_CONFIG.as_ref().unwrap();
                    tray_manager::show_context_menu(hwnd, cfg);
                }
                _ => {}
            }
            LRESULT(0)
        }

        WM_CLOSE => {
            // 닫기 버튼은 종료 대신 숨김 처리하되 마지막 창 위치/크기는 저장
            save_window_bounds(hwnd);
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }

        WM_SYSCOMMAND => {
            let cmd = (wparam.0 & 0xFFF0) as u32;
            if cmd == SC_MINIMIZE {
                // 최소화 명령 가로채기: 닫기 버튼과 동일하게 트레이로 숨김 처리
                save_window_bounds(hwnd);
                let _ = ShowWindow(hwnd, SW_HIDE);
                LRESULT(0)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }

        WM_SETTINGCHANGE => {
            // Windows 라이트/다크 테마 변경 시 트레이 아이콘 색상 갱신
            if let Some(nid) = &mut G_NOTIFY_ICON_DATA {
                let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
                tray_manager::refresh_tray_icon(nid, hinstance);
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            save_window_bounds(hwnd);
            cleanup_embedded_font();
            editor_window::cleanup_theme();
            if let Some(nid) = &G_NOTIFY_ICON_DATA {
                tray_manager::remove_tray_icon(nid);
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

unsafe fn edit_proc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_CHAR || msg == WM_SYSCHAR {
        if wparam.0 == 0x7F || G_SUPPRESS_NEXT_WM_CHAR {
            G_SUPPRESS_NEXT_WM_CHAR = false;
            return LRESULT(0);
        }
    }

    if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
        let key = wparam.0 as u32;
        let ctrl_pressed = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
        let shift_pressed = (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;

        if key == VK_F1 {
            toggle_input_mode_and_focus_editor();
            G_SUPPRESS_NEXT_WM_CHAR = true;
            return LRESULT(0);
        }

        if msg == WM_SYSKEYDOWN && key == 'Z' as u32 {
            handle_alt_z_word_wrap(G_HWND_MAIN);
            return LRESULT(0);
        }

        if is_modifier_key(key) {
            return CallWindowProcW(G_ORIG_EDIT_PROC, hwnd, msg, wparam, lparam);
        }

        if ctrl_pressed && key == 'A' as u32 {
            commit_composition();
            set_selection(hwnd, 0, u32::MAX);
            G_SUPPRESS_NEXT_WM_CHAR = true;
            return LRESULT(0);
        }

        if ctrl_pressed && key == VK_BACKSPACE {
            handle_ctrl_backspace(hwnd);
            G_SUPPRESS_NEXT_WM_CHAR = true;
            return LRESULT(0);
        }

        if ctrl_pressed {
            commit_composition();
            return CallWindowProcW(G_ORIG_EDIT_PROC, hwnd, msg, wparam, lparam);
        }

        if key == VK_BACKSPACE {
            if composer_mut().has_pending() {
                let text = composer_mut().pop_key();
                render_composition_text(hwnd, &text);
                if text.is_empty() {
                    clear_composition_tracking();
                }
                G_SUPPRESS_NEXT_WM_CHAR = true;
                return LRESULT(0);
            }
            return CallWindowProcW(G_ORIG_EDIT_PROC, hwnd, msg, wparam, lparam);
        }

        if G_INPUT_MODE == InputMode::Han {
            if let Some(eng_key) = map_physical_key_to_eng(key, shift_pressed) {
                let (sel_start, sel_end) = get_selection(hwnd);
                if G_COMPOSE_START.is_none() || sel_start != sel_end {
                    start_composition_at_caret(hwnd);
                }
                let step = composer_mut().feed_key(eng_key);
                append_committed_text(hwnd, &step.committed);
                render_composition_text(hwnd, &step.composing);
                G_SUPPRESS_NEXT_WM_CHAR = true;
                return LRESULT(0);
            }
        } else {
            // 영 모드에서는 기본 영어 입력으로 통과
            commit_composition();
            return CallWindowProcW(G_ORIG_EDIT_PROC, hwnd, msg, wparam, lparam);
        }

        // 조합을 끊는 키가 오면 현재 조합을 확정하고 기본 동작으로 넘긴다.
        commit_composition();
    }

    if msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN {
        commit_composition();
    }

    CallWindowProcW(G_ORIG_EDIT_PROC, hwnd, msg, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_physical_key_to_eng() {
        assert_eq!(map_physical_key_to_eng('R' as u32, false), Some('r'));
        assert_eq!(map_physical_key_to_eng('R' as u32, true), Some('R'));
        assert_eq!(map_physical_key_to_eng('G' as u32, false), Some('g'));
        assert_eq!(map_physical_key_to_eng('G' as u32, true), Some('g'));
        assert_eq!(map_physical_key_to_eng(VK_BACKSPACE, false), None);
        assert_eq!(map_physical_key_to_eng('T' as u32, true), Some('T'));
        assert!(is_modifier_key(VK_SHIFT.0 as u32));
    }

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

    #[test]
    fn test_korean_e_backspace_to_ieung() {
        let mut composer = hangul_engine::HangulComposer::new();
        let mut committed = String::new();
        let mut composing = String::new();
        for ch in "gksrnrdjdl".chars() {
            let step = composer.feed_key(ch);
            committed.push_str(&step.committed);
            composing = step.composing;
        }
        assert_eq!(format!("{committed}{composing}"), "한국어이");
        assert_eq!(composer.pop_key(), "ㅇ");
    }

    #[test]
    fn test_ctrl_backspace_korean_word_boundary() {
        let chars: Vec<char> = "입력한다? 입".chars().collect();
        let cursor = chars.len();
        assert_eq!(find_ctrl_backspace_delete_from(&chars, cursor), "입력한다? ".chars().count());
    }

    #[test]
    fn test_ctrl_backspace_deletes_punctuation_at_boundary() {
        let chars: Vec<char> = "입력한다?".chars().collect();
        let cursor = chars.len();
        assert_eq!(find_ctrl_backspace_delete_from(&chars, cursor), "입력한다".chars().count());
    }

    #[test]
    fn test_ctrl_backspace_whitespace_boundary() {
        let chars: Vec<char> = "hello   world".chars().collect();
        let cursor = chars.len();
        assert_eq!(find_ctrl_backspace_delete_from(&chars, cursor), "hello   ".chars().count());
    }
}

// 에디터 창 생성 및 리사이즈/스크롤 관리 모듈

use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::Controls::{
            InitCommonControlsEx, ICC_BAR_CLASSES, INITCOMMONCONTROLSEX, TTF_IDISHWND,
            TTF_SUBCLASS, TTM_ADDTOOLW, TTS_ALWAYSTIP, TTTOOLINFOW,
        },
        UI::WindowsAndMessaging::*,
    },
};

use crate::config_store;
use crate::input_controller::TextBuffer;

pub const EDIT_OUTER_MARGIN: i32 = 8;
pub const STATUS_BAR_HEIGHT: i32 = 34;
// 상태바 컨트롤 상하 여백 (세로 중앙 정렬)
const STATUS_PAD: i32 = 4;
pub const GUTTER_WIDTH: i32 = 44;
pub const IDC_ALWAYS_ON_TOP_CHECK: u32 = 2001;
pub const IDC_MODE_HAN: u32 = 2002;
pub const IDC_MODE_ENG: u32 = 2003;
pub const IDC_WORD_WRAP_CHECK: u32 = 2004;
const WRAP_CHECK_WIDTH: i32 = 70;
// "Always on Top" + 체크박스 여유 (140이면 글자가 Wrap 위로 넘침)
const AOT_CHECK_WIDTH: i32 = 170;
const STATUS_GAP: i32 = 8;
// ㅎ/A 라디오 오른쪽 끝 + 여백
const STATUS_LEFT_ZONE_END: i32 = EDIT_OUTER_MARGIN + 48 + 40 + STATUS_GAP;

/// 상태바 컨트롤이 겹치지 않는 최소 클라이언트 폭.
pub fn min_client_width() -> i32 {
    STATUS_LEFT_ZONE_END + WRAP_CHECK_WIDTH + STATUS_GAP + AOT_CHECK_WIDTH + EDIT_OUTER_MARGIN
}

/// 에디터·상태바가 쓸 수 있는 최소 클라이언트 높이.
pub fn min_client_height() -> i32 {
    STATUS_BAR_HEIGHT + 120
}
const GUTTER_CLASS: PCWSTR = w!("k2g_gutter");
// 에디터 첫 행이 상태바 구분선에 붙지 않도록 주는 내부 여백
const EDITOR_PADDING_TOP: i32 = 6;
const EDITOR_PADDING_LEFT: i32 = 4;

// EditControl 메시지 상수 (windows 크레이트 미노출)
const EM_GETSEL_RAW: u32 = 0x00B0;
const EM_GETRECT: u32 = 0x00B2;
const EM_SETRECT: u32 = 0x00B3;
const EM_GETLINECOUNT: u32 = 0x00BA;
const EM_LINEINDEX: u32 = 0x00BB;
const EM_GETFIRSTVISIBLELINE: u32 = 0x00CE;
const EM_SETSEL_RAW: u32 = 0x00B1;
const EM_REPLACESEL_RAW: u32 = 0x00C2;
const EM_SETLIMITTEXT_RAW: u32 = 0x00C5;

/// 실제 EDIT 컨트롤을 입력 컨트롤러의 TextBuffer로 감싼다.
pub struct EditBuffer(pub HWND);

impl TextBuffer for EditBuffer {
    fn text_utf16(&self) -> Vec<u16> {
        unsafe {
            let len = GetWindowTextLengthW(self.0) as usize;
            let mut buf: Vec<u16> = vec![0u16; len + 1];
            let copied = GetWindowTextW(self.0, &mut buf) as usize;
            buf.truncate(copied.min(len));
            buf
        }
    }

    fn selection(&self) -> (u32, u32) {
        let mut start = 0u32;
        let mut end = 0u32;
        unsafe {
            SendMessageW(
                self.0,
                EM_GETSEL_RAW,
                WPARAM(&mut start as *mut _ as usize),
                LPARAM(&mut end as *mut _ as isize),
            );
        }
        (start, end)
    }

    fn set_selection(&mut self, start: u32, end: u32) {
        unsafe {
            SendMessageW(self.0, EM_SETSEL_RAW, WPARAM(start as usize), LPARAM(end as isize));
        }
    }

    fn replace_selection(&mut self, text: &str) {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            SendMessageW(
                self.0,
                EM_REPLACESEL_RAW,
                WPARAM(1),
                LPARAM(wide.as_ptr() as isize),
            );
        }
    }
}

// 거터가 참조하는 에디터 핸들 (단일 인스턴스 앱이므로 모듈 전역으로 관리)
static mut G_GUTTER_HWND: HWND = HWND(0 as _);
static mut G_GUTTER_EDIT: HWND = HWND(0 as _);

// ---------- 색상 테마 ----------

pub struct Theme {
    pub line_number: COLORREF,
    pub line_number_current: COLORREF,
    pub editor_bg: COLORREF,
    gutter_brush: HBRUSH,
    status_brush: HBRUSH,
    editor_brush: HBRUSH,
    separator_brush: HBRUSH,
}

static mut G_THEME: Option<Theme> = None;

unsafe fn resolve_color(value: Option<&str>, fallback: u32) -> COLORREF {
    let resolved = value
        .and_then(config_store::parse_hex_color)
        .unwrap_or(fallback);
    COLORREF(resolved)
}

/// config의 colors 설정으로 테마를 초기화한다. 값이 없거나 잘못되면 시스템 기본색.
pub unsafe fn init_theme(colors: &config_store::ColorConfig) {
    let gutter_bg = resolve_color(colors.gutter_background.as_deref(), GetSysColor(COLOR_BTNFACE));
    let line_number = resolve_color(colors.line_number.as_deref(), GetSysColor(COLOR_GRAYTEXT));
    let status_bar = resolve_color(colors.status_bar.as_deref(), GetSysColor(COLOR_BTNFACE));
    let editor_bg = resolve_color(colors.editor_background.as_deref(), GetSysColor(COLOR_WINDOW));
    let line_number_current = COLORREF(GetSysColor(COLOR_WINDOWTEXT));
    let separator = COLORREF(GetSysColor(COLOR_BTNSHADOW));

    G_THEME = Some(Theme {
        line_number,
        line_number_current,
        editor_bg,
        gutter_brush: CreateSolidBrush(gutter_bg),
        status_brush: CreateSolidBrush(status_bar),
        editor_brush: CreateSolidBrush(editor_bg),
        separator_brush: CreateSolidBrush(separator),
    });
}

pub unsafe fn theme() -> &'static Theme {
    if G_THEME.is_none() {
        init_theme(&config_store::ColorConfig::default());
    }
    G_THEME.as_ref().unwrap()
}

pub unsafe fn status_bar_brush() -> HBRUSH {
    theme().status_brush
}

pub unsafe fn editor_bg_brush() -> HBRUSH {
    theme().editor_brush
}

pub unsafe fn separator_brush() -> HBRUSH {
    theme().separator_brush
}

pub unsafe fn cleanup_theme() {
    if let Some(t) = G_THEME.take() {
        let _ = DeleteObject(HGDIOBJ(t.gutter_brush.0));
        let _ = DeleteObject(HGDIOBJ(t.status_brush.0));
        let _ = DeleteObject(HGDIOBJ(t.editor_brush.0));
        let _ = DeleteObject(HGDIOBJ(t.separator_brush.0));
    }
}

pub struct EditorControls {
    pub hwnd_mode_han: HWND,
    pub hwnd_mode_eng: HWND,
    pub hwnd_wrap_check: HWND,
    pub hwnd_topmost_check: HWND,
    pub hwnd_gutter: HWND,
    pub hwnd_edit: HWND,
}

unsafe fn create_mode_han_radio(parent: HWND, hinstance: HINSTANCE) -> HWND {
    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        w!("ㅎ"),
        WS_CHILD
            | WS_VISIBLE
            | WS_TABSTOP
            | WINDOW_STYLE(BS_AUTORADIOBUTTON as u32)
            | WINDOW_STYLE(WS_GROUP.0),
        EDIT_OUTER_MARGIN,
        STATUS_PAD,
        40,
        STATUS_BAR_HEIGHT - STATUS_PAD * 2,
        parent,
        HMENU(IDC_MODE_HAN as usize as *mut core::ffi::c_void),
        hinstance,
        None,
    )
    .unwrap_or(HWND(0 as _))
}

unsafe fn create_mode_eng_radio(parent: HWND, hinstance: HINSTANCE) -> HWND {
    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        w!("A"),
        WS_CHILD
            | WS_VISIBLE
            | WS_TABSTOP
            | WINDOW_STYLE(BS_AUTORADIOBUTTON as u32),
        EDIT_OUTER_MARGIN + 48,
        STATUS_PAD,
        40,
        STATUS_BAR_HEIGHT - STATUS_PAD * 2,
        parent,
        HMENU(IDC_MODE_ENG as usize as *mut core::ffi::c_void),
        hinstance,
        None,
    )
    .unwrap_or(HWND(0 as _))
}

unsafe fn create_wrap_checkbox(parent: HWND, hinstance: HINSTANCE) -> HWND {
    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        w!("Wrap"),
        WS_CHILD
            | WS_VISIBLE
            | WS_TABSTOP
            | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
        0,
        STATUS_PAD,
        WRAP_CHECK_WIDTH,
        STATUS_BAR_HEIGHT - STATUS_PAD * 2,
        parent,
        HMENU(IDC_WORD_WRAP_CHECK as usize as *mut core::ffi::c_void),
        hinstance,
        None,
    )
    .unwrap_or(HWND(0 as _))
}

unsafe fn create_topmost_checkbox(parent: HWND, hinstance: HINSTANCE) -> HWND {
    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        w!("Always on Top"),
        WS_CHILD
            | WS_VISIBLE
            | WS_TABSTOP
            | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
        0,
        STATUS_PAD,
        AOT_CHECK_WIDTH,
        STATUS_BAR_HEIGHT - STATUS_PAD * 2,
        parent,
        HMENU(IDC_ALWAYS_ON_TOP_CHECK as usize as *mut core::ffi::c_void),
        hinstance,
        None,
    )
    .unwrap_or(HWND(0 as _))
}

// ---------- 행 번호 거터 ----------

unsafe fn register_gutter_class(hinstance: HINSTANCE) {
    static mut REGISTERED: bool = false;
    if REGISTERED {
        return;
    }
    let wc = WNDCLASSW {
        lpfnWndProc: Some(gutter_wnd_proc),
        hInstance: hinstance,
        lpszClassName: GUTTER_CLASS,
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
        ..Default::default()
    };
    RegisterClassW(&wc);
    REGISTERED = true;
}

unsafe fn create_line_gutter(parent: HWND, hinstance: HINSTANCE) -> HWND {
    register_gutter_class(hinstance);
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        GUTTER_CLASS,
        None,
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
        0,
        STATUS_BAR_HEIGHT,
        GUTTER_WIDTH,
        1,
        parent,
        None,
        hinstance,
        None,
    )
    .unwrap_or(HWND(0 as _));
    G_GUTTER_HWND = hwnd;
    hwnd
}

/// 거터가 행 번호 계산에 사용할 에디터 핸들을 지정한다 (Wrap 토글로 재생성 시 갱신).
pub unsafe fn set_gutter_target(hwnd_edit: HWND) {
    G_GUTTER_EDIT = hwnd_edit;
    invalidate_gutter();
}

/// 거터를 다시 그리게 한다 (스크롤·입력·리사이즈 후 호출).
pub unsafe fn invalidate_gutter() {
    if G_GUTTER_HWND != HWND(0 as _) {
        let _ = InvalidateRect(G_GUTTER_HWND, None, false);
    }
}

unsafe extern "system" fn gutter_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint_gutter(hwnd);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn paint_gutter(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);

    let mut client = RECT::default();
    GetClientRect(hwnd, &mut client).ok();
    FillRect(hdc, &client, theme().gutter_brush);

    // 거터↔에디터 구분선 (오른쪽 끝 1px)
    let separator_rect = RECT {
        left: client.right - 1,
        top: client.top,
        right: client.right,
        bottom: client.bottom,
    };
    FillRect(hdc, &separator_rect, theme().separator_brush);

    let edit = G_GUTTER_EDIT;
    if edit == HWND(0 as _) {
        let _ = EndPaint(hwnd, &ps);
        return;
    }

    // 에디터와 같은 폰트로 행 높이를 일치시킨다
    let hfont = HFONT(SendMessageW(edit, WM_GETFONT, WPARAM(0), LPARAM(0)).0 as _);
    let old_font = if hfont != HFONT(0 as _) {
        SelectObject(hdc, HGDIOBJ(hfont.0))
    } else {
        HGDIOBJ(0 as _)
    };

    let mut tm = TEXTMETRICW::default();
    let _ = GetTextMetricsW(hdc, &mut tm);
    let line_height = (tm.tmHeight + tm.tmExternalLeading).max(1);

    // 에디터 포맷 영역의 상단 오프셋에 맞춰 시작 y를 정렬
    let mut fmt_rect = RECT::default();
    SendMessageW(
        edit,
        EM_GETRECT,
        WPARAM(0),
        LPARAM(&mut fmt_rect as *mut _ as isize),
    );

    let first_visible =
        SendMessageW(edit, EM_GETFIRSTVISIBLELINE, WPARAM(0), LPARAM(0)).0 as i32;
    let line_count = SendMessageW(edit, EM_GETLINECOUNT, WPARAM(0), LPARAM(0)).0 as i32;

    let text_len = GetWindowTextLengthW(edit) as usize;
    let mut buf: Vec<u16> = vec![0u16; text_len + 1];
    GetWindowTextW(edit, &mut buf);
    buf.truncate(text_len);

    // 현재 커서(캐럿)가 속한 논리 행 번호 계산
    let newline = '\n' as u16;
    let mut sel_start = 0u32;
    let mut sel_end = 0u32;
    SendMessageW(
        edit,
        EM_GETSEL_RAW,
        WPARAM(&mut sel_start as *mut _ as usize),
        LPARAM(&mut sel_end as *mut _ as isize),
    );
    let caret = (sel_end as usize).min(buf.len());
    let caret_line = buf[..caret].iter().filter(|&&c| c == newline).count() + 1;

    let _ = SetBkMode(hdc, TRANSPARENT);

    let visible_rows = client.bottom / line_height + 1;
    // 시각행 시작 문자 인덱스는 단조 증가하므로 개행 수를 증분 계산한다
    let mut scan_pos: usize = 0;
    let mut newlines_before: usize = 0;

    for i in 0..visible_rows {
        let vl = first_visible + i;
        if vl >= line_count {
            break;
        }
        let char_start = SendMessageW(edit, EM_LINEINDEX, WPARAM(vl as usize), LPARAM(0)).0;
        if char_start < 0 {
            break;
        }
        let char_start = (char_start as usize).min(buf.len());

        while scan_pos < char_start {
            if buf[scan_pos] == newline {
                newlines_before += 1;
            }
            scan_pos += 1;
        }

        // 논리 행의 첫 시각행에만 번호를 표시 (Wrap으로 꺾인 행은 공백)
        let is_logical_start = char_start == 0 || buf.get(char_start - 1) == Some(&newline);
        if !is_logical_start {
            continue;
        }

        let line_no = newlines_before + 1;
        // 현재 커서 행은 강조색으로 표시
        let color = if line_no == caret_line {
            theme().line_number_current
        } else {
            theme().line_number
        };
        let _ = SetTextColor(hdc, color);

        let number = line_no.to_string();
        let mut wide: Vec<u16> = number.encode_utf16().collect();
        let mut rect = RECT {
            left: 0,
            top: fmt_rect.top + i * line_height,
            right: client.right - 8,
            bottom: fmt_rect.top + (i + 1) * line_height,
        };
        DrawTextW(hdc, &mut wide, &mut rect, DT_RIGHT | DT_TOP | DT_SINGLELINE);
    }

    if old_font != HGDIOBJ(0 as _) {
        SelectObject(hdc, old_font);
    }
    let _ = EndPaint(hwnd, &ps);
}

// ---------- 툴팁 ----------

unsafe fn create_tooltip_window(parent: HWND, hinstance: HINSTANCE) -> HWND {
    let icc = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_BAR_CLASSES,
    };
    let _ = InitCommonControlsEx(&icc);

    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("tooltips_class32"),
        None,
        WS_POPUP | WINDOW_STYLE(TTS_ALWAYSTIP as u32),
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        parent,
        None,
        hinstance,
        None,
    )
    .unwrap_or(HWND(0 as _))
}

unsafe fn add_tooltip(hwnd_tip: HWND, parent: HWND, target: HWND, text: &str) {
    if hwnd_tip == HWND(0 as _) || target == HWND(0 as _) {
        return;
    }
    // TTM_ADDTOOLW는 문자열을 내부 복사하므로 임시 버퍼로 충분하다
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let ti = TTTOOLINFOW {
        cbSize: std::mem::size_of::<TTTOOLINFOW>() as u32,
        uFlags: TTF_IDISHWND | TTF_SUBCLASS,
        hwnd: parent,
        uId: target.0 as usize,
        lpszText: PWSTR(wide.as_ptr() as *mut u16),
        ..Default::default()
    };
    SendMessageW(
        hwnd_tip,
        TTM_ADDTOOLW,
        WPARAM(0),
        LPARAM(&ti as *const _ as isize),
    );
}

/// 상태바 컨트롤에 단축키 안내 툴팁을 붙인다.
pub unsafe fn setup_tooltips(parent: HWND, controls: &EditorControls) {
    let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
    let hwnd_tip = create_tooltip_window(parent, hinstance);
    add_tooltip(hwnd_tip, parent, controls.hwnd_mode_han, "F1");
    add_tooltip(hwnd_tip, parent, controls.hwnd_mode_eng, "F1");
    add_tooltip(hwnd_tip, parent, controls.hwnd_wrap_check, "Alt + Z");
}

// ---------- 에디터 ----------

/// wrap 설정에 맞는 멀티라인 EDIT 컨트롤을 생성한다.
/// word_wrap=true 이면 가로 스크롤 없이 자동 줄바꿈, false 이면 가로 스크롤 활성.
pub unsafe fn create_edit_control(
    parent: HWND,
    hinstance: HINSTANCE,
    word_wrap: bool,
    width: i32,
    height: i32,
) -> HWND {
    let mut style = WS_CHILD
        | WS_VISIBLE
        | WS_CLIPSIBLINGS
        | WS_VSCROLL
        | WINDOW_STYLE(ES_MULTILINE as u32)
        | WINDOW_STYLE(ES_AUTOVSCROLL as u32)
        | WINDOW_STYLE(ES_WANTRETURN as u32);

    if !word_wrap {
        style |= WS_HSCROLL | WINDOW_STYLE(ES_AUTOHSCROLL as u32);
    }

    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("EDIT"),
        None,
        style,
        GUTTER_WIDTH,
        STATUS_BAR_HEIGHT,
        width.max(1),
        height.max(1),
        parent,
        None,
        hinstance,
        None,
    )
    .unwrap_or(HWND(0 as _));
    // 기본 입력 한도(32,767자)를 해제한다 (0 = 최대)
    SendMessageW(hwnd, EM_SETLIMITTEXT_RAW, WPARAM(0), LPARAM(0));
    apply_editor_padding(hwnd);
    hwnd
}

/// 에디터 포맷 영역에 상단·왼쪽 여백을 준다.
/// WM_SIZE·WM_SETFONT가 포맷 영역을 재계산하므로 그 이후마다 다시 호출해야 한다.
pub unsafe fn apply_editor_padding(hwnd_edit: HWND) {
    if hwnd_edit == HWND(0 as _) {
        return;
    }
    let mut rect = RECT::default();
    GetClientRect(hwnd_edit, &mut rect).ok();
    rect.top += EDITOR_PADDING_TOP;
    rect.left += EDITOR_PADDING_LEFT;
    SendMessageW(
        hwnd_edit,
        EM_SETRECT,
        WPARAM(0),
        LPARAM(&rect as *const _ as isize),
    );
}

/// 상태바와 에디터 컨트롤을 생성한다.
pub unsafe fn create_editor_controls(parent: HWND, word_wrap: bool) -> EditorControls {
    let mut rect = RECT::default();
    GetClientRect(parent, &mut rect).ok();
    let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();

    // 상태바 배경은 STATIC 없이 부모 WM_PAINT에서 그린다
    // (STATIC + 테마 체크박스 형제 페인팅이 리사이즈 잔상을 만든다)
    let hwnd_gutter = create_line_gutter(parent, hinstance);
    let hedit = create_edit_control(
        parent,
        hinstance,
        word_wrap,
        rect.right - GUTTER_WIDTH,
        rect.bottom - STATUS_BAR_HEIGHT,
    );
    set_gutter_target(hedit);
    let hwnd_mode_han = create_mode_han_radio(parent, hinstance);
    let hwnd_mode_eng = create_mode_eng_radio(parent, hinstance);
    let hwnd_wrap_check = create_wrap_checkbox(parent, hinstance);
    let hwnd_topmost_check = create_topmost_checkbox(parent, hinstance);

    EditorControls {
        hwnd_mode_han,
        hwnd_mode_eng,
        hwnd_wrap_check,
        hwnd_topmost_check,
        hwnd_gutter,
        hwnd_edit: hedit,
    }
}

/// 상태바와 에디터 컨트롤 크기를 새 클라이언트 영역에 맞게 조정한다.
///
/// 최소 폭은 WM_GETMINMAXINFO로 막는다. DeferWindowPos로 원자 배치한다.
pub unsafe fn resize_editor_controls(
    hwnd_mode_han: HWND,
    hwnd_mode_eng: HWND,
    hwnd_wrap_check: HWND,
    hwnd_topmost_check: HWND,
    hwnd_gutter: HWND,
    hwnd_edit: HWND,
    width: i32,
    height: i32,
) {
    let width = width.max(1);
    let height = height.max(1);
    let control_h = (STATUS_BAR_HEIGHT - STATUS_PAD * 2).max(16);
    let aot_x = (width - AOT_CHECK_WIDTH - EDIT_OUTER_MARGIN).max(STATUS_LEFT_ZONE_END);
    let wrap_x = (aot_x - WRAP_CHECK_WIDTH - STATUS_GAP).max(STATUS_LEFT_ZONE_END);
    let edit_h = (height - STATUS_BAR_HEIGHT).max(1);
    let edit_w = (width - GUTTER_WIDTH).max(1);

    let Ok(mut hdwp) = BeginDeferWindowPos(6) else {
        return;
    };

    // 실패 시 HDWP는 무효이므로 즉시 중단한다
    macro_rules! defer {
        ($hwnd:expr, $x:expr, $y:expr, $cx:expr, $cy:expr, $flags:expr) => {
            if $hwnd != HWND(0 as _) {
                match DeferWindowPos(hdwp, $hwnd, HWND(0 as _), $x, $y, $cx, $cy, $flags) {
                    Ok(next) => hdwp = next,
                    Err(_) => return,
                }
            }
        };
    }

    defer!(
        hwnd_mode_han,
        EDIT_OUTER_MARGIN,
        STATUS_PAD,
        40,
        control_h,
        SWP_NOZORDER | SWP_NOACTIVATE
    );
    defer!(
        hwnd_mode_eng,
        EDIT_OUTER_MARGIN + 48,
        STATUS_PAD,
        40,
        control_h,
        SWP_NOZORDER | SWP_NOACTIVATE
    );
    defer!(
        hwnd_wrap_check,
        wrap_x,
        STATUS_PAD,
        WRAP_CHECK_WIDTH,
        control_h,
        SWP_NOZORDER | SWP_NOACTIVATE
    );
    defer!(
        hwnd_topmost_check,
        aot_x,
        STATUS_PAD,
        AOT_CHECK_WIDTH,
        control_h,
        SWP_NOZORDER | SWP_NOACTIVATE
    );
    defer!(
        hwnd_gutter,
        0,
        STATUS_BAR_HEIGHT,
        GUTTER_WIDTH,
        edit_h,
        SWP_NOZORDER | SWP_NOACTIVATE
    );
    defer!(
        hwnd_edit,
        GUTTER_WIDTH,
        STATUS_BAR_HEIGHT,
        edit_w,
        edit_h,
        SWP_NOZORDER | SWP_NOACTIVATE
    );

    let _ = EndDeferWindowPos(hdwp);

    if hwnd_gutter != HWND(0 as _) {
        let _ = InvalidateRect(hwnd_gutter, None, true);
    }
    if hwnd_edit != HWND(0 as _) {
        apply_editor_padding(hwnd_edit);
    }
}

/// 항상 위 설정을 창에 즉시 반영한다.
pub unsafe fn apply_always_on_top(hwnd: HWND, on_top: bool) {
    let insert_after = if on_top { HWND_TOPMOST } else { HWND_NOTOPMOST };
    SetWindowPos(hwnd, insert_after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE).ok();
}

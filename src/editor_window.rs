// 에디터 창 생성 및 리사이즈/스크롤 관리 모듈
// 상단바(한/A 세그먼트·Wrap·항상 위)·행 번호 거터·EDIT 컨트롤, 라이트/다크 팔레트, DPI 배율

use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::{
            Gdi::*,
            GdiPlus::{
                FillModeAlternate, GdipAddPathArc, GdipClosePathFigure, GdipCreateFromHDC,
                GdipCreatePath, GdipCreatePen1, GdipCreateSolidFill, GdipDeleteBrush,
                GdipDeleteGraphics, GdipDeletePath, GdipDeletePen, GdipDrawPath, GdipFillPath,
                GdipSetSmoothingMode, GdiplusShutdown, GdiplusStartup, GdiplusStartupInput,
                GpBrush, GpGraphics, GpPath, GpPen, GpSolidFill, SmoothingModeAntiAlias, Status,
                UnitPixel,
            },
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::Controls::{
            InitCommonControlsEx, SetWindowTheme, ICC_BAR_CLASSES, INITCOMMONCONTROLSEX,
            TTF_SUBCLASS, TTM_ADDTOOLW, TTM_NEWTOOLRECTW, TTS_ALWAYSTIP, TTS_NOPREFIX,
            TTTOOLINFOW,
        },
        UI::Input::KeyboardAndMouse::{
            ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
        },
        UI::WindowsAndMessaging::*,
    },
};

use crate::app_info;
use crate::config_store::{self, AppConfig};
use crate::input_controller::TextBuffer;

pub const IDC_ALWAYS_ON_TOP_CHECK: u32 = 2001;
pub const IDC_MODE_HAN: u32 = 2002;
pub const IDC_MODE_ENG: u32 = 2003;
pub const IDC_WORD_WRAP_CHECK: u32 = 2004;

// ---------- DPI 배율 (치수는 모두 96 DPI 기준 값) ----------

const TOOLBAR_HEIGHT: i32 = 36;
const GUTTER_WIDTH: i32 = 44;
// 에디터 첫 행이 상단바 구분선에 붙지 않도록 주는 내부 여백
const EDITOR_PADDING_TOP: i32 = 6;
const EDITOR_PADDING_LEFT: i32 = 4;

static mut G_DPI: u32 = 96;

pub unsafe fn set_dpi(dpi: u32) {
    G_DPI = dpi.max(96 / 2);
}

pub unsafe fn dpi() -> u32 {
    G_DPI
}

/// 96 DPI 기준 px을 현재 DPI로 환산한다.
pub unsafe fn px(v: i32) -> i32 {
    (v * G_DPI as i32 + 48) / 96
}

pub unsafe fn toolbar_height() -> i32 {
    px(TOOLBAR_HEIGHT)
}

pub unsafe fn gutter_width() -> i32 {
    px(GUTTER_WIDTH)
}

/// 상단바 컨트롤이 겹치지 않는 최소 클라이언트 폭 (배지는 숨겨진 상태 기준).
pub unsafe fn min_client_width() -> i32 {
    let toolbar = TB_MARGIN + SEG_CELL_W * 2 + 16 + BTN_W + BTN_GAP + BTN_W + TB_MARGIN;
    px(toolbar.max(240))
}

/// 에디터·상단바가 쓸 수 있는 최소 클라이언트 높이.
pub unsafe fn min_client_height() -> i32 {
    toolbar_height() + px(120)
}

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

const fn rgb(hex: u32) -> COLORREF {
    // 0xRRGGBB → COLORREF(0x00BBGGRR)
    COLORREF(((hex & 0xFF) << 16) | (hex & 0xFF00) | ((hex >> 16) & 0xFF))
}

fn blend(a: COLORREF, b: COLORREF, t: f32) -> COLORREF {
    let ch = |shift: u32| {
        let x = ((a.0 >> shift) & 0xFF) as f32;
        let y = ((b.0 >> shift) & 0xFF) as f32;
        ((x * t + y * (1.0 - t)).round() as u32) << shift
    };
    COLORREF(ch(0) | ch(8) | ch(16))
}

fn argb(c: COLORREF) -> u32 {
    let r = c.0 & 0xFF;
    let g = (c.0 >> 8) & 0xFF;
    let b = (c.0 >> 16) & 0xFF;
    0xFF00_0000 | (r << 16) | (g << 8) | b
}

pub struct Theme {
    pub dark: bool,
    pub text: COLORREF,
    text_secondary: COLORREF,
    pub line_number: COLORREF,
    pub line_number_current: COLORREF,
    pub editor_bg: COLORREF,
    accent: COLORREF,
    on_accent: COLORREF,
    accent_soft: COLORREF,
    accent_soft_hover: COLORREF,
    segment_bg: COLORREF,
    hover: COLORREF,
    pressed: COLORREF,
    badge_border: COLORREF,
    gutter_brush: HBRUSH,
    toolbar_brush: HBRUSH,
    editor_brush: HBRUSH,
    separator_brush: HBRUSH,
}

static mut G_THEME: Option<Theme> = None;

fn resolve_color(value: Option<&str>, fallback: COLORREF) -> COLORREF {
    value
        .and_then(config_store::parse_hex_color)
        .map(COLORREF)
        .unwrap_or(fallback)
}

/// 설정 색상(라이트: colors, 다크: colors_dark)과 내장 팔레트로 테마를 (재)생성한다.
pub unsafe fn init_theme(cfg: &AppConfig, dark: bool, accent: COLORREF) {
    cleanup_theme();

    let (gutter, line_number, toolbar, editor) = if dark {
        let c = &cfg.colors_dark;
        (
            resolve_color(c.gutter_background.as_deref(), rgb(0x202020)),
            resolve_color(c.line_number.as_deref(), rgb(0x7A7A7A)),
            resolve_color(c.status_bar.as_deref(), rgb(0x202020)),
            resolve_color(c.editor_background.as_deref(), rgb(0x1C1C1C)),
        )
    } else {
        let c = &cfg.colors;
        (
            resolve_color(c.gutter_background.as_deref(), rgb(0xF3F3F3)),
            resolve_color(c.line_number.as_deref(), rgb(0x8A8A8A)),
            resolve_color(c.status_bar.as_deref(), rgb(0xF3F3F3)),
            resolve_color(c.editor_background.as_deref(), rgb(0xFFFFFF)),
        )
    };

    let (text, text_secondary, separator, segment_bg, hover, pressed, badge_border, on_accent) =
        if dark {
            (
                rgb(0xE6E6E6),
                rgb(0xA0A0A0),
                rgb(0x333333),
                blend(rgb(0xFFFFFF), toolbar, 0.06),
                blend(rgb(0xFFFFFF), toolbar, 0.08),
                blend(rgb(0xFFFFFF), toolbar, 0.04),
                rgb(0x4A4A4A),
                rgb(0x000000),
            )
        } else {
            (
                rgb(0x1A1A1A),
                rgb(0x5F5F5F),
                rgb(0xE0E0E0),
                blend(rgb(0x000000), toolbar, 0.05),
                blend(rgb(0x000000), toolbar, 0.06),
                blend(rgb(0x000000), toolbar, 0.10),
                rgb(0xC8C8C8),
                rgb(0xFFFFFF),
            )
        };

    G_THEME = Some(Theme {
        dark,
        text,
        text_secondary,
        line_number,
        line_number_current: text,
        editor_bg: editor,
        accent,
        on_accent,
        accent_soft: blend(accent, toolbar, 0.18),
        accent_soft_hover: blend(accent, toolbar, 0.28),
        segment_bg,
        hover,
        pressed,
        badge_border,
        gutter_brush: CreateSolidBrush(gutter),
        toolbar_brush: CreateSolidBrush(toolbar),
        editor_brush: CreateSolidBrush(editor),
        separator_brush: CreateSolidBrush(separator),
    });
}

pub unsafe fn theme() -> &'static Theme {
    if G_THEME.is_none() {
        init_theme(&AppConfig::default(), false, rgb(0x005FB8));
    }
    G_THEME.as_ref().unwrap()
}

pub unsafe fn editor_bg_brush() -> HBRUSH {
    theme().editor_brush
}

pub unsafe fn cleanup_theme() {
    if let Some(t) = G_THEME.take() {
        let _ = DeleteObject(HGDIOBJ(t.gutter_brush.0));
        let _ = DeleteObject(HGDIOBJ(t.toolbar_brush.0));
        let _ = DeleteObject(HGDIOBJ(t.editor_brush.0));
        let _ = DeleteObject(HGDIOBJ(t.separator_brush.0));
    }
}

/// 스크롤바·툴팁 같은 공용 컨트롤의 테마를 라이트/다크에 맞춘다.
pub unsafe fn apply_control_theme(hwnd: HWND) {
    if hwnd == HWND(0 as _) {
        return;
    }
    let sub_app = if theme().dark { w!("DarkMode_Explorer") } else { w!("Explorer") };
    let _ = SetWindowTheme(hwnd, sub_app, PCWSTR::null());
}

pub struct EditorControls {
    pub hwnd_toolbar: HWND,
    pub hwnd_gutter: HWND,
    pub hwnd_edit: HWND,
}

// ---------- 상단바 ----------

const TOOLBAR_CLASS: PCWSTR = w!("k2g_toolbar");
const WM_MOUSELEAVE: u32 = 0x02A3;

// 상단바 치수 (96 DPI 기준)
const TB_MARGIN: i32 = 8;
const CTRL_H: i32 = 26;
const SEG_CELL_W: i32 = 34;
const SEG_RADIUS: f32 = 6.0;
const SEG_INSET: i32 = 2;
const BTN_W: i32 = 30;
const BTN_GAP: i32 = 4;
const BTN_RADIUS: f32 = 5.0;
const BADGE_H: i32 = 18;
const BADGE_PAD_X: i32 = 6;
const BADGE_RADIUS: f32 = 4.0;
const BADGE_GAP: i32 = 8;

// Segoe Fluent Icons / Segoe MDL2 Assets 글리프
const GLYPH_WRAP: &str = "\u{E751}";
const GLYPH_PIN: &str = "\u{E718}";
const GLYPH_PINNED: &str = "\u{E840}";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Han = 1,
    Eng,
    ModeBadge,
    Wrap,
    Pin,
}

const ALL_PARTS: [Part; 5] = [Part::Han, Part::Eng, Part::ModeBadge, Part::Wrap, Part::Pin];

struct ToolbarState {
    han: bool,
    wrap: bool,
    topmost: bool,
    hover: Option<Part>,
    pressed: Option<Part>,
    tracking_leave: bool,
}

static mut G_TOOLBAR: ToolbarState = ToolbarState {
    han: true,
    wrap: true,
    topmost: false,
    hover: None,
    pressed: None,
    tracking_leave: false,
};
static mut G_TOOLBAR_HWND: HWND = HWND(0 as _);
static mut G_TOOLTIP_HWND: HWND = HWND(0 as _);

struct ToolbarFonts {
    mode: HFONT,
    icon: HFONT,
    badge: HFONT,
    mode_badge_w: i32,
    wrap_badge_w: i32,
}

static mut G_TB_FONTS: Option<ToolbarFonts> = None;
static mut G_GDIPLUS_TOKEN: usize = 0;

unsafe fn create_font(height_px: i32, weight: i32, face: PCWSTR) -> HFONT {
    CreateFontW(
        -height_px,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32,
        CLEARTYPE_QUALITY.0 as u32,
        (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
        face,
    )
}

/// Win11의 Segoe Fluent Icons, 없으면(Win10) Segoe MDL2 Assets
unsafe fn create_icon_font(hdc: HDC, height_px: i32) -> HFONT {
    let fluent = create_font(height_px, FW_NORMAL.0 as i32, w!("Segoe Fluent Icons"));
    let old = SelectObject(hdc, HGDIOBJ(fluent.0));
    let mut face = [0u16; 64];
    let n = GetTextFaceW(hdc, Some(&mut face)) as usize;
    SelectObject(hdc, old);
    if String::from_utf16_lossy(&face[..n.min(face.len())]).trim_end_matches('\0') == "Segoe Fluent Icons" {
        return fluent;
    }
    let _ = DeleteObject(HGDIOBJ(fluent.0));
    create_font(height_px, FW_NORMAL.0 as i32, w!("Segoe MDL2 Assets"))
}

unsafe fn text_width(hdc: HDC, font: HFONT, text: &str) -> i32 {
    let wide: Vec<u16> = text.encode_utf16().collect();
    let old = SelectObject(hdc, HGDIOBJ(font.0));
    let mut size = SIZE::default();
    let _ = GetTextExtentPoint32W(hdc, &wide, &mut size);
    SelectObject(hdc, old);
    size.cx
}

unsafe fn delete_toolbar_fonts() {
    if let Some(f) = G_TB_FONTS.take() {
        let _ = DeleteObject(HGDIOBJ(f.mode.0));
        let _ = DeleteObject(HGDIOBJ(f.icon.0));
        let _ = DeleteObject(HGDIOBJ(f.badge.0));
    }
}

/// 현재 DPI로 상단바 폰트를 다시 만든다 (생성 시·WM_DPICHANGED).
pub unsafe fn refresh_toolbar_fonts() {
    delete_toolbar_fonts();
    let hdc = GetDC(None);
    let mode = create_font(px(14), FW_NORMAL.0 as i32, w!("Pretendard"));
    let icon = create_icon_font(hdc, px(16));
    let badge = create_font(px(12), FW_NORMAL.0 as i32, w!("Segoe UI"));
    let mode_badge_w = text_width(hdc, badge, app_info::SHORTCUT_TOGGLE_MODE) + px(BADGE_PAD_X) * 2;
    let wrap_badge_w = text_width(hdc, badge, app_info::SHORTCUT_WORD_WRAP) + px(BADGE_PAD_X) * 2;
    ReleaseDC(None, hdc);
    G_TB_FONTS = Some(ToolbarFonts { mode, icon, badge, mode_badge_w, wrap_badge_w });
    if G_TOOLBAR_HWND != HWND(0 as _) {
        update_tooltip_rects(G_TOOLBAR_HWND);
        let _ = InvalidateRect(G_TOOLBAR_HWND, None, false);
    }
}

struct ToolbarLayout {
    segment: RECT,
    han: RECT,
    eng: RECT,
    mode_badge: Option<RECT>,
    wrap: RECT,
    wrap_badge: Option<RECT>,
    pin: RECT,
}

impl ToolbarLayout {
    fn rect(&self, part: Part) -> Option<RECT> {
        match part {
            Part::Han => Some(self.han),
            Part::Eng => Some(self.eng),
            Part::ModeBadge => self.mode_badge,
            Part::Wrap => Some(self.wrap),
            Part::Pin => Some(self.pin),
        }
    }
}

fn rect_xywh(x: i32, y: i32, w: i32, h: i32) -> RECT {
    RECT { left: x, top: y, right: x + w, bottom: y + h }
}

unsafe fn toolbar_layout(width: i32) -> ToolbarLayout {
    let fonts = G_TB_FONTS.as_ref();
    let mode_badge_w = fonts.map_or(px(28), |f| f.mode_badge_w);
    let wrap_badge_w = fonts.map_or(px(44), |f| f.wrap_badge_w);

    let h = toolbar_height();
    let ctrl_h = px(CTRL_H);
    let y = (h - ctrl_h) / 2;
    let margin = px(TB_MARGIN);
    let cell_w = px(SEG_CELL_W);
    let btn_w = px(BTN_W);
    let badge_h = px(BADGE_H);
    let badge_y = (h - badge_h) / 2;

    let segment = rect_xywh(margin, y, cell_w * 2, ctrl_h);
    let wrap_full_w = btn_w + wrap_badge_w + px(4);
    // 배지까지 모두 들어갈 폭이 아니면 배지를 숨긴다
    let needed = margin * 2 + cell_w * 2 + px(BADGE_GAP) + mode_badge_w + px(16)
        + wrap_full_w + px(BTN_GAP) + btn_w;
    let show_badges = width >= needed;

    let pin = rect_xywh(width - margin - btn_w, y, btn_w, ctrl_h);
    let wrap_w = if show_badges { wrap_full_w } else { btn_w };
    let wrap = rect_xywh(pin.left - px(BTN_GAP) - wrap_w, y, wrap_w, ctrl_h);

    ToolbarLayout {
        segment,
        han: rect_xywh(segment.left, y, cell_w, ctrl_h),
        eng: rect_xywh(segment.left + cell_w, y, cell_w, ctrl_h),
        mode_badge: show_badges
            .then(|| rect_xywh(segment.right + px(BADGE_GAP), badge_y, mode_badge_w, badge_h)),
        wrap,
        wrap_badge: show_badges
            .then(|| rect_xywh(wrap.left + btn_w - px(2), badge_y, wrap_badge_w, badge_h)),
        pin,
    }
}

unsafe fn client_width(hwnd: HWND) -> i32 {
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    rc.right
}

unsafe fn hit_test(hwnd: HWND, x: i32, y: i32) -> Option<Part> {
    let layout = toolbar_layout(client_width(hwnd));
    let pt = POINT { x, y };
    ALL_PARTS
        .into_iter()
        .find(|&p| layout.rect(p).is_some_and(|r| PtInRect(&r, pt).as_bool()))
}

/// 모드·Wrap·항상 위 상태를 상단바에 반영한다.
pub unsafe fn toolbar_set_state(han: bool, wrap: bool, topmost: bool) {
    G_TOOLBAR.han = han;
    G_TOOLBAR.wrap = wrap;
    G_TOOLBAR.topmost = topmost;
    if G_TOOLBAR_HWND != HWND(0 as _) {
        let _ = InvalidateRect(G_TOOLBAR_HWND, None, false);
    }
}

unsafe fn part_command(part: Part) -> u32 {
    match part {
        Part::Han => IDC_MODE_HAN,
        Part::Eng => IDC_MODE_ENG,
        Part::ModeBadge => {
            if G_TOOLBAR.han {
                IDC_MODE_ENG
            } else {
                IDC_MODE_HAN
            }
        }
        Part::Wrap => IDC_WORD_WRAP_CHECK,
        Part::Pin => IDC_ALWAYS_ON_TOP_CHECK,
    }
}

unsafe fn register_toolbar_class(hinstance: HINSTANCE) {
    static mut REGISTERED: bool = false;
    if REGISTERED {
        return;
    }
    let input = GdiplusStartupInput { GdiplusVersion: 1, ..Default::default() };
    let _ = GdiplusStartup(&mut G_GDIPLUS_TOKEN, &input, std::ptr::null_mut());

    let wc = WNDCLASSW {
        lpfnWndProc: Some(toolbar_wnd_proc),
        hInstance: hinstance,
        lpszClassName: TOOLBAR_CLASS,
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
        ..Default::default()
    };
    RegisterClassW(&wc);
    REGISTERED = true;
}

unsafe fn create_toolbar(parent: HWND, hinstance: HINSTANCE) -> HWND {
    register_toolbar_class(hinstance);
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        TOOLBAR_CLASS,
        None,
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
        0,
        0,
        1,
        toolbar_height(),
        parent,
        None,
        hinstance,
        None,
    )
    .unwrap_or(HWND(0 as _));
    G_TOOLBAR_HWND = hwnd;
    refresh_toolbar_fonts();
    create_toolbar_tooltips(parent, hwnd, hinstance);
    hwnd
}

unsafe extern "system" fn toolbar_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    match msg {
        WM_PAINT => {
            paint_toolbar(hwnd);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_SIZE => {
            update_tooltip_rects(hwnd);
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if !G_TOOLBAR.tracking_leave {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                G_TOOLBAR.tracking_leave = TrackMouseEvent(&mut tme).is_ok();
            }
            let hover = hit_test(hwnd, x, y);
            if hover != G_TOOLBAR.hover {
                G_TOOLBAR.hover = hover;
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            G_TOOLBAR.tracking_leave = false;
            G_TOOLBAR.hover = None;
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            G_TOOLBAR.pressed = hit_test(hwnd, x, y);
            if G_TOOLBAR.pressed.is_some() {
                SetCapture(hwnd);
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let pressed = G_TOOLBAR.pressed.take();
            let _ = ReleaseCapture();
            let _ = InvalidateRect(hwnd, None, false);
            if let Some(part) = pressed {
                if hit_test(hwnd, x, y) == Some(part) {
                    let id = part_command(part);
                    let parent = GetParent(hwnd).unwrap_or_default();
                    SendMessageW(
                        parent,
                        WM_COMMAND,
                        WPARAM((BN_CLICKED as usize) << 16 | id as usize),
                        LPARAM(hwnd.0 as isize),
                    );
                }
            }
            LRESULT(0)
        }
        WM_CAPTURECHANGED => {
            if G_TOOLBAR.pressed.take().is_some() {
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// --- GDI+ 둥근 사각형 (안티앨리어싱) ---

unsafe fn fill_round_rect(g: *mut GpGraphics, r: RECT, radius: f32, color: COLORREF) {
    let mut path: *mut GpPath = std::ptr::null_mut();
    if GdipCreatePath(FillModeAlternate, &mut path) != Status(0) {
        return;
    }
    add_round_rect(path, r.left as f32, r.top as f32, (r.right - r.left) as f32, (r.bottom - r.top) as f32, radius);
    let mut brush: *mut GpSolidFill = std::ptr::null_mut();
    if GdipCreateSolidFill(argb(color), &mut brush) == Status(0) {
        GdipFillPath(g, brush as *mut GpBrush, path);
        GdipDeleteBrush(brush as *mut GpBrush);
    }
    GdipDeletePath(path);
}

unsafe fn stroke_round_rect(g: *mut GpGraphics, r: RECT, radius: f32, color: COLORREF) {
    let mut path: *mut GpPath = std::ptr::null_mut();
    if GdipCreatePath(FillModeAlternate, &mut path) != Status(0) {
        return;
    }
    // 1px 선이 픽셀 경계에 맞도록 0.5px 안쪽으로
    add_round_rect(
        path,
        r.left as f32 + 0.5,
        r.top as f32 + 0.5,
        (r.right - r.left) as f32 - 1.0,
        (r.bottom - r.top) as f32 - 1.0,
        radius,
    );
    let mut pen: *mut GpPen = std::ptr::null_mut();
    if GdipCreatePen1(argb(color), 1.0, UnitPixel, &mut pen) == Status(0) {
        GdipDrawPath(g, pen, path);
        GdipDeletePen(pen);
    }
    GdipDeletePath(path);
}

unsafe fn add_round_rect(path: *mut GpPath, x: f32, y: f32, w: f32, h: f32, radius: f32) {
    let d = (radius * 2.0).min(w).min(h);
    GdipAddPathArc(path, x, y, d, d, 180.0, 90.0);
    GdipAddPathArc(path, x + w - d, y, d, d, 270.0, 90.0);
    GdipAddPathArc(path, x + w - d, y + h - d, d, d, 0.0, 90.0);
    GdipAddPathArc(path, x, y + h - d, d, d, 90.0, 90.0);
    GdipClosePathFigure(path);
}

fn inset(r: RECT, d: i32) -> RECT {
    RECT { left: r.left + d, top: r.top + d, right: r.right - d, bottom: r.bottom - d }
}

unsafe fn draw_text_centered(hdc: HDC, font: HFONT, text: &str, mut r: RECT, color: COLORREF) {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let old = SelectObject(hdc, HGDIOBJ(font.0));
    let _ = SetTextColor(hdc, color);
    DrawTextW(hdc, &mut wide, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
    SelectObject(hdc, old);
}

unsafe fn paint_toolbar(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc_screen = BeginPaint(hwnd, &mut ps);
    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);
    let (w, h) = (client.right.max(1), client.bottom.max(1));

    // 깜빡임 없이 그리기 위해 메모리 DC에 그린 뒤 한 번에 복사
    let hdc = CreateCompatibleDC(hdc_screen);
    let bmp = CreateCompatibleBitmap(hdc_screen, w, h);
    let old_bmp = SelectObject(hdc, HGDIOBJ(bmp.0));

    let t = theme();
    let st = &G_TOOLBAR;
    let layout = toolbar_layout(w);

    FillRect(hdc, &client, t.toolbar_brush);
    // 상단바 아래 구분선 (상단바↔에디터)
    let bottom_line = RECT { left: 0, top: h - 1, right: w, bottom: h };
    FillRect(hdc, &bottom_line, t.separator_brush);

    let is_hover = |p: Part| st.hover == Some(p) && (st.pressed.is_none() || st.pressed == Some(p));
    let is_pressed = |p: Part| st.pressed == Some(p) && st.hover == Some(p);

    // 도형 (GDI+)
    let mut g: *mut GpGraphics = std::ptr::null_mut();
    if GdipCreateFromHDC(hdc, &mut g) == Status(0) {
        GdipSetSmoothingMode(g, SmoothingModeAntiAlias);

        // 한/A 세그먼트: 바탕 + 선택 칸
        fill_round_rect(g, layout.segment, px(SEG_RADIUS as i32) as f32, t.segment_bg);
        for (part, selected) in [(Part::Han, st.han), (Part::Eng, !st.han)] {
            let cell = inset(layout.rect(part).unwrap(), px(SEG_INSET));
            let radius = (px(SEG_RADIUS as i32) - px(SEG_INSET)) as f32;
            if selected {
                fill_round_rect(g, cell, radius, t.accent);
            } else if is_pressed(part) {
                fill_round_rect(g, cell, radius, t.pressed);
            } else if is_hover(part) {
                fill_round_rect(g, cell, radius, t.hover);
            }
        }

        // Wrap·항상 위 토글 버튼
        for (part, on) in [(Part::Wrap, st.wrap), (Part::Pin, st.topmost)] {
            let r = layout.rect(part).unwrap();
            let radius = px(BTN_RADIUS as i32) as f32;
            let fill = match (on, is_pressed(part), is_hover(part)) {
                (true, true, _) => Some(t.accent_soft),
                (true, false, true) => Some(t.accent_soft_hover),
                (true, false, false) => Some(t.accent_soft),
                (false, true, _) => Some(t.pressed),
                (false, false, true) => Some(t.hover),
                (false, false, false) => None,
            };
            if let Some(color) = fill {
                fill_round_rect(g, r, radius, color);
            }
        }

        // 단축키 배지 테두리
        let badge_radius = px(BADGE_RADIUS as i32) as f32;
        if let Some(r) = layout.mode_badge {
            if is_hover(Part::ModeBadge) {
                fill_round_rect(g, r, badge_radius, t.hover);
            }
            stroke_round_rect(g, r, badge_radius, t.badge_border);
        }
        if let Some(r) = layout.wrap_badge {
            stroke_round_rect(g, r, badge_radius, t.badge_border);
        }

        GdipDeleteGraphics(g);
    }

    // 글자 (GDI, ClearType)
    let _ = SetBkMode(hdc, TRANSPARENT);
    if let Some(fonts) = G_TB_FONTS.as_ref() {
        let han_color = if st.han { t.on_accent } else { t.text };
        let eng_color = if st.han { t.text } else { t.on_accent };
        draw_text_centered(hdc, fonts.mode, "한", layout.han, han_color);
        draw_text_centered(hdc, fonts.mode, "A", layout.eng, eng_color);

        let wrap_icon = RECT { right: layout.wrap.left + px(BTN_W), ..layout.wrap };
        draw_text_centered(hdc, fonts.icon, GLYPH_WRAP, wrap_icon, if st.wrap { t.accent } else { t.text });
        let pin_glyph = if st.topmost { GLYPH_PINNED } else { GLYPH_PIN };
        draw_text_centered(hdc, fonts.icon, pin_glyph, layout.pin, if st.topmost { t.accent } else { t.text });

        if let Some(r) = layout.mode_badge {
            draw_text_centered(hdc, fonts.badge, app_info::SHORTCUT_TOGGLE_MODE, r, t.text_secondary);
        }
        if let Some(r) = layout.wrap_badge {
            draw_text_centered(hdc, fonts.badge, app_info::SHORTCUT_WORD_WRAP, r, t.text_secondary);
        }
    }

    let _ = BitBlt(hdc_screen, 0, 0, w, h, hdc, 0, 0, SRCCOPY);
    SelectObject(hdc, old_bmp);
    let _ = DeleteObject(HGDIOBJ(bmp.0));
    let _ = DeleteDC(hdc);
    let _ = EndPaint(hwnd, &ps);
}

// --- 상단바 툴팁 (영역별) ---

fn part_tooltip(part: Part) -> String {
    match part {
        Part::Han | Part::Eng | Part::ModeBadge => {
            format!("Hangul / English  ({})", app_info::SHORTCUT_TOGGLE_MODE)
        }
        Part::Wrap => format!("Word wrap  ({})", app_info::SHORTCUT_WORD_WRAP),
        Part::Pin => "Always on top".to_string(),
    }
}

unsafe fn create_toolbar_tooltips(owner: HWND, toolbar: HWND, hinstance: HINSTANCE) {
    let icc = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_BAR_CLASSES,
    };
    let _ = InitCommonControlsEx(&icc);

    let tip = CreateWindowExW(
        WS_EX_TOPMOST,
        w!("tooltips_class32"),
        None,
        WS_POPUP | WINDOW_STYLE(TTS_ALWAYSTIP | TTS_NOPREFIX),
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        owner,
        None,
        hinstance,
        None,
    )
    .unwrap_or(HWND(0 as _));
    if tip == HWND(0 as _) {
        return;
    }
    G_TOOLTIP_HWND = tip;
    apply_control_theme(tip);

    let layout = toolbar_layout(client_width(toolbar));
    for part in ALL_PARTS {
        // TTM_ADDTOOLW는 문자열을 내부 복사하므로 임시 버퍼로 충분하다
        let text: Vec<u16> = part_tooltip(part).encode_utf16().chain(std::iter::once(0)).collect();
        let ti = TTTOOLINFOW {
            cbSize: std::mem::size_of::<TTTOOLINFOW>() as u32,
            uFlags: TTF_SUBCLASS,
            hwnd: toolbar,
            uId: part as usize,
            rect: layout.rect(part).unwrap_or_default(),
            lpszText: PWSTR(text.as_ptr() as *mut u16),
            ..Default::default()
        };
        SendMessageW(tip, TTM_ADDTOOLW, WPARAM(0), LPARAM(&ti as *const _ as isize));
    }
}

unsafe fn update_tooltip_rects(toolbar: HWND) {
    if G_TOOLTIP_HWND == HWND(0 as _) {
        return;
    }
    let layout = toolbar_layout(client_width(toolbar));
    for part in ALL_PARTS {
        let ti = TTTOOLINFOW {
            cbSize: std::mem::size_of::<TTTOOLINFOW>() as u32,
            hwnd: toolbar,
            uId: part as usize,
            rect: layout.rect(part).unwrap_or_default(),
            ..Default::default()
        };
        SendMessageW(G_TOOLTIP_HWND, TTM_NEWTOOLRECTW, WPARAM(0), LPARAM(&ti as *const _ as isize));
    }
}

/// 테마 변경 후 상단바·툴팁을 다시 그린다.
pub unsafe fn refresh_toolbar_theme() {
    apply_control_theme(G_TOOLTIP_HWND);
    if G_TOOLBAR_HWND != HWND(0 as _) {
        let _ = InvalidateRect(G_TOOLBAR_HWND, None, false);
    }
}

// ---------- 행 번호 거터 ----------

const GUTTER_CLASS: PCWSTR = w!("k2g_gutter");

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
        toolbar_height(),
        gutter_width(),
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
            right: client.right - px(8),
            bottom: fmt_rect.top + (i + 1) * line_height,
        };
        DrawTextW(hdc, &mut wide, &mut rect, DT_RIGHT | DT_TOP | DT_SINGLELINE);
    }

    if old_font != HGDIOBJ(0 as _) {
        SelectObject(hdc, old_font);
    }
    let _ = EndPaint(hwnd, &ps);
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
        gutter_width(),
        toolbar_height(),
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
    // 스크롤바를 라이트/다크 테마에 맞춘다
    apply_control_theme(hwnd);
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
    rect.top += px(EDITOR_PADDING_TOP);
    rect.left += px(EDITOR_PADDING_LEFT);
    SendMessageW(
        hwnd_edit,
        EM_SETRECT,
        WPARAM(0),
        LPARAM(&rect as *const _ as isize),
    );
}

/// 상단바·거터·에디터 컨트롤을 생성한다.
pub unsafe fn create_editor_controls(parent: HWND, word_wrap: bool) -> EditorControls {
    let mut rect = RECT::default();
    GetClientRect(parent, &mut rect).ok();
    let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();

    let hwnd_toolbar = create_toolbar(parent, hinstance);
    let hwnd_gutter = create_line_gutter(parent, hinstance);
    let hedit = create_edit_control(
        parent,
        hinstance,
        word_wrap,
        rect.right - gutter_width(),
        rect.bottom - toolbar_height(),
    );
    set_gutter_target(hedit);

    EditorControls {
        hwnd_toolbar,
        hwnd_gutter,
        hwnd_edit: hedit,
    }
}

/// 상단바·거터·에디터 크기를 새 클라이언트 영역에 맞게 조정한다.
///
/// 최소 폭은 WM_GETMINMAXINFO로 막는다. DeferWindowPos로 원자 배치한다.
pub unsafe fn resize_editor_controls(
    hwnd_toolbar: HWND,
    hwnd_gutter: HWND,
    hwnd_edit: HWND,
    width: i32,
    height: i32,
) {
    let width = width.max(1);
    let height = height.max(1);
    let bar_h = toolbar_height();
    let gutter_w = gutter_width();
    let edit_h = (height - bar_h).max(1);
    let edit_w = (width - gutter_w).max(1);

    let Ok(mut hdwp) = BeginDeferWindowPos(3) else {
        return;
    };

    // 실패 시 HDWP는 무효이므로 즉시 중단한다
    macro_rules! defer {
        ($hwnd:expr, $x:expr, $y:expr, $cx:expr, $cy:expr) => {
            if $hwnd != HWND(0 as _) {
                match DeferWindowPos(
                    hdwp,
                    $hwnd,
                    HWND(0 as _),
                    $x,
                    $y,
                    $cx,
                    $cy,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                ) {
                    Ok(next) => hdwp = next,
                    Err(_) => return,
                }
            }
        };
    }

    defer!(hwnd_toolbar, 0, 0, width, bar_h);
    defer!(hwnd_gutter, 0, bar_h, gutter_w, edit_h);
    defer!(hwnd_edit, gutter_w, bar_h, edit_w, edit_h);

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

/// 종료 시 GDI 자원 해제.
pub unsafe fn cleanup() {
    cleanup_theme();
    delete_toolbar_fonts();
    if G_GDIPLUS_TOKEN != 0 {
        GdiplusShutdown(G_GDIPLUS_TOKEN);
        G_GDIPLUS_TOKEN = 0;
    }
}

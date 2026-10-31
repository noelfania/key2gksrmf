// 창 투명도 모듈
// 레이어드 창 알파, 투명도 슬라이더 팝업

use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::{
            Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE},
            Gdi::*,
        },
        System::{LibraryLoader::GetModuleHandleW, SystemInformation::GetTickCount},
        UI::Controls::{
            TBM_SETLINESIZE, TBM_SETPAGESIZE, TBM_SETPOS, TBM_SETRANGEMAX, TBM_SETRANGEMIN,
            TBS_HORZ, TBS_NOTICKS, TRACKBAR_CLASSW,
        },
        UI::Input::KeyboardAndMouse::SetFocus,
        UI::WindowsAndMessaging::*,
    },
};

use crate::config_store::MAX_TRANSPARENCY;
use crate::editor_window::{self, px};

// ---------- 레이어드 알파 ----------

/// 투명도(%)를 창에 적용한다. 0이면 레이어드 스타일을 떼어 일반 창으로 되돌린다.
pub unsafe fn apply(hwnd: HWND, transparency: u8) {
    let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    let layered = ex & WS_EX_LAYERED.0 != 0;
    if transparency == 0 {
        if layered {
            SetWindowLongW(hwnd, GWL_EXSTYLE, (ex & !WS_EX_LAYERED.0) as i32);
            // 레이어드 해제 직후에는 다시 그려야 잔상이 남지 않는다
            let _ = RedrawWindow(
                hwnd,
                None,
                None,
                RDW_ERASE | RDW_INVALIDATE | RDW_FRAME | RDW_ALLCHILDREN,
            );
        }
        return;
    }
    if !layered {
        SetWindowLongW(hwnd, GWL_EXSTYLE, (ex | WS_EX_LAYERED.0) as i32);
    }
    let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha_for(transparency), LWA_ALPHA);
}

fn alpha_for(transparency: u8) -> u8 {
    let t = transparency.min(MAX_TRANSPARENCY) as u32;
    (255 * (100 - t) / 100) as u8
}

// ---------- 슬라이더 팝업 ----------

const POPUP_CLASS: PCWSTR = w!("k2g_opacity_popup");
// 팝업 치수 (96 DPI 기준)
const POPUP_W: i32 = 220;
const POPUP_H: i32 = 40;
const POPUP_PAD: i32 = 8;
const LABEL_W: i32 = 40;
const POPUP_GAP: i32 = 4;
const TBM_GETPOS: u32 = 0x0400;
const VK_ESCAPE_RAW: u16 = 0x1B;
const VK_RETURN_RAW: u16 = 0x0D;
const DWMWA_WINDOW_CORNER_PREFERENCE: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(33);
const DWMWCP_ROUNDSMALL: i32 = 3;
// 바깥 클릭으로 닫힌 직후 같은 클릭이 버튼에 닿아 다시 열리는 것을 막는 시간 (ms)
const REOPEN_GUARD_MS: u32 = 300;

/// 값이 바뀔 때마다 (투명도, 확정 여부)로 호출된다. 확정은 팝업이 닫힐 때 한 번.
pub type OnChange = unsafe fn(u8, bool);

struct SliderPopup {
    hwnd: HWND,
    trackbar: HWND,
    font: HFONT,
    initial: u8,
    value: u8,
    on_change: OnChange,
}

static mut G_POPUP: Option<SliderPopup> = None;
static mut G_LAST_CLOSE_TICK: u32 = 0;

/// 투명도 버튼(`anchor`, 화면 좌표) 아래에 슬라이더를 연다. 이미 열려 있으면 닫는다.
pub unsafe fn toggle_slider(owner: HWND, anchor: RECT, value: u8, on_change: OnChange) {
    if G_POPUP.is_some() {
        close_slider(true);
        return;
    }
    if GetTickCount().wrapping_sub(G_LAST_CLOSE_TICK) < REOPEN_GUARD_MS {
        return;
    }

    let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
    register_popup_class(hinstance);

    let (w, h) = (px(POPUP_W), px(POPUP_H));
    let (x, y) = popup_position(anchor, w, h);
    // owner(메인 창)가 항상 위면 팝업도 그 위에 떠야 한다
    let owner_ex = GetWindowLongW(owner, GWL_EXSTYLE) as u32;
    let ex = WS_EX_TOOLWINDOW | WINDOW_EX_STYLE(owner_ex & WS_EX_TOPMOST.0);
    let Ok(hwnd) = CreateWindowExW(
        ex,
        POPUP_CLASS,
        None,
        WS_POPUP | WS_BORDER,
        x,
        y,
        w,
        h,
        owner,
        None,
        hinstance,
        None,
    ) else {
        return;
    };
    let corner = DWMWCP_ROUNDSMALL;
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &corner as *const i32 as *const core::ffi::c_void,
        std::mem::size_of::<i32>() as u32,
    );

    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);
    let pad = px(POPUP_PAD);
    let trackbar = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        TRACKBAR_CLASSW,
        None,
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(TBS_HORZ | TBS_NOTICKS),
        pad,
        0,
        (client.right - pad * 2 - px(LABEL_W)).max(1),
        client.bottom,
        hwnd,
        None,
        hinstance,
        None,
    )
    .unwrap_or_default();
    SendMessageW(trackbar, TBM_SETRANGEMIN, WPARAM(0), LPARAM(0));
    SendMessageW(trackbar, TBM_SETRANGEMAX, WPARAM(1), LPARAM(MAX_TRANSPARENCY as isize));
    SendMessageW(trackbar, TBM_SETPAGESIZE, WPARAM(0), LPARAM(10));
    SendMessageW(trackbar, TBM_SETLINESIZE, WPARAM(0), LPARAM(5));
    SendMessageW(trackbar, TBM_SETPOS, WPARAM(1), LPARAM(value as isize));
    editor_window::apply_control_theme(trackbar);

    let font = CreateFontW(
        -px(13),
        0,
        0,
        0,
        FW_NORMAL.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32,
        CLEARTYPE_QUALITY.0 as u32,
        (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
        w!("Segoe UI"),
    );

    G_POPUP = Some(SliderPopup { hwnd, trackbar, font, initial: value, value, on_change });
    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = SetFocus(trackbar);
}

/// 팝업을 닫는다. `commit`이 false(Esc)면 연 시점의 투명도로 되돌린다.
pub unsafe fn close_slider(commit: bool) {
    let Some(popup) = G_POPUP.take() else {
        return;
    };
    G_LAST_CLOSE_TICK = GetTickCount();
    let _ = DestroyWindow(popup.hwnd);
    let _ = DeleteObject(HGDIOBJ(popup.font.0));
    let value = if commit { popup.value } else { popup.initial };
    (popup.on_change)(value, true);
}

/// 메시지 루프에서 먼저 호출한다. 팝업의 Esc(취소)·Enter(확정)를 처리했으면 true.
pub unsafe fn pre_translate(msg: &MSG) -> bool {
    let Some(popup) = G_POPUP.as_ref() else {
        return false;
    };
    if msg.message != WM_KEYDOWN || (msg.hwnd != popup.hwnd && msg.hwnd != popup.trackbar) {
        return false;
    }
    match msg.wParam.0 as u16 {
        k if k == VK_ESCAPE_RAW => close_slider(false),
        k if k == VK_RETURN_RAW => close_slider(true),
        _ => return false,
    }
    true
}

/// 테마가 바뀌면 열린 팝업도 다시 칠한다.
pub unsafe fn refresh_slider_theme() {
    if let Some(popup) = G_POPUP.as_ref() {
        editor_window::apply_control_theme(popup.trackbar);
        let _ = RedrawWindow(
            popup.hwnd,
            None,
            None,
            RDW_ERASE | RDW_INVALIDATE | RDW_FRAME | RDW_ALLCHILDREN,
        );
    }
}

/// 버튼 아래, 오른쪽 끝을 맞춰 두고 모니터 작업 영역 밖으로 나가지 않게 한다.
unsafe fn popup_position(anchor: RECT, w: i32, h: i32) -> (i32, i32) {
    let mut x = anchor.right - w;
    let mut y = anchor.bottom + px(POPUP_GAP);
    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    if GetMonitorInfoW(MonitorFromRect(&anchor, MONITOR_DEFAULTTONEAREST), &mut mi).as_bool() {
        let work = mi.rcWork;
        if y + h > work.bottom {
            y = anchor.top - px(POPUP_GAP) - h;
        }
        x = x.clamp(work.left, (work.right - w).max(work.left));
        y = y.clamp(work.top, (work.bottom - h).max(work.top));
    }
    (x, y)
}

unsafe fn register_popup_class(hinstance: HINSTANCE) {
    static mut REGISTERED: bool = false;
    if REGISTERED {
        return;
    }
    let wc = WNDCLASSW {
        lpfnWndProc: Some(popup_wnd_proc),
        hInstance: hinstance,
        lpszClassName: POPUP_CLASS,
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
        ..Default::default()
    };
    RegisterClassW(&wc);
    REGISTERED = true;
}

unsafe fn label_rect(hwnd: HWND) -> RECT {
    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);
    RECT { left: client.right - px(POPUP_PAD) - px(LABEL_W), right: client.right - px(POPUP_PAD), ..client }
}

unsafe extern "system" fn popup_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let theme = editor_window::theme();
            FillRect(hdc, &ps.rcPaint, editor_window::editor_bg_brush());
            if let Some(popup) = G_POPUP.as_ref() {
                let mut text: Vec<u16> = format!("{}%", popup.value).encode_utf16().collect();
                let mut r = label_rect(hwnd);
                let old = SelectObject(hdc, HGDIOBJ(popup.font.0));
                let _ = SetBkMode(hdc, TRANSPARENT);
                let _ = SetTextColor(hdc, theme.text);
                DrawTextW(hdc, &mut text, &mut r, DT_RIGHT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
                SelectObject(hdc, old);
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_CTLCOLORSTATIC => {
            // 트랙바 배경
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            let _ = SetBkColor(hdc, editor_window::theme().editor_bg);
            LRESULT(editor_window::editor_bg_brush().0 as isize)
        }
        WM_HSCROLL => {
            if let Some(popup) = G_POPUP.as_mut() {
                let pos = SendMessageW(popup.trackbar, TBM_GETPOS, WPARAM(0), LPARAM(0)).0;
                let value = pos.clamp(0, MAX_TRANSPARENCY as isize) as u8;
                if value != popup.value {
                    popup.value = value;
                    let on_change = popup.on_change;
                    let _ = InvalidateRect(hwnd, Some(&label_rect(hwnd)), false);
                    on_change(value, false);
                }
            }
            LRESULT(0)
        }
        WM_ACTIVATE => {
            // 바깥을 누르면 현재 값으로 확정하고 닫는다 (WM_ACTIVATE 안에서 바로 파괴하지 않는다)
            if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE {
                let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            if G_POPUP.as_ref().is_some_and(|p| p.hwnd == hwnd) {
                close_slider(true);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alpha_for_clamps_to_max_transparency() {
        assert_eq!(alpha_for(0), 255);
        assert_eq!(alpha_for(50), 127);
        assert_eq!(alpha_for(80), 51);
        assert_eq!(alpha_for(100), alpha_for(MAX_TRANSPARENCY));
    }
}

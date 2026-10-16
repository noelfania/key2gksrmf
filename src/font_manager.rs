// 내장 폰트 로드/적용/해제 모듈

use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        UI::WindowsAndMessaging::*,
    },
};

pub struct EmbeddedFont {
    mem_handle: Option<HANDLE>,
    hfont: HFONT,
}

impl EmbeddedFont {
    pub fn hfont(&self) -> HFONT {
        self.hfont
    }

    /// DPI에 맞춰 폰트를 다시 만든다. 이전 HFONT는 새 폰트를 컨트롤에 적용한 뒤 호출자가 지운다.
    pub unsafe fn recreate_for_dpi(&mut self, dpi: u32) -> Option<HFONT> {
        let hfont = create_ui_font(editor_font_height(dpi));
        if hfont == HFONT(0 as _) {
            return None;
        }
        Some(std::mem::replace(&mut self.hfont, hfont))
    }
}

// 96 DPI 기준 에디터 글자 높이(px)
const EDITOR_FONT_PX: i32 = 16;

fn editor_font_height(dpi: u32) -> i32 {
    -((EDITOR_FONT_PX * dpi as i32 + 48) / 96)
}

const FONT_DATA: &[u8] = include_bytes!("../assets/fonts/Pretendard-Regular.ttf");

unsafe fn create_ui_font(height: i32) -> HFONT {
    // 기본 UI 크기에서 과도하게 커지지 않도록 고정 높이 사용
    CreateFontW(
        height,
        0,
        0,
        0,
        FW_REGULAR.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32,
        CLEARTYPE_QUALITY.0 as u32,
        (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
        w!("Pretendard"),
    )
}

pub unsafe fn load_embedded_font(dpi: u32) -> Option<EmbeddedFont> {
    let mut count = 0u32;
    let mem_handle = AddFontMemResourceEx(
        FONT_DATA.as_ptr() as *const core::ffi::c_void,
        FONT_DATA.len() as u32,
        None,
        &mut count,
    );

    let hfont = create_ui_font(editor_font_height(dpi));
    if hfont == HFONT(0 as _) {
        if mem_handle != HANDLE(0 as _) {
            let _ = RemoveFontMemResourceEx(mem_handle);
        }
        return None;
    }

    Some(EmbeddedFont {
        mem_handle: if mem_handle == HANDLE(0 as _) {
            None
        } else {
            Some(mem_handle)
        },
        hfont,
    })
}

pub unsafe fn apply_font_to_control(hwnd: HWND, hfont: HFONT) {
    if hwnd == HWND(0 as _) || hfont == HFONT(0 as _) {
        return;
    }
    SendMessageW(hwnd, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
}

pub unsafe fn cleanup_embedded_font(font: EmbeddedFont) {
    if font.hfont != HFONT(0 as _) {
        let _ = DeleteObject(HGDIOBJ(font.hfont.0));
    }
    if let Some(handle) = font.mem_handle {
        let _ = RemoveFontMemResourceEx(handle);
    }
}

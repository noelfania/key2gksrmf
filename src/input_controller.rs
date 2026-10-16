// 에디터 키 입력 → 한글 조합·편집 처리 모듈
// Win32 EDIT에 직접 의존하지 않고 TextBuffer 트레이트로 동작해 테스트에서 재현할 수 있다.

use crate::hangul_engine::HangulComposer;

pub const VK_BACK: u32 = 0x08;
pub const VK_SHIFT: u32 = 0x10;
pub const VK_CONTROL: u32 = 0x11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Han,
    Eng,
}

/// 조합 대상 텍스트 저장소. 위치는 모두 UTF-16 단위 (EDIT의 EM_GETSEL과 동일).
pub trait TextBuffer {
    fn text_utf16(&self) -> Vec<u16>;
    fn selection(&self) -> (u32, u32);
    fn set_selection(&mut self, start: u32, end: u32);
    fn replace_selection(&mut self, text: &str);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct KeyEvent {
    pub vk: u32,
    pub shift: bool,
    pub ctrl: bool,
    /// WM_SYSKEYDOWN (Alt 누름) 여부
    pub alt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyResult {
    /// 컨트롤러가 처리함. 원래 EDIT 프로시저로 넘기지 않는다.
    Handled,
    /// 원래 EDIT 프로시저의 기본 동작에 맡긴다.
    PassThrough,
}

fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

pub fn map_physical_key_to_eng(vk: u32, shift: bool) -> Option<char> {
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
    vk == VK_SHIFT || vk == VK_CONTROL
}

fn is_word_boundary(ch: char) -> bool {
    ch.is_whitespace()
        || ch.is_ascii_punctuation()
        || matches!(ch, '·' | '…' | '—' | '–' | '「' | '」' | '『' | '』')
}

fn is_punctuation_boundary(ch: char) -> bool {
    is_word_boundary(ch) && !ch.is_whitespace()
}

pub fn find_ctrl_backspace_delete_from(chars: &[char], cursor: usize) -> usize {
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

#[derive(Debug)]
pub struct InputController {
    mode: InputMode,
    composer: HangulComposer,
    compose_start: Option<u32>,
    compose_len_utf16: u32,
    suppress_next_char: bool,
}

impl Default for InputController {
    fn default() -> Self {
        Self::new()
    }
}

impl InputController {
    pub fn new() -> Self {
        InputController {
            mode: InputMode::Han,
            composer: HangulComposer::new(),
            compose_start: None,
            compose_len_utf16: 0,
            suppress_next_char: false,
        }
    }

    pub fn mode(&self) -> InputMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: InputMode) {
        self.commit();
        self.mode = mode;
    }

    pub fn toggle_mode(&mut self) {
        let next = match self.mode {
            InputMode::Han => InputMode::Eng,
            InputMode::Eng => InputMode::Han,
        };
        self.set_mode(next);
    }

    /// 조합 중인 글자를 그대로 확정한다 (텍스트는 이미 버퍼에 있으므로 추적만 끊는다).
    pub fn commit(&mut self) {
        if self.composer.has_pending() {
            self.clear_tracking();
        }
    }

    pub fn on_mouse_down(&mut self) {
        self.commit();
    }

    /// 다음 WM_CHAR를 한 번 삼키도록 표시한다 (키다운에서 직접 처리한 키).
    pub fn suppress_next_char(&mut self) {
        self.suppress_next_char = true;
    }

    /// 보류 중인 WM_CHAR 억제 플래그를 소비한다.
    pub fn take_suppress(&mut self) -> bool {
        std::mem::take(&mut self.suppress_next_char)
    }

    /// WM_CHAR/WM_SYSCHAR 필터. false면 문자를 삼킨다.
    pub fn filter_char(&mut self, ch: u32) -> bool {
        if ch == 0x7F || self.suppress_next_char {
            self.suppress_next_char = false;
            return false;
        }
        true
    }

    fn clear_tracking(&mut self) {
        self.compose_start = None;
        self.compose_len_utf16 = 0;
        self.composer.clear();
    }

    fn render_composition<B: TextBuffer>(&mut self, buf: &mut B, text: &str) {
        let start = self.compose_start.unwrap_or(0);
        buf.set_selection(start, start + self.compose_len_utf16);
        buf.replace_selection(text);
        let next = start + utf16_len(text);
        buf.set_selection(next, next);
        self.compose_len_utf16 = utf16_len(text);
    }

    fn append_committed<B: TextBuffer>(&mut self, buf: &mut B, text: &str) {
        if text.is_empty() {
            return;
        }
        let start = self.compose_start.unwrap_or(0);
        buf.set_selection(start, start + self.compose_len_utf16);
        buf.replace_selection(text);
        let next = start + utf16_len(text);
        buf.set_selection(next, next);
        self.compose_start = Some(next);
        self.compose_len_utf16 = 0;
    }

    fn start_composition_at_caret<B: TextBuffer>(&mut self, buf: &mut B) {
        let (start, end) = buf.selection();
        if start != end {
            buf.set_selection(start, end);
            buf.replace_selection("");
            buf.set_selection(start, start);
        }
        self.compose_start = Some(start);
        self.compose_len_utf16 = 0;
        self.composer.clear();
    }

    fn backspace_composition<B: TextBuffer>(&mut self, buf: &mut B) {
        let text = self.composer.pop_key();
        self.render_composition(buf, &text);
        if text.is_empty() {
            self.clear_tracking();
        }
    }

    fn ctrl_backspace<B: TextBuffer>(&mut self, buf: &mut B) {
        if self.composer.has_pending() {
            self.backspace_composition(buf);
            return;
        }

        let (start, end) = buf.selection();
        if start != end {
            buf.replace_selection("");
            buf.set_selection(start, start);
            return;
        }

        let wide = buf.text_utf16();
        if wide.is_empty() || start == 0 {
            return;
        }
        // EM_GETSEL 위치는 UTF-16 단위이므로 문자 인덱스로 바꿔 계산한 뒤 다시 UTF-16으로 환산
        let cursor = (start as usize).min(wide.len());
        let chars: Vec<char> = String::from_utf16_lossy(&wide[..cursor]).chars().collect();
        let delete_from = find_ctrl_backspace_delete_from(&chars, chars.len());
        let from: u32 = chars[..delete_from].iter().map(|c| c.len_utf16() as u32).sum();
        buf.set_selection(from, cursor as u32);
        buf.replace_selection("");
        buf.set_selection(from, from);
    }

    /// WM_KEYDOWN/WM_SYSKEYDOWN 처리. F1·Alt+Z 같은 창 단위 단축키는 호출 전에 처리한다.
    pub fn handle_key<B: TextBuffer>(&mut self, buf: &mut B, ev: KeyEvent) -> KeyResult {
        let key = ev.vk;

        if is_modifier_key(key) {
            return KeyResult::PassThrough;
        }

        // Alt+문자는 메뉴/시스템 단축키 몫이다. 자모로 조합하지 않는다.
        if ev.alt {
            self.commit();
            return KeyResult::PassThrough;
        }

        if ev.ctrl && key == 'A' as u32 {
            self.commit();
            buf.set_selection(0, u32::MAX);
            self.suppress_next_char = true;
            return KeyResult::Handled;
        }

        if ev.ctrl && key == VK_BACK {
            self.ctrl_backspace(buf);
            self.suppress_next_char = true;
            return KeyResult::Handled;
        }

        if ev.ctrl {
            self.commit();
            return KeyResult::PassThrough;
        }

        if key == VK_BACK {
            if self.composer.has_pending() {
                self.backspace_composition(buf);
                self.suppress_next_char = true;
                return KeyResult::Handled;
            }
            return KeyResult::PassThrough;
        }

        if self.mode == InputMode::Eng {
            // 영 모드에서는 기본 영어 입력으로 통과
            self.commit();
            return KeyResult::PassThrough;
        }

        if let Some(eng_key) = map_physical_key_to_eng(key, ev.shift) {
            let (sel_start, sel_end) = buf.selection();
            if self.compose_start.is_none() || sel_start != sel_end {
                self.start_composition_at_caret(buf);
            }
            let step = self.composer.feed_key(eng_key);
            self.append_committed(buf, &step.committed);
            self.render_composition(buf, &step.composing);
            self.suppress_next_char = true;
            return KeyResult::Handled;
        }

        // 조합을 끊는 키가 오면 현재 조합을 확정하고 기본 동작으로 넘긴다.
        self.commit();
        KeyResult::PassThrough
    }
}

#[cfg(test)]
mod tests;

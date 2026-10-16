// 입력 컨트롤러 테스트
// tests/editor_cases.txt 의 시나리오를 메모리 버퍼와 실제 Win32 EDIT 양쪽에서 실행한다.

use super::*;
use crate::editor_window::{self, EditBuffer};
use windows::Win32::{
    Foundation::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};
use windows::core::{w, PCWSTR};

const CASES: &str = include_str!("../../tests/editor_cases.txt");

const VK_TAB: u32 = 0x09;
const VK_RETURN: u32 = 0x0D;
const VK_SPACE: u32 = 0x20;
const VK_END: u32 = 0x23;
const VK_HOME: u32 = 0x24;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;
const VK_DELETE: u32 = 0x2E;
// 문자 키가 아닌 기호 키 대표값 (VK_OEM_2: / ?)
const VK_OEM_OTHER: u32 = 0xBF;

// ---------- 케이스 파일 파서 ----------

#[derive(Debug)]
struct Case {
    name: String,
    line: usize,
    start: String,
    keys: String,
    expect: String,
}

fn field_value(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.strip_prefix(':')?;
    // 콜론 뒤 공백 한 칸만 구분자로 보고 나머지 공백은 값으로 유지한다
    Some(rest.strip_prefix(' ').unwrap_or(rest).to_string())
}

fn parse_cases(src: &str) -> Vec<Case> {
    let mut cases: Vec<Case> = Vec::new();
    for (idx, raw) in src.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            cases.push(Case {
                name: trimmed[1..trimmed.len() - 1].to_string(),
                line: idx + 1,
                start: "|".to_string(),
                keys: String::new(),
                expect: String::new(),
            });
            continue;
        }
        let case = cases
            .last_mut()
            .unwrap_or_else(|| panic!("editor_cases.txt:{}: [케이스 이름] 앞에 필드가 있음", idx + 1));
        if let Some(v) = field_value(line, "start") {
            case.start = v;
        } else if let Some(v) = field_value(line, "keys") {
            case.keys = v;
        } else if let Some(v) = field_value(line, "expect") {
            case.expect = v;
        } else {
            panic!("editor_cases.txt:{}: 알 수 없는 줄: {line}", idx + 1);
        }
    }
    cases
}

#[derive(Debug, Clone)]
enum Token {
    Key(KeyEvent, Option<u16>),
    F1,
    Click(u32),
    Paste(String),
}

fn key(vk: u32, ch: Option<u16>) -> Token {
    Token::Key(KeyEvent { vk, ..Default::default() }, ch)
}

fn char_token(c: char) -> Token {
    match c {
        'a'..='z' => key(c.to_ascii_uppercase() as u32, Some(c as u16)),
        'A'..='Z' => Token::Key(
            KeyEvent { vk: c as u32, shift: true, ..Default::default() },
            Some(c as u16),
        ),
        '0'..='9' => key(c as u32, Some(c as u16)),
        ' ' => key(VK_SPACE, Some(0x20)),
        c if c.is_ascii_punctuation() => key(VK_OEM_OTHER, Some(c as u16)),
        _ => panic!("keys에 직접 쓸 수 없는 문자: {c:?} ({{Paste:...}}를 사용)"),
    }
}

fn named_token(name: &str) -> Token {
    if let Some(n) = name.strip_prefix("Click:") {
        return Token::Click(n.parse().expect("Click:N 의 N은 UTF-16 위치"));
    }
    if let Some(text) = name.strip_prefix("Paste:") {
        return Token::Paste(text.to_string());
    }
    if let Some(rest) = name.strip_prefix("Ctrl+") {
        let (vk, ch) = if rest == "BS" {
            (VK_BACK, 0x7F)
        } else {
            let c = rest.chars().next().expect("Ctrl+ 뒤 키 이름").to_ascii_uppercase();
            (c as u32, (c as u16) - 0x40)
        };
        return Token::Key(KeyEvent { vk, ctrl: true, ..Default::default() }, Some(ch));
    }
    if let Some(rest) = name.strip_prefix("Alt+") {
        let c = rest.chars().next().expect("Alt+ 뒤 키 이름");
        let vk = c.to_ascii_uppercase() as u32;
        return Token::Key(
            KeyEvent { vk, alt: true, ..Default::default() },
            Some(c.to_ascii_lowercase() as u16),
        );
    }
    match name {
        "Space" => key(VK_SPACE, Some(0x20)),
        "Enter" => key(VK_RETURN, Some(0x0D)),
        "Tab" => key(VK_TAB, Some(0x09)),
        "BS" => key(VK_BACK, Some(0x08)),
        "Del" => key(VK_DELETE, None),
        "Left" => key(VK_LEFT, None),
        "Right" => key(VK_RIGHT, None),
        "Up" => key(VK_UP, None),
        "Down" => key(VK_DOWN, None),
        "Home" => key(VK_HOME, None),
        "End" => key(VK_END, None),
        "F1" => Token::F1,
        _ => panic!("알 수 없는 키 토큰: {{{name}}}"),
    }
}

fn parse_keys(src: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') {
                chars.next();
                tokens.push(char_token('{'));
                continue;
            }
            let name: String = chars.by_ref().take_while(|&c| c != '}').collect();
            tokens.push(named_token(&name));
        } else {
            tokens.push(char_token(c));
        }
    }
    tokens
}

/// 마크업(`|` 캐럿, `[..]` 선택, `\` 이스케이프, 줄바꿈은 `\n`)을 EDIT 텍스트(CRLF)와 선택 영역으로 변환
fn parse_markup(src: &str) -> (Vec<u16>, (u32, u32)) {
    let mut text: Vec<u16> = Vec::new();
    let mut caret: Option<u32> = None;
    let mut sel_start: Option<u32> = None;
    let mut sel_end: Option<u32> = None;
    let mut chars = src.chars();
    while let Some(c) = chars.next() {
        let pos = text.len() as u32;
        match c {
            '|' => caret = Some(pos),
            '[' => sel_start = Some(pos),
            ']' => sel_end = Some(pos),
            '\\' => match chars.next() {
                Some('n') => text.extend_from_slice(&[0x0D, 0x0A]),
                Some(e) => {
                    let mut b = [0u16; 2];
                    text.extend_from_slice(e.encode_utf16(&mut b));
                }
                None => {}
            },
            _ => {
                let mut b = [0u16; 2];
                text.extend_from_slice(c.encode_utf16(&mut b));
            }
        }
    }
    let end = text.len() as u32;
    let sel = match (sel_start, sel_end, caret) {
        (Some(s), Some(e), _) => (s, e),
        (_, _, Some(c)) => (c, c),
        _ => (end, end),
    };
    (text, sel)
}

fn render_markup(text: &[u16], sel: (u32, u32)) -> String {
    let (s, e) = (sel.0 as usize, sel.1 as usize);
    let mut out = String::new();
    let mut i = 0;
    let push_marks = |out: &mut String, i: usize| {
        if s == e {
            if i == s {
                out.push('|');
            }
        } else {
            if i == s {
                out.push('[');
            }
            if i == e {
                out.push(']');
            }
        }
    };
    while i < text.len() {
        push_marks(&mut out, i);
        if text[i] == 0x0D && text.get(i + 1) == Some(&0x0A) {
            out.push_str("\\n");
            i += 2;
            continue;
        }
        let unit_len = if (0xD800..0xDC00).contains(&text[i]) && i + 1 < text.len() { 2 } else { 1 };
        let piece = String::from_utf16_lossy(&text[i..i + unit_len]);
        for c in piece.chars() {
            if matches!(c, '|' | '[' | ']' | '\\') {
                out.push('\\');
            }
            out.push(c);
        }
        i += unit_len;
    }
    push_marks(&mut out, text.len());
    out
}

// ---------- 시뮬레이션 버퍼 ----------

/// 원래 EDIT 프로시저가 하던 기본 동작(PassThrough 이후)을 재현하는 버퍼
trait SimBuffer: TextBuffer {
    fn load(&mut self, text: &[u16], sel: (u32, u32));
    fn key_default(&mut self, vk: u32);
    fn char_default(&mut self, ch: u16);
    fn paste(&mut self, text: &str) {
        self.replace_selection(text);
    }
}

#[derive(Default)]
struct MemBuffer {
    text: Vec<u16>,
    sel: (u32, u32),
}

impl MemBuffer {
    fn len(&self) -> u32 {
        self.text.len() as u32
    }

    // 캐럿 왼쪽 한 단위(CRLF·서로게이트 쌍은 한 단위)의 시작 위치
    fn prev_unit(&self, pos: u32) -> u32 {
        let p = pos as usize;
        if p == 0 {
            return 0;
        }
        if p >= 2 && self.text[p - 2] == 0x0D && self.text[p - 1] == 0x0A {
            return pos - 2;
        }
        if p >= 2 && (0xDC00..0xE000).contains(&self.text[p - 1]) && (0xD800..0xDC00).contains(&self.text[p - 2]) {
            return pos - 2;
        }
        pos - 1
    }

    fn next_unit(&self, pos: u32) -> u32 {
        let p = pos as usize;
        if p >= self.text.len() {
            return self.len();
        }
        if self.text[p] == 0x0D && self.text.get(p + 1) == Some(&0x0A) {
            return pos + 2;
        }
        if (0xD800..0xDC00).contains(&self.text[p]) && p + 1 < self.text.len() {
            return pos + 2;
        }
        pos + 1
    }

    fn line_start(&self, pos: u32) -> u32 {
        let mut p = pos as usize;
        while p > 0 && self.text[p - 1] != 0x0A {
            p -= 1;
        }
        p as u32
    }

    fn line_end(&self, pos: u32) -> u32 {
        let mut p = pos as usize;
        while p < self.text.len() && self.text[p] != 0x0D {
            p += 1;
        }
        p as u32
    }
}

impl TextBuffer for MemBuffer {
    fn text_utf16(&self) -> Vec<u16> {
        self.text.clone()
    }

    fn selection(&self) -> (u32, u32) {
        self.sel
    }

    fn set_selection(&mut self, start: u32, end: u32) {
        let len = self.len();
        self.sel = (start.min(len), end.min(len));
    }

    fn replace_selection(&mut self, text: &str) {
        let (s, e) = (self.sel.0.min(self.sel.1) as usize, self.sel.0.max(self.sel.1) as usize);
        let wide: Vec<u16> = text.encode_utf16().collect();
        let caret = (s + wide.len()) as u32;
        self.text.splice(s..e, wide);
        self.sel = (caret, caret);
    }
}

impl SimBuffer for MemBuffer {
    fn load(&mut self, text: &[u16], sel: (u32, u32)) {
        self.text = text.to_vec();
        self.set_selection(sel.0, sel.1);
    }

    fn key_default(&mut self, vk: u32) {
        let (s, e) = self.sel;
        let caret = e;
        match vk {
            VK_LEFT => {
                let p = if s != e { s.min(e) } else { self.prev_unit(caret) };
                self.sel = (p, p);
            }
            VK_RIGHT => {
                let p = if s != e { s.max(e) } else { self.next_unit(caret) };
                self.sel = (p, p);
            }
            VK_HOME => {
                let p = self.line_start(caret);
                self.sel = (p, p);
            }
            VK_END => {
                let p = self.line_end(caret);
                self.sel = (p, p);
            }
            VK_DELETE => {
                if s == e {
                    let next = self.next_unit(caret);
                    self.sel = (caret, next);
                }
                self.replace_selection("");
            }
            _ => {}
        }
    }

    fn char_default(&mut self, ch: u16) {
        match ch {
            0x08 => {
                let (s, e) = self.sel;
                if s == e {
                    let prev = self.prev_unit(s);
                    self.sel = (prev, s);
                }
                self.replace_selection("");
            }
            0x0D => self.replace_selection("\r\n"),
            0x09 => self.replace_selection("\t"),
            c if c >= 0x20 => self.replace_selection(&String::from_utf16_lossy(&[c])),
            _ => {}
        }
    }
}

/// 숨김 창 위의 실제 EDIT 컨트롤 (운영 코드와 같은 create_edit_control 사용)
struct RealEdit {
    parent: HWND,
    edit: EditBuffer,
}

impl RealEdit {
    fn new() -> Self {
        unsafe {
            let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
            let parent = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                PCWSTR::null(),
                WS_POPUP,
                0,
                0,
                600,
                400,
                None,
                None,
                hinstance,
                None,
            )
            .expect("테스트용 부모 창 생성 실패");
            // 줄바꿈 없음: Home/End가 논리 행 기준으로 동작하도록
            let edit = editor_window::create_edit_control(parent, hinstance, false, 600, 400);
            assert!(edit != HWND(0 as _), "테스트용 EDIT 생성 실패");
            RealEdit { parent, edit: EditBuffer(edit) }
        }
    }
}

impl Drop for RealEdit {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.parent);
        }
    }
}

impl TextBuffer for RealEdit {
    fn text_utf16(&self) -> Vec<u16> {
        self.edit.text_utf16()
    }
    fn selection(&self) -> (u32, u32) {
        self.edit.selection()
    }
    fn set_selection(&mut self, start: u32, end: u32) {
        self.edit.set_selection(start, end)
    }
    fn replace_selection(&mut self, text: &str) {
        self.edit.replace_selection(text)
    }
}

impl SimBuffer for RealEdit {
    fn load(&mut self, text: &[u16], sel: (u32, u32)) {
        let mut wide = text.to_vec();
        wide.push(0);
        unsafe {
            let _ = SetWindowTextW(self.edit.0, PCWSTR(wide.as_ptr()));
        }
        self.set_selection(sel.0, sel.1);
    }

    fn key_default(&mut self, vk: u32) {
        unsafe {
            SendMessageW(self.edit.0, WM_KEYDOWN, WPARAM(vk as usize), LPARAM(0));
        }
    }

    fn char_default(&mut self, ch: u16) {
        unsafe {
            SendMessageW(self.edit.0, WM_CHAR, WPARAM(ch as usize), LPARAM(0));
        }
    }
}

// ---------- 실행기 ----------

/// 운영 코드의 메시지 흐름(edit_proc → 원래 EDIT 프로시저)을 그대로 따라 키를 입력한다.
fn run_keys<S: SimBuffer>(buf: &mut S, ctrl: &mut InputController, tokens: &[Token]) {
    for tok in tokens {
        match tok {
            // F1은 메시지 루프에서 처리된다 (toggle_input_mode_and_focus_editor)
            Token::F1 => ctrl.toggle_mode(),
            Token::Click(pos) => {
                ctrl.on_mouse_down();
                buf.set_selection(*pos, *pos);
            }
            Token::Paste(text) => {
                let ev = KeyEvent { vk: 'V' as u32, ctrl: true, ..Default::default() };
                if ctrl.handle_key(buf, ev) == KeyResult::PassThrough {
                    // Ctrl+V의 WM_CHAR(0x16)는 필터만 거치고 붙여넣기는 직접 수행
                    ctrl.filter_char(0x16);
                    buf.paste(text);
                }
            }
            Token::Key(ev, ch) => {
                if ctrl.handle_key(buf, *ev) == KeyResult::PassThrough && !ev.alt {
                    buf.key_default(ev.vk);
                }
                if let Some(c) = ch {
                    // Alt 조합은 WM_SYSCHAR → EDIT는 문자를 넣지 않는다
                    if ctrl.filter_char(*c as u32) && !ev.alt {
                        buf.char_default(*c);
                    }
                }
            }
        }
    }
}

fn run_case<S: SimBuffer>(buf: &mut S, case: &Case) -> String {
    let (text, sel) = parse_markup(&case.start);
    buf.load(&text, sel);
    let mut ctrl = InputController::new();
    run_keys(buf, &mut ctrl, &parse_keys(&case.keys));
    let wide = buf.text_utf16();
    render_markup(&wide, buf.selection())
}

fn run_all<S: SimBuffer>(label: &str, mut make: impl FnMut() -> S) {
    let cases = parse_cases(CASES);
    assert!(!cases.is_empty(), "editor_cases.txt에 케이스가 없음");
    let mut failures = Vec::new();
    for case in &cases {
        let mut buf = make();
        let actual = run_case(&mut buf, case);
        if actual != case.expect {
            failures.push(format!(
                "  [{}] (editor_cases.txt:{})\n    start : {}\n    keys  : {}\n    expect: {}\n    actual: {}",
                case.name, case.line, case.start, case.keys, case.expect, actual
            ));
        }
    }
    let passed = cases.len() - failures.len();
    println!("{label}: {passed} passed / {} failed", failures.len());
    if !failures.is_empty() {
        panic!(
            "{label}: {} / {} 케이스 실패\n{}",
            failures.len(),
            cases.len(),
            failures.join("\n")
        );
    }
}

#[test]
fn editor_cases_mem_buffer() {
    run_all("MemBuffer", MemBuffer::default);
}

#[test]
fn editor_cases_real_edit() {
    run_all("RealEdit", RealEdit::new);
}

/// EDIT 기본 입력 한도(32,767자)를 넘는 문서에서도 계속 입력할 수 있어야 한다.
#[test]
fn real_edit_accepts_text_beyond_default_limit() {
    let mut buf = RealEdit::new();
    let long: Vec<u16> = vec!['a' as u16; 40_000];
    buf.load(&long, (40_000, 40_000));
    let mut ctrl = InputController::new();
    run_keys(&mut buf, &mut ctrl, &parse_keys("rk{F1}b"));
    let text = buf.text_utf16();
    assert_eq!(text.len(), 40_002, "긴 문서 끝에 입력이 막힘");
    assert_eq!(String::from_utf16_lossy(&text[40_000..]), "가b");
}

#[test]
fn markup_roundtrip() {
    for src in ["|", "가[나다]라", "a\\nb|", "\\|x|", "😀 |"] {
        let (text, sel) = parse_markup(src);
        assert_eq!(render_markup(&text, sel), src);
    }
}

// ---------- 단위 테스트 ----------

#[test]
fn test_map_physical_key_to_eng() {
    assert_eq!(map_physical_key_to_eng('R' as u32, false), Some('r'));
    assert_eq!(map_physical_key_to_eng('R' as u32, true), Some('R'));
    assert_eq!(map_physical_key_to_eng('G' as u32, false), Some('g'));
    assert_eq!(map_physical_key_to_eng('G' as u32, true), Some('g'));
    assert_eq!(map_physical_key_to_eng(VK_BACK, false), None);
    assert_eq!(map_physical_key_to_eng('T' as u32, true), Some('T'));
    assert!(is_modifier_key(VK_SHIFT));
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

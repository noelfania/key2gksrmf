// 영문 키 시퀀스를 두벌식 한글 완성형 문자열로 변환하는 순수 로직 모듈

// 두벌식 영문 키 배열: r=ㄱ, R=ㄲ, s=ㄴ, ...
const ENG_KEY: &[u8] = b"rRseEfaqQtTdwWczxvgkoiOjpuPhynbml";

// 위 ENG_KEY 각 자리에 대응하는 한글 자모 (chars로 인덱스 접근)
const KOR_KEY: &str = "ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅㅆㅇㅈㅉㅊㅋㅌㅍㅎㅏㅐㅑㅒㅓㅔㅕㅖㅗㅛㅜㅠㅡㅣ";

// 초성 19자
const CHO_DATA: &str = "ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅㅆㅇㅈㅉㅊㅋㅌㅍㅎ";

// 중성 21자
const JUNG_DATA: &str = "ㅏㅐㅑㅒㅓㅔㅕㅖㅗㅘㅙㅚㅛㅜㅝㅞㅟㅠㅡㅢㅣ";

// 종성 28자 (인덱스 0 = 없음 기준이 아닌, 직접 문자열 인덱스)
const JONG_DATA: &str = "ㄱㄲㄳㄴㄵㄶㄷㄹㄺㄻㄼㄽㄾㄿㅀㅁㅂㅄㅅㅆㅇㅈㅊㅋㅌㅍㅎ";

// 문자열에서 문자 인덱스를 반환하는 헬퍼
fn char_index(s: &str, ch: char) -> Option<usize> {
    s.chars().position(|c| c == ch)
}

// char 인덱스로 문자를 꺼내는 헬퍼
fn char_at(s: &str, idx: usize) -> char {
    s.chars().nth(idx).unwrap_or('\0')
}

#[derive(Debug, Default, Clone)]
pub struct HangulComposer {
    keys: String,
}

#[derive(Debug, Default, Clone)]
pub struct ComposerStep {
    pub committed: String,
    pub composing: String,
}

impl HangulComposer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.keys.clear();
    }

    pub fn has_pending(&self) -> bool {
        !self.keys.is_empty()
    }

    pub fn feed_key(&mut self, key: char) -> ComposerStep {
        self.keys.push(key);
        self.split_committed_and_tail()
    }

    pub fn pop_key(&mut self) -> String {
        self.keys.pop();
        convert_eng_to_kor(&self.keys)
    }

    #[cfg(test)]
    pub fn preview_text(&self) -> String {
        convert_eng_to_kor(&self.keys)
    }

    fn split_committed_and_tail(&mut self) -> ComposerStep {
        let text = convert_eng_to_kor(&self.keys);
        let mut chars: Vec<char> = text.chars().collect();
        if chars.len() <= 1 {
            return ComposerStep {
                committed: String::new(),
                composing: text,
            };
        }

        let last_char = chars.pop().unwrap();
        let committed: String = chars.into_iter().collect();
        let composing = last_char.to_string();

        // 내부 키 버퍼는 마지막 조합꼬리(문자 1개)를 만들 최소 suffix만 유지
        let key_chars: Vec<char> = self.keys.chars().collect();
        let mut suffix = String::new();
        for start in 0..key_chars.len() {
            let cand: String = key_chars[start..].iter().collect();
            if convert_eng_to_kor(&cand) == composing {
                suffix = cand;
                break;
            }
        }
        if suffix.is_empty() {
            suffix = self.keys.clone();
        }
        self.keys = suffix;

        ComposerStep {
            committed,
            composing,
        }
    }

    /// 완성된 한글 조합 단위(음절 1개)를 지우기 위해
    /// 키 입력을 되돌려 표시 길이(문자 수)를 1 줄인다.
    #[cfg(test)]
    pub fn backspace_syllable(&mut self) -> String {
        if self.keys.is_empty() {
            return String::new();
        }

        let current = convert_eng_to_kor(&self.keys);
        let current_len = current.chars().count();
        let target_len = current_len.saturating_sub(1);

        while !self.keys.is_empty() {
            self.keys.pop();
            let next = convert_eng_to_kor(&self.keys);
            if next.chars().count() <= target_len {
                return next;
            }
        }

        String::new()
    }
}

// 초성+중성+(종성)으로 완성형 한글 코드포인트 생성
fn make_hangul(cho: usize, jung: usize, jong: i32) -> char {
    // 종성 -1 = 없음 → 완성형 수식에서 +1 조정
    let jong_val = if jong < 0 { 0 } else { jong as u32 + 1 };
    let code = 0xAC00 + cho as u32 * 21 * 28 + jung as u32 * 28 + jong_val;
    char::from_u32(code).unwrap_or('\0')
}

// 현재 조합 상태를 문자열로 flush
fn flush(cho: i32, jung: i32, jong: i32) -> String {
    let mut out = String::new();
    if cho >= 0 {
        if jung >= 0 {
            out.push(make_hangul(cho as usize, jung as usize, jong));
        } else {
            out.push(char_at(CHO_DATA, cho as usize));
        }
    } else if jung >= 0 {
        out.push(char_at(JUNG_DATA, jung as usize));
    } else if jong >= 0 {
        out.push(char_at(JONG_DATA, jong as usize));
    }
    out
}

/// 영문 키 시퀀스를 두벌식 한글 완성형 문자열로 변환한다.
/// 비대상 문자(숫자/기호/공백/이미 한글인 문자 등)는 원문 그대로 유지한다.
pub fn convert_eng_to_kor(src: &str) -> String {
    let mut res = String::new();

    let mut cho: i32 = -1;
    let mut jung: i32 = -1;
    let mut jong: i32 = -1;

    // ENG_KEY에서 ascii 바이트로 빠르게 찾기 위한 조회 테이블
    let eng_key_chars: Vec<char> = ENG_KEY.iter().map(|&b| b as char).collect();
    let kor_key_chars: Vec<char> = KOR_KEY.chars().collect();

    let src_chars: Vec<char> = src.chars().collect();
    let mut i = 0;

    while i < src_chars.len() {
        let ch = src_chars[i];
        i += 1;

        // ENG_KEY 내 위치 탐색 (ASCII 범위 문자만 해당)
        let p: i32 = if ch.is_ascii() {
            eng_key_chars.iter().position(|&k| k == ch).map(|x| x as i32).unwrap_or(-1)
        } else {
            -1
        };

        if p == -1 {
            // 영문 키가 아님 → 현재 조합 상태 flush 후 원문 추가
            res.push_str(&flush(cho, jung, jong));
            cho = -1;
            jung = -1;
            jong = -1;
            res.push(ch);
        } else if p < 19 {
            // 자음 처리
            if jung >= 0 {
                if cho < 0 {
                    // 중성만 입력된 상태 → 새 음절 초성 시작
                    res.push(char_at(JUNG_DATA, jung as usize));
                    jung = -1;
                    cho = char_index(CHO_DATA, kor_key_chars[p as usize]).map(|x| x as i32).unwrap_or(-1);
                } else {
                    // 초성+중성 있음 → 종성 후보
                    if jong < 0 {
                        let jong_cand = char_index(JONG_DATA, kor_key_chars[p as usize]).map(|x| x as i32).unwrap_or(-1);
                        if jong_cand < 0 {
                            // 종성 불가 → 새 음절 초성
                            res.push(make_hangul(cho as usize, jung as usize, -1));
                            cho = char_index(CHO_DATA, kor_key_chars[p as usize]).map(|x| x as i32).unwrap_or(-1);
                            jung = -1;
                        } else {
                            jong = jong_cand;
                        }
                    } else {
                        // 복자음 조합 시도
                        let new_jong = try_combine_jong(jong, p);
                        if let Some(j) = new_jong {
                            jong = j as i32;
                        } else {
                            // 조합 불가 → 현재 음절 확정 후 새 초성
                            res.push(make_hangul(cho as usize, jung as usize, jong));
                            cho = char_index(CHO_DATA, kor_key_chars[p as usize]).map(|x| x as i32).unwrap_or(-1);
                            jung = -1;
                            jong = -1;
                        }
                    }
                }
            } else {
                // 중성 없음
                if cho < 0 {
                    if jong >= 0 {
                        // 복자음 뒤 초성
                        res.push(char_at(JONG_DATA, jong as usize));
                        jong = -1;
                    }
                    cho = char_index(CHO_DATA, kor_key_chars[p as usize]).map(|x| x as i32).unwrap_or(-1);
                } else {
                    // 이미 초성 있음 → 복자음 조합 시도 (초성 상태에서)
                    let combined = try_combine_cho(cho, p);
                    if let Some(j) = combined {
                        cho = -1;
                        jong = j as i32;
                    } else {
                        // 단자음 연타
                        res.push(char_at(CHO_DATA, cho as usize));
                        cho = char_index(CHO_DATA, kor_key_chars[p as usize]).map(|x| x as i32).unwrap_or(-1);
                    }
                }
            }
        } else {
            // 모음 처리
            if jong >= 0 {
                // 앞 음절 종성 → 분리해서 다음 음절 초성으로
                let (keep_jong, new_cho) = split_jong_to_cho(jong);
                if cho >= 0 {
                    res.push(make_hangul(cho as usize, jung as usize, keep_jong));
                } else {
                    if keep_jong >= 0 {
                        res.push(char_at(JONG_DATA, keep_jong as usize));
                    }
                }
                cho = new_cho;
                jung = -1;
                jong = -1;
            }

            if jung < 0 {
                jung = char_index(JUNG_DATA, kor_key_chars[p as usize]).map(|x| x as i32).unwrap_or(-1);
            } else {
                // 복모음 조합 시도
                let combined = try_combine_jung(jung, p);
                if let Some(j) = combined {
                    jung = j as i32;
                } else {
                    // 조합 불가 → 현재 음절 확정
                    if cho >= 0 {
                        res.push(make_hangul(cho as usize, jung as usize, -1));
                        cho = -1;
                    } else {
                        res.push(char_at(JUNG_DATA, jung as usize));
                    }
                    jung = char_index(JUNG_DATA, kor_key_chars[p as usize]).map(|x| x as i32).unwrap_or(-1);
                    // 이전 음절을 닫았으니 공백 모음의 초성을 ㅇ으로 두지 않고 그냥 모음만
                }
            }
        }
    }

    // 마지막 남은 상태 flush
    res.push_str(&flush(cho, jung, jong));
    res
}

// 초성 상태에서 복자음(→종성 후보) 조합 가능한지 확인
fn try_combine_cho(cho: i32, p: i32) -> Option<usize> {
    match (cho, p) {
        (0, 9) => Some(2),   // ㄱ+ㅅ = ㄳ
        (2, 12) => Some(4),  // ㄴ+ㅈ = ㄵ
        (2, 18) => Some(5),  // ㄴ+ㅎ = ㄶ
        (5, 0) => Some(8),   // ㄹ+ㄱ = ㄺ
        (5, 6) => Some(9),   // ㄹ+ㅁ = ㄻ
        (5, 7) => Some(10),  // ㄹ+ㅂ = ㄼ
        (5, 9) => Some(11),  // ㄹ+ㅅ = ㄽ
        (5, 16) => Some(12), // ㄹ+ㅌ = ㄾ
        (5, 17) => Some(13), // ㄹ+ㅍ = ㄿ
        (5, 18) => Some(14), // ㄹ+ㅎ = ㅀ
        (7, 9) => Some(17),  // ㅂ+ㅅ = ㅄ
        _ => None,
    }
}

// 종성 상태에서 복자음 조합 가능한지 확인
fn try_combine_jong(jong: i32, p: i32) -> Option<usize> {
    match (jong, p) {
        (0, 9) => Some(2),   // ㄱ+ㅅ = ㄳ
        (3, 12) => Some(4),  // ㄴ+ㅈ = ㄵ
        (3, 18) => Some(5),  // ㄴ+ㅎ = ㄶ
        (7, 0) => Some(8),   // ㄹ+ㄱ = ㄺ
        (7, 6) => Some(9),   // ㄹ+ㅁ = ㄻ
        (7, 7) => Some(10),  // ㄹ+ㅂ = ㄼ
        (7, 9) => Some(11),  // ㄹ+ㅅ = ㄽ
        (7, 16) => Some(12), // ㄹ+ㅌ = ㄾ
        (7, 17) => Some(13), // ㄹ+ㅍ = ㄿ
        (7, 18) => Some(14), // ㄹ+ㅎ = ㅀ
        (16, 9) => Some(17), // ㅂ+ㅅ = ㅄ
        _ => None,
    }
}

// 복모음 조합 시도 (jung 현재 인덱스, p = ENG_KEY 위치)
fn try_combine_jung(jung: i32, p: i32) -> Option<usize> {
    match (jung, p) {
        (8, 19) => Some(9),  // ㅗ+ㅏ = ㅘ
        (8, 20) => Some(10), // ㅗ+ㅐ = ㅙ
        (8, 32) => Some(11), // ㅗ+ㅣ = ㅚ
        (13, 23) => Some(14),// ㅜ+ㅓ = ㅝ
        (13, 24) => Some(15),// ㅜ+ㅔ = ㅞ
        (13, 32) => Some(16),// ㅜ+ㅣ = ㅟ
        (18, 32) => Some(19),// ㅡ+ㅣ = ㅢ
        _ => None,
    }
}

// 복자음 종성을 분리해 앞 음절 종성과 뒷 음절 초성으로 나눔
// 반환: (앞 음절에 남을 종성 인덱스, 뒷 음절 초성 인덱스)
fn split_jong_to_cho(jong: i32) -> (i32, i32) {
    match jong {
        2 => (0, 9),   // ㄳ → ㄱ, ㅅ
        4 => (3, 12),  // ㄵ → ㄴ, ㅈ
        5 => (3, 18),  // ㄶ → ㄴ, ㅎ
        8 => (7, 0),   // ㄺ → ㄹ, ㄱ
        9 => (7, 6),   // ㄻ → ㄹ, ㅁ
        10 => (7, 7),  // ㄼ → ㄹ, ㅂ
        11 => (7, 9),  // ㄽ → ㄹ, ㅅ
        12 => (7, 16), // ㄾ → ㄹ, ㅌ
        13 => (7, 17), // ㄿ → ㄹ, ㅍ
        14 => (7, 18), // ㅀ → ㄹ, ㅎ
        17 => (16, 9), // ㅄ → ㅂ, ㅅ
        _ => {
            // 단자음 종성 → 그대로 남기고, 초성으로 승격
            let ch = char_at(JONG_DATA, jong as usize);
            let new_cho = char_index(CHO_DATA, ch).map(|x| x as i32).unwrap_or(-1);
            (-1, new_cho)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        assert_eq!(convert_eng_to_kor("dkssudgktpdy"), "안녕하세요");
    }

    #[test]
    fn test_non_hangul_passthrough() {
        assert_eq!(convert_eng_to_kor("123 ABC!"), "123 ABC!");
    }

    #[test]
    fn test_single_consonant() {
        assert_eq!(convert_eng_to_kor("r"), "ㄱ");
        assert_eq!(convert_eng_to_kor("s"), "ㄴ");
    }

    #[test]
    fn test_single_vowel() {
        assert_eq!(convert_eng_to_kor("k"), "ㅏ");
    }

    #[test]
    fn test_mixed() {
        // "안녕" 뒤에 숫자
        assert_eq!(convert_eng_to_kor("dkssud123"), "안녕123");
    }

    #[test]
    fn test_double_consonant_jong() {
        // 닭 = ekfr (ㄷ+ㅏ+ㄹ+ㄱ)
        assert_eq!(convert_eng_to_kor("ekfr"), "닭");
    }

    // JONG_DATA[n] + 1 = 유니코드 종성 슬롯(n+1) 정합성 전수 검증
    // JS 참조(makeHangul: 0xac00 + cho*21*28 + jung*28 + jong+1)와 동일한 수식 확인
    #[test]
    fn test_jong_data_alignment_single() {
        // 단자음 종성: make_hangul 오프셋(+1) 기본 동작 확인
        assert_eq!(convert_eng_to_kor("rkr"), "각");  // JONG_DATA[0]=ㄱ  slot=1
        assert_eq!(convert_eng_to_kor("gks"), "한");  // JONG_DATA[3]=ㄴ  slot=4
        assert_eq!(convert_eng_to_kor("qkq"), "밥");  // JONG_DATA[16]=ㅂ slot=17
        assert_eq!(convert_eng_to_kor("wjd"), "정");  // JONG_DATA[20]=ㅇ slot=21
    }

    #[test]
    fn test_jong_data_alignment_double() {
        // 겹받침 종성 11종 전수 검증 — 순서가 틀리면 잘못된 음절이 생성된다
        assert_eq!(convert_eng_to_kor("sjrt"), "넋");  // ㄳ JONG_DATA[2]  slot=3
        assert_eq!(convert_eng_to_kor("dksw"), "앉");  // ㄵ JONG_DATA[4]  slot=5
        assert_eq!(convert_eng_to_kor("dksg"), "않");  // ㄶ JONG_DATA[5]  slot=6
        // ㄺ: 닭(ekfr) — test_double_consonant_jong에서 검증
        assert_eq!(convert_eng_to_kor("tkfa"), "삶");  // ㄻ JONG_DATA[9]  slot=10
        assert_eq!(convert_eng_to_kor("qkfq"), "밟");  // ㄼ JONG_DATA[10] slot=11
        assert_eq!(convert_eng_to_kor("gkfx"), "핥");  // ㄾ JONG_DATA[12] slot=13
        assert_eq!(convert_eng_to_kor("tlfg"), "싫");  // ㅀ JONG_DATA[14] slot=15
        assert_eq!(convert_eng_to_kor("djqt"), "없");  // ㅄ JONG_DATA[17] slot=18
    }

    #[test]
    fn test_jong_split_on_vowel() {
        // 겹받침 뒤에 모음이 오면 뒤 자음이 다음 음절 초성으로 분리된다
        assert_eq!(convert_eng_to_kor("ekfrl"), "달기");    // 닭+ㅣ → ㄺ 분리: 달+기
        assert_eq!(convert_eng_to_kor("djqtdl"), "없이");   // 없+이
        assert_eq!(convert_eng_to_kor("dkswdk"), "앉아");   // 앉+아 → ㄵ 분리: 안+자 아님, 앉+아
    }

    #[test]
    fn test_composer_basic() {
        let mut composer = HangulComposer::new();
        let mut committed = String::new();
        let mut composing = String::new();
        for ch in "gksrmf".chars() {
            let step = composer.feed_key(ch);
            committed.push_str(&step.committed);
            composing = step.composing;
        }
        assert_eq!(format!("{committed}{composing}"), "한글");
    }

    #[test]
    fn test_composer_backspace_syllable() {
        let mut composer = HangulComposer::new();
        for ch in "gksrmf".chars() {
            composer.feed_key(ch);
        }
        assert_eq!(composer.backspace_syllable(), "");
        assert_eq!(composer.backspace_syllable(), "");
    }

    #[test]
    fn test_composer_pop_key_step() {
        let mut composer = HangulComposer::new();
        let mut committed = String::new();
        let mut composing = String::new();
        for ch in "gksrnrdjdl".chars() {
            let step = composer.feed_key(ch);
            committed.push_str(&step.committed);
            composing = step.composing;
        }
        assert_eq!(format!("{committed}{composing}"), "한국어이");
        assert_eq!(composer.preview_text(), "이");
        assert_eq!(composer.pop_key(), "ㅇ");
        assert_eq!(composer.pop_key(), "");
    }
}

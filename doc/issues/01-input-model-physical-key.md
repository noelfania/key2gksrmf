# 입력 모델: 물리키 기반 한글 조합

- **상태**: 해결
- **관련 계획**: `.cursor/plans/physical_key_input_1b0fdef8.plan.md`
- **관련 코드**: `src/main.rs` (`edit_proc`), `src/hangul_engine.rs` (`HangulComposer`)

## 고민한 점

- 영문 **문자값**을 받아 변환할지, 키보드 **물리키**를 받아 변환할지.
- Win32 `EDIT`의 `EN_CHANGE` 후처리로 전체 텍스트를 다시 변환하면 캐럿·백스페이스·선택 영역이 어색해진다.
- `gksrmf` 입력 시 `ㅎㅏㄴㄱㅡㄹ`처럼 자모가 나뉘어 보이는 문제가 있었다.

## 원인

- `EN_CHANGE`는 이미 화면에 들어간 문자열을 기준으로 후처리한다.
- 조합 중인 음절과 확정된 음절의 경계를 OS 기본 편집 모델과 맞추기 어렵다.
- IME·레이아웃에 따라 들어오는 문자와 의도한 영문 키 시퀀스가 어긋날 수 있다.

## 결정

- 입력창 포커스 상태에서 `WM_KEYDOWN`으로 물리키를 직접 받는다.
- `HangulComposer`가 두벌식 키 시퀀스를 조합하고, 확정분은 실제 완성형 한글로 커밋한다.
- `EN_CHANGE` 기반 `G_RAW_INPUT` / `handle_text_change` 흐름은 제거했다.

## 동작 기준 (해결 후)

- `gksrmf` → `한글`
- 조합 중 Backspace: `하` → `ㅎ` → 빈 조합
- 완성 후 Backspace: 음절 단위 삭제
- Space/Enter/방향키/마우스 클릭 등은 조합을 먼저 커밋
- `Ctrl+A`: 조합 커밋 후 전체 선택

## 남은 리스크

- 조합 중 `Ctrl+Z/Y`는 기본 `EDIT` 동작에 맡기므로 undo 체감은 경계마다 달라질 수 있다.
- 붙여넣기 직후·방향키 이동 후 재입력은 추가 수동 점검 권장.

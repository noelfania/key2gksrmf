---
name: editor-test
description: key2gksrmf 에디터 입력 변환(한글 조합·Backspace·Ctrl+Backspace·한/영 전환·캐럿/선택 처리) 회귀 테스트를 실행하고 passed/failed를 보고한다. src/hangul_engine.rs, src/input_controller.rs(및 input_controller/tests.rs), src/main.rs의 edit_proc·키 처리, src/editor_window.rs의 EditBuffer·create_edit_control, tests/editor_cases.txt 중 하나라도 수정했다면 작업을 끝내기 전에 반드시 실행한다. 사용자가 "입력 테스트", "변환 테스트", "editor test"를 요청할 때도 사용한다.
---

# 에디터 입력 변환 테스트

`tests/editor_cases.txt`의 시나리오를 **메모리 버퍼**(`MemBuffer`)와 **실제 Win32 EDIT 컨트롤**(`RealEdit`) 양쪽에서 실행한다. 둘 다 운영 코드와 같은 `InputController`를 거친다.

## 절차

1. 테스트를 실행한다. release exe가 실행 중이어도 debug 타깃이라 상관없다.
   ```bash
   cargo test 2>&1 | grep -E "warning|error|FAILED|panicked|passed /|test result|^\s+\[|^\s+(start|keys|expect|actual)"
   ```
2. 결과를 보고한다.
   - 맨 앞에 한 줄 요약: `editor-test: N passed / M failed` (마지막 `test result:` 줄 기준). 케이스 단위 결과(`MemBuffer: x passed / y failed`, `RealEdit: ...`)도 함께 적는다.
   - 실패가 있으면 표로 정리한다: 케이스 이름 | 버퍼 | keys | expect | actual
   - MemBuffer만 실패하고 RealEdit은 통과하면, 실제 EDIT 동작과 시뮬레이션이 어긋난 것이다. 이때는 `MemBuffer` 쪽을 실제 EDIT에 맞춘다.
3. 실패하면 원인을 찾아 **코드를 고친다**.
   - **기대값(`expect:`)을 실제 결과에 맞춰 바꾸는 방식으로 통과시키지 않는다.** 사양이 바뀌어 기대값을 수정해야 한다고 판단되면, 바꾸기 전에 사용자에게 이유를 설명하고 확인을 받는다.
   - 고친 뒤 1번부터 다시 실행해서 모두 통과하는 것을 확인한다.
4. 새 버그를 고쳤거나 새 입력 동작을 추가했으면 재현 케이스를 `tests/editor_cases.txt`에 추가한다. 수정 전 코드에서는 실패하고 수정 후에는 통과해야 한다.

## 케이스 작성법

```
[케이스 이름]
start: 가|라          # 생략하면 빈 문서 "|"
keys: sk
expect: 가나|라
```

- **상태 마크업**: `|` 캐럿, `[..]` 선택 영역, `\n` 줄바꿈(EDIT 안에서는 CRLF). `\|` `\[` `\]` `\\`는 문자 그대로. 마커가 없으면 캐럿은 문서 끝이다.
- **keys**
  - `a`~`z`: 물리키. 한 모드면 두벌식으로 조합된다. `A`~`Z`: Shift+키. 숫자, 공백, 기호는 그대로 입력된다.
  - `{Space} {Enter} {Tab} {BS} {Del} {Left} {Right} {Up} {Down} {Home} {End}`
  - `{F1}`: 한/영 전환. `{Ctrl+A}` `{Ctrl+BS}` `{Ctrl+X}`: Ctrl 조합. `{Alt+X}`: Alt 조합.
  - `{Click:N}`: UTF-16 위치 N을 마우스로 클릭. `{Paste:텍스트}`: Ctrl+V 붙여넣기. `{{`: 문자 `{`.
  - 한글이나 이모지를 직접 입력하는 키는 없다. 그런 텍스트는 `start:`나 `{Paste:...}`로 넣는다.
- 위치와 길이는 EDIT과 같이 **UTF-16 단위**다(이모지 = 2, 줄바꿈 = 2).

## 구조 참고

- `src/input_controller.rs`: `InputController`(모드, 조합 추적, WM_CHAR 억제)와 `TextBuffer` 트레이트
- `src/input_controller/tests.rs`: 케이스 파서, `MemBuffer`, `RealEdit`, 실행기(`run_keys`). 운영 메시지 흐름을 따른다(`handle_key` → PassThrough면 EDIT 기본 키 처리 → `filter_char` → WM_CHAR).
- F1과 Alt+Z는 운영에서 컨트롤러 밖(메시지 루프/`edit_proc`)에서 처리된다. 실행기는 F1을 `toggle_mode()`로 흉내 낸다.

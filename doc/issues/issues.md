# 개발 중 고민·이슈 정리

key2gksrmf를 만들면서 마주한 문제, 검토, 해결 여부를 모아 둔 문서입니다.

## 분류 기준

| 상태 | 의미 |
|---|---|
| **해결** | 구현 완료, 동작 확인됨 |
| **검토** | 가능 여부·정책만 확인하고 코드 변경 없음, 또는 별도 기능으로 보류 |
| **보류** | 의도적으로 다음 버전으로 미룸 |
| **미확인** | 추가 수동 점검 또는 후속 작업 필요 |

## 목차

### 해결

| 문서 | 요약 |
|---|---|
| [01-input-model-physical-key.md](01-input-model-physical-key.md) | `EN_CHANGE` 후처리 → `WM_KEYDOWN` 물리키 조합으로 전환 |
| [02-backspace-compose-tail.md](02-backspace-compose-tail.md) | 백스페이스 시 앞 음절 오염 (`한국엉`) 수정 |
| [03-han-eng-mode-toggle.md](03-han-eng-mode-toggle.md) | 오른쪽 Alt 충돌 → 상태바 `ㅎ`/`A` + F1 단축키 |
| [04-embedded-korean-font.md](04-embedded-korean-font.md) | 한글 폰트 없는 환경 대비 Pretendard Regular 내장 |
| [05-ui-window-behavior.md](05-ui-window-behavior.md) | 캐럿, 스크롤바, 작업 표시줄, 트레이, 상태바 등 UI |

### 검토

| 문서 | 요약 |
|---|---|
| [06-unicode-clipboard.md](06-unicode-clipboard.md) | UTF-8 vs Windows 클립보드 `CF_UNICODETEXT` 정책 |

### 보류

| 문서 | 요약 |
|---|---|
| [07-deferred-features.md](07-deferred-features.md) | 줄 끝 기호, 시스템 폰트 설정 UI 등 |

### 미확인

| 문서 | 요약 |
|---|---|
| [08-open-checks.md](08-open-checks.md) | Undo/Redo, Ctrl+Backspace, SmartScreen 등 |

---

## 추가 해결 기록

아래는 개별 이슈 파일로 쪼개기 전·후에 누적된 해결 메모입니다. 상세 구현은 [app-detailed-design.md](../app-detailed-design.md)를 우선합니다.

### 단일 프로그램

- 이름 있는 Windows mutex(`Local\\key2gksrmf_single_instance`)로 중복 실행을 차단했다.
- 두 번째 실행 시 새 프로세스는 종료되고, 기존 실행 중인 창(`key2gksrmf_wnd`)을 복원/포커스한다.
- 상수·타이틀 생성은 `src/app_info.rs`에 둔다.

### 브랜딩·레이아웃·입력 보정

- 상태바: `ㅎ`/`A` 라디오 + `Wrap` + `Always on Top` 체크박스.
- Wrap 해제 시에만 에디터에 가로 스크롤(`WS_HSCROLL`, `ES_AUTOHSCROLL`)을 사용한다.
- `hangul_engine`에 `wnjT -> 줬`, `wnjt -> 줫` 회귀 테스트 추가. `Shift` 키다운이 조합을 끊지 않도록 보정.
- `Ctrl+Backspace`는 구두점(`?`, `!`, `.` 등)을 단어 경계로 취급. 회귀 테스트 포함.
- 표시명 `key2gksrmf`, 타이틀 `key2gksrmf 64bit {version}`, 트레이 메뉴 영어.

### Alt+Tab 포커스·Wrap 토글

- `WM_ACTIVATE`에서 `WA_INACTIVE`가 아니면 `SetFocus(EDIT)`.
- Wrap 체크박스 단축키 **Alt+Z**, `config.json`의 `word_wrap`에 영속화(기본 `true`).
- `ES_AUTOHSCROLL` 변경 불가 → 토글 시 EDIT 재생성 후 텍스트·선택·스크롤(`EM_SCROLLCARET`) 이관.

### 행 번호·툴팁·전역 단축키

- 거터(`k2g_gutter`, 폭 44px): 논리 행 번호, Wrap 연속행은 번호 생략, 현재 커서 행 강조.
- comctl32 툴팁: 라디오 `F1`, Wrap `Alt + Z`.
- 매니페스트로 comctl32 v6 로드 (`assets/app.manifest` → `RT_MANIFEST`). 구버전 comctl32에서는 `TTM_ADDTOOLW`가 `cbSize` 불일치로 거부됐다.
- 전역 단축키 Ctrl+Alt+Shift+G: 트레이 `Shortcuts` 서브메뉴로 on/off, 기본 off (`global_hotkey_open`).

### 레이아웃 다듬기·색상 테마

- 외곽 8px 여백 제거, 1px 구분선 3종(타이틀↔상태바·상태바↔에디터·거터↔에디터).
- 에디터 내부 여백: `EM_SETRECT` 상단 6px·왼쪽 4px (`WM_SIZE`/`WM_SETFONT` 후 재적용).
- `config.json` `colors`: `gutter_background`, `line_number`, `status_bar`, `editor_background` (`#RRGGBB`).

### UI 레이아웃·아이콘 보정

- `WS_CLIPCHILDREN`으로 구분선이 컨트롤을 가리지 않게 함.
- 리사이즈: `SWP_NOZORDER`, `DeferWindowPos`, 최소 창 폭 제한. Wrap/AOT를 Hide/Show 하면 테마 체크박스가 깨짐.
- `WM_CTLCOLORBTN`에서 테마 버튼에 커스텀 브러시를 주지 않음.
- Wrap/AOT 체크박스에는 Pretendard를 넣지 않음(체크 □ 잔상). Pretendard는 에디터·ㅎ/A만.
- 상태바 배경 STATIC 자식 제거 → 부모 `WM_PAINT`에서 칠함.
- 아이콘: `key2gksrmf_light.ico`(검은 '나') / `key2gksrmf_dark.ico`(흰 '나'), `doc/images/app-icon.png` / `app-icon-dark.png`. 원본은 `assets/icons/key2gksrmf-na-source.png`.

### 물리키 입력 전환 후 남은 확인

- 조합 중 `Ctrl+Z/Y` undo 체감, 붙여넣기·방향키·마우스 클릭 후 재입력 → [08-open-checks.md](08-open-checks.md).

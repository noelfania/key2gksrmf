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
| [03-han-eng-mode-toggle.md](03-han-eng-mode-toggle.md) | 오른쪽 Alt 충돌 → 상단바 `한`/`A` + F1 단축키 |
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

### 입력 컨트롤러 분리·에디터 시나리오 테스트

- 조합·편집 로직을 `main.rs`에서 `src/input_controller.rs`(`InputController`, `TextBuffer`)로 옮겼다. Win32 메시지를 모르므로 테스트에서 그대로 실행할 수 있다.
- `tests/editor_cases.txt`의 시나리오를 메모리 버퍼와 **실제 Win32 EDIT**(숨김 창) 양쪽에서 실행한다. 두 버퍼 결과가 같다는 것도 이 테스트로 함께 확인된다.
- 입력 코드를 고친 뒤 실행할 Claude 스킬: `.claude/skills/editor-test/SKILL.md` (`/editor-test`). 규칙은 루트 `CLAUDE.md`.
- 이 테스트로 재현 후 수정한 버그:
  - `Ctrl+Backspace`가 EM_GETSEL의 UTF-16 위치를 문자 인덱스로 써서, 앞에 이모지가 있으면 엉뚱한 범위를 지웠다.
  - 한 모드에서 `Alt+문자`가 자모로 입력됐다 (`WM_SYSKEYDOWN`도 조합 경로로 들어감).
  - EDIT 기본 입력 한도(32,767자)에 걸려 긴 문서에서 입력이 막혔다 → `EM_SETLIMITTEXT(0)`.
- 수식키 상태는 `GetAsyncKeyState`(처리 시점) 대신 `GetKeyState`(메시지 시점)로 읽는다.

### 다크 모드·상단바 재디자인·DPI

- 테마 라디오/체크박스 4개를 커스텀 그리기 창 `k2g_toolbar` 하나로 교체했다. 위 "UI 레이아웃·아이콘 보정"의 테마 체크박스 잔상, □ 글리프, `WM_CTLCOLORBTN` 제약이 함께 사라졌다.
  - Wrap/Always on Top 글씨가 이상했던 원인: 체크박스에 `WM_SETFONT`를 하지 않아 비트맵 System 폰트로 그려지고 있었다.
- `한`|`A` 세그먼트, Wrap(↵)·항상 위(핀) 아이콘 토글, 단축키 배지(`F1`, `Alt+Z`). 도형은 GDI+로 안티앨리어싱.
- 다크 모드: 앱 모드(`AppsUseLightTheme`)를 따라 제목 표시줄(DWM)·팔레트·스크롤바·툴팁·트레이 메뉴를 다크로. 실행 중 전환은 `WM_SETTINGCHANGE("ImmersiveColorSet")`에서 즉시 반영.
- 창 아이콘도 테마를 따른다. 기존에는 시작할 때 `WNDCLASSW.hIcon`(32px)만 정해져 트레이만 테마를 따라갔다.
- 매니페스트에 DPI 선언이 없어 125~150%에서 창 전체가 흐리게 확대되던 것을 PerMonitorV2로 바꿨다.
- 기존 config의 기본 색상(`#F0F0F0`, `#6E6E6E`)은 새 팔레트 값으로 자동 이전한다. 다크 모드 덮어쓰기는 `colors_dark`.

### 트레이 개선

- 탐색기가 다시 시작되면 트레이 아이콘이 사라지던 문제 → `TaskbarCreated`에서 재등록.
- `NOTIFYICON_VERSION_4`: 키보드(Win+B) 접근, 단일 클릭으로 창 열기/숨기기.
- 메뉴: 굵은 Open/Hide 기본 항목, Word Wrap 추가, 단축키 오른쪽 정렬, `Shortcuts` 서브메뉴 제거, 버전/GitHub 항목, `TrackPopupMenu` 뒤 `WM_NULL`.
- 시작 프로그램 레지스트리 값은 따옴표로 감싼 경로. 켜져 있으면 시작할 때마다 현재 경로로 갱신한다.
- 디버그 빌드는 별도 인스턴스 이름을 쓰고 시작 프로그램 레지스트리를 건드리지 않는다. 개발 중 디버그 빌드를 실행하면, 디버그 config(기본 off)에 맞춰 실제 등록이 지워지는 문제가 있었다.

### 물리키 입력 전환 후 남은 확인

- 조합 중 `Ctrl+Z/Y` undo 체감 → [08-open-checks.md](08-open-checks.md).
- 붙여넣기·방향키·마우스 클릭 후 재입력은 에디터 시나리오 테스트로 옮겼다.

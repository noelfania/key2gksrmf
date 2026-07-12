# 개발 중 고민·이슈 정리

gksrmf를 만들면서 마주한 문제, 검토, 해결 여부를 모아 둔 문서입니다.  
`.cursor/plans`의 계획과 실제 구현·대화 맥락을 바탕으로 정리했습니다.

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
| [03-han-eng-mode-toggle.md](03-han-eng-mode-toggle.md) | 오른쪽 Alt 충돌 → 상태바 클릭 + F1 단축키 |
| [04-embedded-korean-font.md](04-embedded-korean-font.md) | 한글 폰트 없는 환경 대비 오픈 폰트 내장 |
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

## 보류 항목

### [검토] Unicode 클립보드와 UTF-8 복사

- 확인 내용:
  - 앱 내부 문자열은 Rust `String`이므로 UTF-8이다.
  - Win32 `EDIT` 컨트롤과 `W` API는 UTF-16 기반으로 동작한다.
  - Windows 표준 텍스트 클립보드는 보통 `CF_UNICODETEXT`이며, 이는 UTF-16LE 기반이다.
  - 웹브라우저나 최신 메모장이 UTF-8 파일을 다루더라도, Windows 앱 간 복사/붙여넣기 표준 경로는 대개 `CF_UNICODETEXT`이다.
- 결론:
  - 기본 복사는 UTF-8 바이트 복사가 아니라 Windows 표준 유니코드 텍스트 복사로 유지한다.
  - 일반 앱(메모장, 브라우저, IDE, 메신저 등)에 붙여넣을 때 한글이 깨지지 않는 것을 우선한다.
  - UTF-8 바이트 자체가 필요한 경우는 `UTF-8 파일로 내보내기` 또는 커스텀 클립보드 포맷 같은 별도 기능으로 검토한다.



## 물리키 입력 전환 점검 결과

- 적용 상태:
  - 입력 모델을 `EN_CHANGE` 후처리에서 `WM_KEYDOWN` 물리키 처리로 전환했다.
  - 두벌식 매핑 키 입력은 조합 상태를 거쳐 실제 완성형 한글 텍스트로 커밋된다.
  - `Ctrl+A`는 조합 중 상태를 먼저 커밋한 뒤 전체 선택을 수행한다.
  - `Ctrl+Backspace`는 조합 중에는 음절 단위, 비조합 상태에서는 단어 단위 삭제를 수행한다.
- 기대 개선:
  - 기존처럼 `ㅎㅏㄴㄱㅡㄹ` 형태로 분해된 문자열이 남는 빈도가 낮아진다.
  - 완성된 글자는 실제 완성형 한글로 남아 일반 편집 동작과의 일관성이 좋아진다.
- 영향 및 남은 확인 포인트:
  - 조합 중 `Ctrl+Z/Y`는 기본 `EDIT` 동작으로 전달되므로, 조합 경계 직전/직후의 undo 체감은 추가 수동 점검이 필요하다.
  - 마우스 중간 클릭 편집, 붙여넣기 직후 조합 시작, 방향키 이동 후 재입력 시나리오는 실제 사용 테스트를 추가로 권장한다.

## 단일 프로그램 (해결)
- 이름 있는 Windows mutex(`Local\\key2gksrmf_single_instance`)로 중복 실행을 차단했다.
- 두 번째 실행 시 새 프로세스는 종료되고, 기존 실행 중인 창(`key2gksrmf_wnd`)을 복원/포커스한다.

## 브랜딩·레이아웃·입력 보정 (해결)

### [레이아웃 변경](/doc/images/layout.png)
- 상단 상태바를 `F1` 그룹박스 안의 `ㅎ`/`A` 라디오 + `Wrap` + `Always on Top` 체크박스 형태로 변경했다.
- Wrap 해제 시에만 에디터에 가로 스크롤(`WS_HSCROLL`, `ES_AUTOHSCROLL`)을 사용한다.

### 받침 이슈 (`줬` / `줫`)
- `hangul_engine`에 `wnjT -> 줬`, `wnjt -> 줫` 회귀 테스트를 추가했다.
- `Shift+T` 입력 전에 `Shift` 키다운이 조합을 끊지 않도록 modifier 키 처리를 보정했다.

### Ctrl + Backspace 컨벤셔널 동작
- 구두점(`?`, `!`, `.` 등)을 단어 경계로 취급하도록 삭제 경계 로직을 수정했다.
- `입력한다? 입` 예시에서 `입`만 삭제되고 `입력한다?`가 유지되도록 회귀 테스트를 추가했다.

### 프로그램명 / 타이틀 / 트레이 메뉴
- 프로그램 표시명을 `key2gksrmf`로 통일했다.
- 창 타이틀은 `key2gksrmf 64bit {version}` 형식으로 표시한다.
- 트레이 우클릭 메뉴를 영어(`Always on Top`, `Start with Windows`, `Exit`)로 변경했다.

## Alt+Tab 포커스·Wrap 토글 (해결)

### Alt+Tab 후 타이핑 불가
- **원인**: 창이 활성화되어도 포커스가 에디터가 아닌 다른 컨트롤(또는 메인 창)에 남을 수 있음.
- **해결**: `WM_ACTIVATE`에서 `WA_INACTIVE`가 아니면 `SetFocus(EDIT)`로 에디터에 포커스를 준다.

### Word Wrap 체크박스
- 상태바 `Always on Top` 왼쪽에 `Wrap` 체크박스를 추가했다. 단축키는 **Alt+Z**.
- 기본값 `word_wrap=true`: 가로 스크롤 비활성, 창 너비 초과 문자열은 자동 줄바꿈.
- 해제 시: `WS_HSCROLL` + `ES_AUTOHSCROLL`로 가로 스크롤.
- Win32 `EDIT`은 생성 후 `ES_AUTOHSCROLL`을 바꿀 수 없어, 토글 시 컨트롤을 재생성하고 텍스트·선택 범위를 이관한다.
- `config.json`의 `word_wrap`에 영속화한다.

## 행 번호·툴팁·전역 단축키 (해결)

### 행 번호 거터
- Wrap 시 행 구분이 어려워 에디터 왼쪽에 폭 44px 거터를 추가했다.
- Win32 `EDIT`에는 행 번호 기능이 없어 자체 window class(`k2g_gutter`)로 `WM_PAINT`에서 직접 그린다.
- `EM_GETFIRSTVISIBLELINE`/`EM_LINEINDEX`로 시각행을 순회하고, 시작 문자 앞 개행 수로 **논리 행 번호**를 계산한다. Wrap으로 꺾인 연속행은 번호를 생략한다.

### 상태바 툴팁
- `F1` 그룹박스 레이아웃을 제거하고 `ㅎ`/`A` 라디오를 그대로 나열했다.
- 대신 comctl32 툴팁으로 라디오에 `F1`, Wrap 체크박스에 `Alt + Z`를 마우스오버 시 표시한다.

### 툴팁이 표시되지 않던 문제
- **원인**: 매니페스트가 없어 구버전 comctl32 5.82가 로드되는데, windows 크레이트의 `TTTOOLINFOW` 전체 크기(`cbSize`)는 v6 기준이라 `TTM_ADDTOOLW`가 조용히 거부됐다.
- **해결**: `assets/app.manifest`에 comctl32 v6 의존성을 선언하고 `key2gksrmf.rc`에 `RT_MANIFEST`로 임베드했다.
- 부수 효과: 체크박스·라디오·스크롤바가 현대식 테마 렌더링으로 바뀐다. exe 크기·성능 영향은 무시 가능한 수준.

## 레이아웃 다듬기·색상 테마 (해결)

### 여백 제거와 구분선
- 창 가장자리와 거터·에디터 사이 8px 외곽 여백을 제거하고 창 끝까지 채우도록 변경했다.
- 대신 1px 구분선 3종을 그린다: 타이틀바↔상태바·상태바↔에디터(메인 창 `WM_PAINT`), 거터↔에디터(`paint_gutter`).
- 에디터 첫 행이 구분선에 붙지 않도록 `EM_SETRECT`로 상단 6px·왼쪽 4px 내부 여백을 준다.
  `WM_SIZE`·`WM_SETFONT`가 포맷 영역을 초기화하므로 그 후마다 재적용해야 한다.
  거터는 `EM_GETRECT` 상단 오프셋을 읽으므로 행 번호 정렬이 자동으로 따라온다.

### 현재 커서 행 강조
- 거터에서 캐럿(`EM_GETSEL`)이 속한 논리 행 번호를 `COLOR_WINDOWTEXT`(진한 색)로, 나머지는 회색으로 표시한다.

### 색상 커스터마이즈
- `config.json`의 `colors`에 `gutter_background`, `line_number`, `status_bar`, `editor_background`를 `"#RRGGBB"`로 지정할 수 있다.
- 값이 없거나 파싱 실패 시 시스템 기본색으로 폴백한다 (`parse_hex_color` 단위 테스트 포함).
- `COLORREF`는 바이트 순서가 `0x00BBGGRR`이므로 파서에서 변환한다.

### 전역 단축키 Ctrl+Alt+Shift+G
- `RegisterHotKey`로 등록하고 `WM_HOTKEY`에서 창 표시 + 에디터 포커스 처리.
- 트레이 메뉴 `Ctrl+Alt+Shift+G (Open Window)` 체크로 on/off. 기본은 off (`global_hotkey_open: false`).
- 다른 앱이 조합을 선점해 등록 실패 시 토글에서는 메시지박스로 안내하고, 시작 시에는 조용히 설정을 원복한다.

## UI 레이아웃·아이콘 보정 (해결)

### 구분선이 컨트롤을 가리는 문제
- 메인 창에 `WS_CLIPCHILDREN`을 추가해 `WM_PAINT` 구분선이 라디오·체크박스 위를 덮지 않도록 했다.

### 리사이즈 시 레이아웃 깨짐
- `resize_editor_controls`에서 `HWND_TOP` 대신 `SWP_NOZORDER`로 z-order 변경을 막았다.
- `WM_SIZE` 후 `InvalidateRect`로 구분선 스트립을 다시 그리게 했다.

### 상태바 간격
- `STATUS_BAR_HEIGHT` 42→34, `STATUS_PAD` 4로 컨트롤 세로 중앙 정렬.

### Wrap 토글 시 스크롤 점프
- 에디터 재생성 후 `EM_SCROLLCARET`로 캐럿이 보이는 위치로 스크롤 복원.

### 아이콘 교체
- `icon-key2gksrmf-entry-2.png`에서 `key2gksrmf_light.ico`/`key2gksrmf_dark.ico` 및 `doc/images/app-icon.png`를 갱신했다.
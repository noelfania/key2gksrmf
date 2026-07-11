# key2gksrmf — 동작 구조 및 기능 사양 (상세설계)

> 대상: Windows 10/11 Win32 네이티브 단일 프로세스 트레이 앱
>
> 기준: 현재 구현 상태 (v0.1.0)
>
> **서비스사양**: [.cursor/rules/project-standards.mdc](../.cursor/rules/project-standards.mdc)

---

## 1. 목적 및 범위

### 1.1 목적

- 한글 키보드 레이아웃이 설치되지 않은 환경에서도 QWERTY 물리키로 두벌식 한글 입력이 가능하도록 한다.
- 트레이 상주형 경량 앱으로 실행 후 즉시 입력창에서 타이핑할 수 있게 한다.
- 웹뷰·대형 런타임 없이 Rust + Win32로 포터블하게 동작한다.

### 1.2 In-Scope

- 입력 창 포커스 안에서 `WM_KEYDOWN` 물리키 → 두벌식 한글 완성형 조합
- 한/영 모드 전환 (상태바 클릭, F1)
- 상태바 Wrap 체크박스(Alt+Z), Always on Top 체크박스, 트레이 컨텍스트 메뉴
- 설정 영속화(JSON), 창 위치·크기 저장, 시작 프로그램 레지스트리 동기화
- 내장 한글 폰트 (Pretendard Regular)
- Windows 표준 Unicode 클립보드 복사/붙여넣기

### 1.3 Out-of-Scope

- 웹/모바일/크로스플랫폼 GUI 프레임워크 전환
- 계정, 동기화, 클라우드 저장
- macOS/Linux 포팅
- 전역 물리키 훅 (다른 앱 포커스에서의 입력 변환)
- UTF-8 바이트 전용 클립보드 복사 (별도 기능으로만 검토)

### 1.4 런타임 특성

- Rust 2021, Win32 `W` API
- 단일 실행 파일, 무설치(포터블) 지향
- 릴리스: `lto`, `strip`, `opt-level = "z"`

---

## 2. 핵심 설계 결정

### 2.1 입력 모델 (현재 구현)

- **물리키 기반 조합**을 사용한다. `EDIT` subclass `edit_proc`에서 `WM_KEYDOWN`을 가로챈다.
- `map_physical_key_to_eng`로 가상키를 두벌식 영문 키로 매핑한 뒤 `HangulComposer::feed_key`에 전달한다.
- 조합 중 텍스트는 `EM_REPLACESEL`로 선택 영역만 갱신하고, 확정분(`committed`)은 즉시 커밋한다.
- 초기 `EN_CHANGE` 후처리 방식은 제거했다. (`doc/issues/01-input-model-physical-key.md` 참고)

### 2.2 조합 tail 분리

- Backspace 시 전체 키 버퍼 재변환으로 앞 음절이 오염되는 문제를 막기 위해 `ComposerStep { committed, composing }`를 사용한다.
- `pop_key()`는 마지막 키 1타만 되돌린다. 예: `한국어이` → Backspace → `한국어ㅇ`.

### 2.3 한/영 모드

- 오른쪽 Alt 토글은 Windows 시스템 키 충돌로 **채택하지 않음**.
- 상태바 `ㅎ` / `A` 라디오 버튼 클릭 또는 **F1**으로 전환.
- 한 모드: 물리키 조합. 영 모드: 기본 `EDIT` 동작에 위임.

### 2.4 문자 인코딩 및 클립보드 정책

- 앱 내부: Rust `String` (UTF-8)
- Win32 `EDIT` / `W` API: UTF-16
- 클립보드: Windows 표준 `CF_UNICODETEXT` (UTF-16LE)
- 기본 `Ctrl+C`는 표준 유니코드 텍스트 복사 유지. UTF-8 바이트가 필요하면 별도보내기 기능으로 검토.

### 2.5 폰트

- Pretendard Regular를 `include_bytes`로 내장, `AddFontMemResourceEx` 등록 후 `WM_SETFONT` 적용.
- 등록 실패 시 시스템 기본 폰트 fallback.

---

## 3. 모듈 구조 및 책임

```mermaid
flowchart LR
    main[main.rs] --> trayManager[tray_manager]
    main --> editorWindow[editor_window]
    main --> configStore[config_store]
    main --> startupRegistry[startup_registry]
    main --> fontManager[font_manager]
    main --> hangulEngine[hangul_engine]
    trayManager --> configStore
    editorWindow --> main
```

| 모듈 | 파일 | 역할 |
|---|---|---|
| 앱 진입점 | `src/main.rs` | `wnd_proc`, `edit_proc`, 메시지 루프, 전역 상태, F1 토글 |
| 트레이 | `src/tray_manager.rs` | 아이콘 등록, 테마별 아이콘, 컨텍스트 메뉴, 창 표시/포커스 |
| 에디터 창 | `src/editor_window.rs` | 메인 창 자식 컨트롤 생성·리사이즈, 항상 위 적용 |
| 한글 엔진 | `src/hangul_engine.rs` | `HangulComposer`, `convert_eng_to_kor`, 조합 테이블 |
| 설정 | `src/config_store.rs` | `config.json` 로드/저장 |
| 시작 프로그램 | `src/startup_registry.rs` | `HKCU\...\Run` 등록/해제 |
| 폰트 | `src/font_manager.rs` | 내장 폰트 메모리 등록, `HFONT` 생성·해제 |

> `input_adapter.rs`는 별도 모듈로 분리하지 않았다. 입력 라우팅은 `main.rs`의 `edit_proc`에 있다.

---

## 4. 기동~종료 생명주기

```mermaid
flowchart TD
    boot[ProcessStart] --> loadConfig[LoadAppConfig]
    loadConfig --> syncRegistry[SyncStartupRegistry]
    syncRegistry --> createOwner[CreateInvisibleOwnerWindow]
    createOwner --> createMain[CreateMainWindowAndControls]
    createMain --> loadFont[LoadEmbeddedFont]
    loadFont --> registerTray[RegisterTrayIcon]
    registerTray --> showWindow[ShowWindowAndFocusEdit]
    showWindow --> eventLoop[RunMessageLoop]

    eventLoop -->|TrayDoubleClick| focusWindow[ShowOrFocusWindow]
    eventLoop -->|TrayRightClick| openMenu[OpenContextMenu]
    eventLoop -->|F1| toggleMode[ToggleHanEngMode]
    eventLoop -->|WM_CLOSE| hideWindow[HideAndSaveBounds]
    openMenu -->|Exit| shutdown[CleanupAndQuit]

    focusWindow --> eventLoop
    toggleMode --> eventLoop
    hideWindow --> eventLoop
    shutdown --> endNode[ProcessExit]
```

---

## 5. UI 상세 설계

### 5.1 메인 창

- 기본 크기: 500×500 (`config.json`에서 복원 가능)
- 최초 실행: 창 표시 + 에디터 포커스
- 닫기·최소화: 트레이로 숨김 (종료 아님), 위치·크기 저장
- 작업 표시줄: 미표시 (invisible owner 창 패턴)
- 리사이즈: `WS_THICKFRAME`, 에디터 외곽 8px 마진

### 5.2 상태바 (높이 42px)

| 컨트롤 | ID | 설명 |
|---|---|---|
| 배경 | — | `STATIC` 컨테이너 |
| 한 모드 | `IDC_MODE_HAN` (2002) | `ㅎ` 라디오 버튼, 툴팁 `F1` |
| 영 모드 | `IDC_MODE_ENG` (2003) | `A` 라디오 버튼, 툴팁 `F1` |
| Wrap | `IDC_WORD_WRAP_CHECK` (2004) | 자동 줄바꿈 체크박스, 툴팁 `Alt + Z`, 기본 켜짐 |
| 항상 위 | `IDC_ALWAYS_ON_TOP_CHECK` (2001) | `Always on Top` 체크박스, 트레이 메뉴와 동기화 |

- 툴팁은 comctl32 `tooltips_class32` 하나에 `TTM_ADDTOOLW`(`TTF_IDISHWND | TTF_SUBCLASS`)로 등록

### 5.3 입력 영역

- Win32 멀티라인 `EDIT` (`ES_MULTILINE`, `ES_AUTOVSCROLL`, `ES_WANTRETURN`)
- `word_wrap=true`(기본): 가로 스크롤 없음 → 창 너비 초과 시 자동 줄바꿈
- `word_wrap=false`: `WS_HSCROLL` + `ES_AUTOHSCROLL`로 가로 스크롤
- Wrap 토글 시 `ES_AUTOHSCROLL`은 생성 후 변경 불가이므로 EDIT를 재생성하고 텍스트·선택을 이관
- `edit_proc` subclass로 키 입력 가로채기
- 내장 폰트 `WM_SETFONT` 적용
- `WM_ACTIVATE` 시 에디터에 자동 포커스 (Alt+Tab 복귀 후 즉시 타이핑 가능)

#### 레이아웃·구분선

- 거터·에디터는 창 가장자리까지 여백 없이 채운다 (외곽 마진 없음)
- 구분선 3종 (`COLOR_BTNSHADOW` 1px):
  - 타이틀바↔상태바: 클라이언트 최상단, 메인 창 `WM_PAINT`에서 그림
  - 상태바↔에디터: `STATUS_BAR_HEIGHT - 1` 위치, 메인 창 `WM_PAINT`에서 그림
  - 거터↔에디터: 거터 오른쪽 끝, `paint_gutter`에서 그림
- 에디터 첫 행 여백: `EM_SETRECT`로 포맷 영역에 상단 6px·왼쪽 4px 인셋.
  `WM_SIZE`·`WM_SETFONT`가 포맷 영역을 재계산하므로 리사이즈·폰트 적용 후 재호출 필요

#### 행 번호 거터 (폭 44px)

- 에디터 왼쪽에 자체 window class(`k2g_gutter`)의 자식 창을 배치하고 `WM_PAINT`에서 직접 그린다
- `EM_GETFIRSTVISIBLELINE` → `EM_LINEINDEX`로 시각행 시작 문자 인덱스를 얻고, 그 이전 개행 수로 **논리 행 번호** 계산
- Wrap으로 꺾인 연속 시각행에는 번호를 표시하지 않음 (논리 행 첫 시각행에만 표시)
- **현재 커서 행 강조**: `EM_GETSEL` 캐럿 위치의 논리 행 번호를 `COLOR_WINDOWTEXT`로, 나머지는 `line_number` 색으로 표시
- 행 높이는 에디터 폰트(`WM_GETFONT`)의 `GetTextMetricsW` 기준, `EM_GETRECT` 상단 오프셋으로 정렬 (에디터 상단 패딩과 자동 동기화)
- 갱신: `EN_CHANGE`/`EN_VSCROLL`, `WM_SIZE`, `edit_proc`의 키·마우스·스크롤 처리 후 `InvalidateRect`

#### 색상 테마 (`AppConfig.colors`)

| 설정 키 | 대상 | 기본값 (시스템 색) |
|---|---|---|
| `gutter_background` | 거터 배경 | `COLOR_BTNFACE` |
| `line_number` | 행 번호 텍스트 | `COLOR_GRAYTEXT` |
| `status_bar` | 상태바 배경 | `COLOR_BTNFACE` |
| `editor_background` | 에디터 배경 | `COLOR_WINDOW` |

- `"#RRGGBB"` 형식 (`config_store::parse_hex_color`), 없거나 파싱 실패 시 기본값으로 폴백
- 시작 시 `editor_window::init_theme`에서 브러시를 생성하고 `WM_CTLCOLORSTATIC`/`WM_CTLCOLORBTN`/`WM_CTLCOLOREDIT`·`paint_gutter`에서 사용, 종료 시 `cleanup_theme`

### 5.4 항상 위

- `WS_EX_TOPMOST` / `SetWindowPos(HWND_TOPMOST)` 토글
- 상태바 체크박스·트레이 메뉴·`AppConfig.always_on_top` 동기화

### 5.5 트레이

| 동작 | 결과 |
|---|---|
| 싱글클릭 | 없음 |
| 더블클릭 | 창 표시(`SW_RESTORE`) 및 포커스 |
| 우클릭 | Always on Top, Start with Windows, Ctrl+Alt+Shift+G (Open Window), Exit |

- 아이콘: 라이트/다크 테마별 리소스 (`WM_SETTINGCHANGE` 시 갱신)

### 5.6 전역 단축키 (Ctrl+Alt+Shift+G)

- `RegisterHotKey(hwnd, HOTKEY_ID_OPEN, MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_NOREPEAT, 'G')`
- `WM_HOTKEY` 수신 시 창 표시(`SW_RESTORE`) + 에디터 포커스 → 즉시 타이핑 가능
- 트레이 메뉴 `Ctrl+Alt+Shift+G (Open Window)` 체크로 on/off, 기본 **off**
- 등록 실패(다른 앱 선점) 시: 토글에서는 메시지박스 안내, 시작 시에는 조용히 설정 원복
- `AppConfig.global_hotkey_open`에 영속화

---

## 6. 실시간 변환 엔진 설계

### 6.1 API

| API | 용도 |
|---|---|
| `HangulComposer::feed_key(key) -> ComposerStep` | 물리키 1타 입력, committed/composing 분리 반환 |
| `HangulComposer::pop_key() -> String` | 마지막 키 1타 되돌림 |
| `convert_eng_to_kor(&str) -> String` | 문자열 일괄 변환 (테스트·레거시) |

### 6.2 조합 테이블

- `ENG_KEY`, `KOR_KEY`, `CHO_DATA`, `JUNG_DATA`, `JONG_DATA`
- 완성형: `0xAC00 + cho * 21 * 28 + jung * 28 + jong`

### 6.3 입력 처리 흐름 (`edit_proc`)

```mermaid
flowchart TD
    keyDown[WM_KEYDOWN] --> f1Check{F1?}
    f1Check -->|Yes| toggleMode[Handled in message loop]
    f1Check -->|No| ctrlCheck{Ctrl?}
    ctrlCheck -->|Ctrl+A| selectAll[SelectAll]
    ctrlCheck -->|Ctrl+Backspace| deleteWord[DeleteWordOrComposeStep]
    ctrlCheck -->|Other Ctrl| defaultEdit[Default EDIT]
    ctrlCheck -->|No| modeCheck{Han mode?}
    modeCheck -->|Yes| mapKey[map_physical_key_to_eng]
    mapKey --> composer[HangulComposer.feed_key]
    composer --> render[Commit + render composing tail]
    modeCheck -->|Eng| defaultEdit
    modeCheck -->|Backspace in compose| popKey[pop_key + render]
```

### 6.4 엔진 모듈 경계

- `hangul_engine`는 Win32, 트레이, 파일 I/O를 모르는 순수 로직 모듈
- 단위 테스트: `cargo test` (조합, backspace, 겹받침 등)

---

## 7. 설정 및 저장 설계

### 7.1 AppConfig 스키마

```json
{
  "always_on_top": false,
  "start_with_windows": false,
  "word_wrap": true,
  "global_hotkey_open": false,
  "colors": {
    "gutter_background": "#F0F0F0",
    "line_number": "#6E6E6E",
    "status_bar": "#F0F0F0",
    "editor_background": "#FFFFFF"
  },
  "window": {
    "x": 100,
    "y": 100,
    "width": 500,
    "height": 500
  }
}
```

- `x`, `y`는 optional. 없거나 유효 모니터 밖이면 `CW_USEDEFAULT`
- `word_wrap` 기본값: `true` (기존 config에 없으면 자동 줄바꿈)
- `global_hotkey_open` 기본값: `false` (전역 단축키 미등록)
- `colors`의 각 키는 optional `"#RRGGBB"`. 없거나 형식이 잘못되면 시스템 기본색으로 폴백 (5.3 색상 테마 참고)

### 7.2 저장 규칙

- 경로: 실행 파일과 같은 디렉터리의 `config.json`
- 저장 시점: 항상 위/시작 시 기동/Wrap/전역 단축키 토글, 창 숨김·종료·destroy 시 위치·크기
- 파싱 실패: `.json.bak` 백업 후 기본값

---

## 8. 시작 프로그램 레지스트리

- 키: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
- 값 이름: `key2gksrmf` (legacy `gksrmf` 값은 시작 시 정리)
- 앱 시작 시 `AppConfig`와 레지스트리 정합성 동기화
- 토글 실패 시 설정 롤백 + 메시지박스

---

## 9. 오류 처리 및 복원

| 상황 | 처리 |
|---|---|
| 설정 파일 없음 | 기본값으로 기동 |
| JSON 파싱 실패 | 백업 후 기본값 |
| 레지스트리 실패 | 알림, 체크 상태 원복 |
| 폰트 등록 실패 | 기본 폰트 fallback, 앱 계속 실행 |

---

## 10. 테스트 및 검증

### 10.1 단위 테스트 (`hangul_engine`, `main` tests)

- `gksrmf` → `한글`
- `한국어이` + pop_key → `ㅇ`
- 물리키 매핑, 겹받침/복모음

### 10.2 빌드

```bash
cargo test
cargo build --release
```

### 10.3 수동 점검 (미확인 항목)

- 조합 중 Undo/Redo (`doc/issues/08-open-checks.md`)
- Ctrl+Backspace 한글 경계
- SmartScreen, 타 PC 배포

---

## 11. 소스 맵

| 관심사 | 파일 |
|---|---|
| 앱 시작점·입력 라우팅 | `src/main.rs` |
| 트레이 | `src/tray_manager.rs` |
| 입력 창·상태바 | `src/editor_window.rs` |
| 한글 엔진 | `src/hangul_engine.rs` |
| 설정 | `src/config_store.rs` |
| 시작 프로그램 | `src/startup_registry.rs` |
| 내장 폰트 | `src/font_manager.rs` |
| 폰트 에셋 | `assets/fonts/Pretendard-Regular.ttf`, `OFL.txt` |
| 아이콘 | `assets/` (빌드 시 embed-resource) |

---

## 12. 관련 문서

- [서비스사양](../.cursor/rules/project-standards.mdc) — 목적, 범위, 기능 요약, 모듈 개요
- [개발·빌드·릴리스 가이드](dev-guide.md)
- [개발 이슈 정리](issues/issues.md) — 고민·해결·보류·미확인
- [README](../README.md) — 사용자용 소개 (일본어)

문서는 구현 변경 시 서비스사양·본 문서(상세설계)·코드를 함께 갱신한다.

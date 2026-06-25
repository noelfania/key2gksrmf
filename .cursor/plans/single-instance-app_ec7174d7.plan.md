---
name: single-instance-app
overview: 앱을 하나만 실행되도록 만들고, 두 번째 실행 시 기존 창을 트레이 숨김 상태에서 복원하도록 합니다.
todos:
  - id: add-single-instance-mutex
    content: "`src/main.rs`에 이름 있는 mutex 기반 단일 실행 가드를 추가한다."
    status: completed
  - id: focus-existing-window
    content: 중복 실행 시 기존 `gksrmf_wnd` 창을 찾아 복원/포커스한다.
    status: completed
  - id: update-windows-feature
    content: "`Cargo.toml`의 windows feature에 Threading API를 추가한다."
    status: completed
  - id: document-issue-resolution
    content: "`doc/issues/issues.md`의 단일 프로그램 항목을 구현 내용에 맞게 정리한다."
    status: completed
  - id: verify-build
    content: 테스트와 릴리스 빌드로 검증하고 수동 확인 항목을 점검한다.
    status: completed
isProject: false
---

# 단일 실행 인스턴스 계획

## 접근
- `src/main.rs` 시작 직후 이름 있는 Windows mutex를 생성합니다.
- `ERROR_ALREADY_EXISTS`이면 새 인스턴스는 메시지 루프를 시작하지 않고 종료합니다.
- 이때 기존 메인 창(`gksrmf_wnd`)이 있으면 `ShowWindow(SW_RESTORE)` + `SetForegroundWindow`로 표시합니다.
- 최초 인스턴스는 mutex `HANDLE`을 프로세스 종료까지 보관하고, 종료 시 `CloseHandle`로 정리합니다.

## 변경 파일
- `src/main.rs`
  - `CreateMutexW`, `GetLastError`, `CloseHandle`, `ERROR_ALREADY_EXISTS` import 추가
  - `G_SINGLE_INSTANCE_MUTEX: Option<HANDLE>` 전역 추가
  - `acquire_single_instance()` / `focus_existing_instance()` / `cleanup_single_instance_mutex()` 헬퍼 추가
  - `run()` 초반에서 중복 실행 여부 확인
  - `WM_DESTROY`, 메뉴 종료 경로에서 정리 호출
- `Cargo.toml`
  - `windows` feature에 `Win32_System_Threading` 추가
- `doc/issues/issues.md`
  - 선택된 “단일 프로그램” 메모를 해결 상태로 정리하거나 별도 이슈 항목으로 이동

## 검증
- `cargo test`
- `cargo build --release`
- 수동 확인
  - 첫 실행: 창 표시 및 트레이 아이콘 생성
  - 두 번째 실행: 새 트레이 아이콘/새 창이 생기지 않음
  - 기존 창이 숨김 상태일 때 두 번째 실행: 기존 창이 복원되고 포커스됨

## 주의점
- 두 번째 실행이 첫 번째 인스턴스의 창 생성 전에 매우 빠르게 들어오면, mutex 때문에 새 인스턴스는 종료하되 기존 창 포커스는 생략될 수 있습니다. 일반 사용자 실행 흐름에서는 문제가 거의 없습니다.
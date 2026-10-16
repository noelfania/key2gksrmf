# key2gksrmf

Win32 + Rust(`windows` 크레이트)로 만든 가벼운 Windows 11용 트레이 앱. 물리키 입력을 두벌식 한글로 조합하는 에디터 창을 띄운다. 외부 런타임이나 GUI 프레임워크 의존을 추가하지 않는다.

## 빌드·테스트

- `cargo test`: 한글 엔진, 입력 컨트롤러, 에디터 시나리오(`tests/editor_cases.txt`, 메모리 버퍼 + 실제 EDIT)
- `cargo build --release`: 앱이 실행 중이면 `target\release\key2gksrmf.exe`가 잠겨 실패한다. 앱을 종료하거나 `--target-dir`로 다른 곳에 빌드한다.

## 규칙

- 입력/변환 코드(`src/hangul_engine.rs`, `src/input_controller.rs`, `src/main.rs`의 `edit_proc`·키 처리, `src/editor_window.rs`의 `EditBuffer`·`create_edit_control`)나 `tests/editor_cases.txt`를 수정했다면, 작업을 끝내기 전에 `/editor-test` 스킬을 실행해 모두 통과하는지 확인한다.
- 입력 버그를 고칠 때는 재현 케이스를 `tests/editor_cases.txt`에 먼저 추가해서 실패하는 것을 확인한 뒤 고친다. 기대값을 바꿔서 통과시키지 않는다.
- 문서: 개발 기록은 `doc/issues/`, 설계는 `doc/app-detailed-design.md`.
- 변경 내역은 `CHANGELOG.md`의 `## [Unreleased]`에 적는다. 릴리스는 `Cargo.toml` 버전을 올리고 CHANGELOG 섹션을 만든 뒤 `main`에 머지하면 GitHub Actions(`version-tag.yml` → `release.yml`)가 태그·빌드·릴리스를 한다 (`doc/dev-guide.md`).

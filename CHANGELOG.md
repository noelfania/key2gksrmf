# Changelog

key2gksrmf의 버전별 변경 내역입니다. 형식은 [Keep a Changelog](https://keepachangelog.com/ko/1.1.0/)를 따르고, 0.2.0부터 [Semantic Versioning](https://semver.org/lang/ko/)을 씁니다.

릴리스 방법: `[Unreleased]` 내용을 새 버전 제목 아래로 옮기고 `Cargo.toml`의 `version`을 같은 값으로 올려 `main`에 머지하면, GitHub Actions가 `vX.Y.Z` 태그를 만들고 빌드해 Releases에 올립니다. 릴리스 본문은 이 파일의 해당 버전 섹션입니다. ([개발 가이드](doc/dev-guide.md#릴리스-만들기))

## [Unreleased]

## [0.2.1] - 2026-10-16

### Added

- exe 파일 버전 정보(VERSIONINFO). 탐색기 속성의 "자세히"와 작업 관리자에 버전·이름이 표시된다. `Cargo.toml` 버전에서 빌드 시 생성.

### Fixed

- 웹 페이지 구조화 데이터(`softwareVersion`)가 0.1.1226으로 고정돼 있던 문제. 빌드 시 `Cargo.toml` 버전을 넣는다.

## [0.2.0] - 2026-10-16

### Added

- Windows 라이트/다크 모드 추종. 제목 표시줄, 상단바, 에디터, 거터, 스크롤바, 툴팁, 트레이 메뉴가 모두 다크로 바뀌고 실행 중 전환도 바로 반영된다.
- Windows 강조색을 상단바 선택 표시에 사용.
- 상단바 단축키 표시 (`F1`, `Alt+Z`). 창이 좁으면 자동으로 숨긴다.
- 모니터별 고배율 DPI(PerMonitorV2) 대응. 125~150%에서도 흐리지 않다.
- 트레이: 한 번 클릭으로 창 열기/숨기기, 키보드(Win+B) 접근, 메뉴에 Word Wrap·버전(GitHub) 항목, 툴팁에 현재 입력 모드 표시.
- `config.json`의 `colors_dark` (다크 모드 색상 덮어쓰기).
- 에디터 입력 시나리오 테스트 (`tests/editor_cases.txt`, 메모리 버퍼 + 실제 Win32 EDIT)와 Claude 스킬 `/editor-test`.

### Changed

- 앱 아이콘을 '나' 글자로 교체. 밝은 배경에는 검은 글자, 어두운 배경에는 흰 글자를 쓴다 (트레이·창·Alt+Tab).
- 상단바를 다시 디자인: `한|A` 세그먼트 토글, Wrap·항상 위 아이콘 토글.
- 트레이 메뉴 정리: 굵은 Open/Hide 기본 항목, 단축키 오른쪽 정렬, `Shortcuts` 서브메뉴를 `Global Hotkey` 항목으로 대체.
- 라이트 모드 기본 색상을 Windows 11 톤으로 변경. 이전 기본값을 쓰던 설정은 자동으로 옮긴다.
- 시작 프로그램 레지스트리 값을 따옴표로 감싼 현재 exe 경로로 저장하고, 시작할 때마다 갱신한다.

### Fixed

- 이모지 뒤에서 `Ctrl+Backspace`가 엉뚱한 범위를 지우던 문제.
- 한 모드에서 `Alt+문자`가 자모로 입력되던 문제.
- 32,767자를 넘으면 더 입력되지 않던 문제.
- 빠르게 입력할 때 Shift 상태를 잘못 읽을 수 있던 문제.
- 탐색기가 다시 시작되면 트레이 아이콘이 사라지던 문제.
- Wrap·Always on Top 글자가 비트맵 시스템 폰트로 그려지던 문제.

## [0.1.1226] - 2026-07-18

### Changed

- 웹 데모 SEO(메타·OG·sitemap) 정리.
- 상태바·트레이 UX 정리, 새 스타일 아이콘.

## [0.1.1224] - 2026-07-12

### Added

- 행 번호 거터, Wrap 토글(Alt+Z), 전역 단축키 Ctrl+Alt+Shift+G.
- `config.json`의 `colors` 테마 색상 설정.

### Changed

- 에디터 레이아웃·UX 개선.

## [0.1.1124] - 2026-06-29

### Added

- 웹 데모(GitHub Pages)와 README 미리보기.

### Changed

- 프로젝트 구조 정리.

## [0.1.112] - 2026-06-25

### Added

- 단일 인스턴스 실행 (두 번째 실행 시 기존 창 복원).

### Changed

- 릴리스 배포 방식을 zip으로 변경.

## [0.1.1] - 2026-06-24

### Changed

- 내장 폰트 변경.

## [0.1.0] - 2026-06-24

### Added

- 첫 릴리스. 트레이 상주 에디터에서 물리키를 두벌식 한글로 조합.

[Unreleased]: https://github.com/noelfania/key2gksrmf/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/noelfania/key2gksrmf/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/noelfania/key2gksrmf/compare/v0.1.1226...v0.2.0
[0.1.1226]: https://github.com/noelfania/key2gksrmf/compare/v0.1.1224...v0.1.1226
[0.1.1224]: https://github.com/noelfania/key2gksrmf/compare/v0.1.1124...v0.1.1224
[0.1.1124]: https://github.com/noelfania/key2gksrmf/compare/v0.1.112...v0.1.1124
[0.1.112]: https://github.com/noelfania/key2gksrmf/compare/v0.1.1...v0.1.112
[0.1.1]: https://github.com/noelfania/key2gksrmf/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/noelfania/key2gksrmf/releases/tag/v0.1.0

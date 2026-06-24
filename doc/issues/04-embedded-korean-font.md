# 한글 폰트 내장

- **상태**: 해결
- **관련 계획**: `.cursor/plans/embedded-korean-font_7914d67e.plan.md`
- **관련 코드**: `src/font_manager.rs`, `assets/fonts/`, `src/main.rs`

## 고민한 점

- 한글 키보드 레이아웃이 없는 환경과 **한글 폰트가 없는 환경**은 다르다.
- 시스템에 한글 글리프 폰트가 없으면 입력·복사 데이터는 맞아도 화면에 □( tofu )로 보일 수 있다.
- Windows 기본 폰트(맑은 고딕 등)를 앱에 그대로 내장하면 재배포 라이선스 문제가 있다.

## 검토한 대안

| 방안 | 판단 |
|---|---|
| 시스템 폰트만 사용 | 가볍지만 한글 미지원 환경에서 UI·본문 표시 불안정 |
| `맑은 고딕` 등 시스템 폰트 번들 | 라이선스 위험, 비권장 |
| OFL 오픈 폰트 1웨이트 내장 | 재배포 가능, 표시 안정성 확보, 바이너리 증가는 감수 |

## 결정

- **Noto Sans CJK KR Regular** 1파일을 `assets/fonts/`에 포함.
- `AddFontMemResourceEx`로 메모리 등록 후 `WM_SETFONT`로 `EDIT`, 한/영 버튼, 항상 위 체크박스에 적용.
- `OFL.txt` 라이선스 고지 포함.
- 등록 실패 시 Windows 기본 폰트 fallback으로 계속 동작.

## 트레이드오프

- 실행 파일 크기 증가 (Regular 1개로 제한).
- 더 작은 오픈 폰트(Pretendard, SUIT 등)로 교체 검토 여지는 남음.

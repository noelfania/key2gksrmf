# 한/영 입력 모드 전환

- **상태**: 해결
- **관련 계획**: `.cursor/plans/right-alt-toggle-fix_1af621b0.plan.md`
- **관련 코드**: `src/editor_window.rs`, `src/main.rs`

## 고민한 점

- 처음에는 오른쪽 Alt(`VK_RMENU`)로 한/영을 토글하려 했다.
- Windows에서 Alt는 시스템 키(`WM_SYSKEYDOWN`, `WM_SYSCHAR`)로 처리되어 메뉴 활성화 등과 충돌한다.
- 키보드 레이아웃·원격 데스크톱·가상화 환경마다 오른쪽 Alt 전달 방식이 다를 수 있다.
- “어떤 Windows 환경에서도 안정적”이 목표라면 키보드 단축키보다 명시적 UI가 낫다.

## 시도했던 방향 (기각)

- `VK_RMENU`만 잡아 토글
- extended bit로 오른쪽 Alt 판별
- `WM_SYSCHAR` 소비

→ 환경 의존성이 크고, 앱에 메뉴바가 없어도 OS 기본 Alt 동작과 겹친다.

## 결정

- 오른쪽 Alt 토글은 **제거**.
- 상태바에 클릭 가능한 `[한] | 영` / `한 | [영]` 버튼을 둔다.
- 단축키는 **F1**으로 토글 (메시지 루프에서 처리, 토글 후 에디터 포커스 복귀).
- 한 모드: 물리키 기반 한글 조합. 영 모드: 기본 `EDIT` 영문 입력.

## 부수 이슈 (해결)

- 상태바 한/영 표시·항상 위 체크박스가 안 보이던 문제는 Z-order·`WS_CLIPSIBLINGS`로 대응했다. (`05-ui-window-behavior.md` 참고)

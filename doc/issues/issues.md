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

### [보류] 폰트(Font) 설정 적용

- 요구: 입력창 폰트를 '맑은 고딕' 등 모던 시스템 폰트로 변경해 투박한 UI 개선.
- 보류 사유:
  - 핵심 입력 조합 로직(한글 변환, 중간 편집, Undo/Redo 등)과 성능 안정화를 우선하기 위해, UI 외관 개선은 후속 버전으로 이관.


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




## 가끔씩 받침이 제대로 완성이 안된다.
줬 줫

## ctrl + Backspace의 움직임
예를들어 '입력한다? 입' 라고 할때 ?까지만 지워지는게 보통의 움직임이지만 죄다 지워짐. 

## 프로그램명
keyTo-gksrmf

## 윈도우 타이틀 명
keyTo-gksrmf 32bit 0.1.1
> 32bit 맞나? 그리고 버전을 항상 일치시키려면??

## 단일 프로그램 (해결)
- 이름 있는 Windows mutex(`Local\\gksrmf_single_instance`)로 중복 실행을 차단했다.
- 두 번째 실행 시 새 프로세스는 종료되고, 기존 실행 중인 창(`gksrmf_wnd`)을 복원/포커스한다.
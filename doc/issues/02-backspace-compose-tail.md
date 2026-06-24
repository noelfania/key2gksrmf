# 백스페이스: 조합 tail 분리

- **상태**: 해결
- **관련 계획**: `.cursor/plans/backspace_composition_fix_8f6d3260.plan.md`
- **관련 코드**: `src/hangul_engine.rs` (`ComposerStep`, `pop_key`), `src/main.rs`

## 고민한 점

- 물리키 전환 후에도 `한국어이`에서 Backspace 시 `한국어ㅇ`이 아니라 `한국엉`이 되는 회귀가 있었다.
- “음절 단위 백스페이스”와 “조합 중 한 키 되돌리기”의 경계를 어디에 둘지.

## 원인

- 조합 구간 전체를 하나의 raw 키 버퍼로 유지했다.
- Backspace 시 버퍼 전체를 재변환하면서 **이미 커밋된 앞 음절**까지 다시 조합에 영향을 받았다.

## 결정

- `ComposerStep`으로 `committed`와 `composing` tail을 분리한다.
- `pop_key()`는 마지막 키 1타만 되돌린다.
- 화면에는 확정분을 먼저 붙이고, 조합 중인 tail만 선택 영역으로 갱신한다.
- compose tail이 없을 때만 기본 `EDIT` Backspace(완성 음절 삭제)로 넘긴다.

## 검증 예시

- `한국어이` + Backspace → `한국어ㅇ`
- 연속 Backspace로 `ㅇ` → 빈 조합
- `cargo test`에 `test_korean_e_backspace_to_ieung` 등 회귀 테스트 포함

## 구현 시 주의

- compose 영역 길이는 UTF-16 code unit 기준으로 추적한다 (`G_COMPOSE_LEN_UTF16`).
- 커서 점프를 막기 위해 `EM_SETSEL` / `EM_REPLACESEL` 순서를 유지한다.

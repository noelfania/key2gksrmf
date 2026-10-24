# '뿌' 예비 아이콘

- **상태**: 보류 (예비안만 준비, 앱은 '나' 아이콘 사용 중)

현재 앱 아이콘은 붓글씨 느낌의 '나'다. '뿌'로 바꿀지 검토 중이라, 바로 바꿀 수 있게 파일과 절차를 미리 준비해 둔다.

## 예비 파일

[`assets/icons/candidate-ppu/`](../../assets/icons/candidate-ppu/)에 실제 파일과 **같은 이름**으로 둔다. 적용할 때 그대로 덮어쓰면 된다.

| 예비 파일 | 덮어쓸 위치 | 용도 |
|---|---|---|
| `key2gksrmf_light.ico` | `assets/icons/key2gksrmf_light.ico` | 리소스 101, 검은 글자 (밝은 배경) |
| `key2gksrmf_dark.ico` | `assets/icons/key2gksrmf_dark.ico` | 리소스 102, 흰 글자 (어두운 배경) |
| `app-icon.png` | `doc/images/app-icon.png`, `web/public/app-icon.png` | README, 웹 파비콘·헤더 (256px) |
| `app-icon-dark.png` | `doc/images/app-icon-dark.png` | README 다크 모드 (256px) |
| `og-image.png` | `web/public/og-image.png` | 웹 OG 이미지 (512px, 흰 배경) |
| `key2gksrmf-ppu-source.png` | 그대로 `assets/icons/`로 이동 | 2048px 원본 |
| `make_ppu_icon.py` | (이동 안 함) | 위 파일 생성 스크립트 |

ICO는 현재 '나'와 같이 16·20·24·32·40·48·64·128·256px PNG 프레임 9개다.

## 만든 방법

- '나' 원본(`assets/icons/key2gksrmf-na-source.png`, 54×49)은 출처·글꼴을 알 수 없어 같은 글꼴로 '뿌'를 찍을 수 없었다.
- 대신 '나' 획을 재서 맞춰 그렸다: 획 두께 약 24px(256px 기준), 위가 꺾여 두툼하게 시작하는 머리, 둥근 끝, 오른쪽으로 살짝 올라가는 가로획.
- ㅃ은 획이 많아 18px로 조금 가늘게 해서 16px에서도 ㅂ 속이 보이게 했다.
- 모양을 고치려면 `make_ppu_icon.py`의 `draw_mask()` 좌표·폭을 바꾸고 다시 실행한다 (Python + Pillow).

```powershell
# 예비 파일 다시 생성 (+ '나'와 나란히 비교한 이미지)
python -I assets/icons/candidate-ppu/make_ppu_icon.py $env:TEMP\icon-compare.png
```

## 적용 절차

1. 파일 덮어쓰기 (저장소 루트에서):

   ```powershell
   $c = 'assets/icons/candidate-ppu'
   Copy-Item "$c/key2gksrmf_light.ico", "$c/key2gksrmf_dark.ico" assets/icons/
   Copy-Item "$c/app-icon.png", "$c/app-icon-dark.png" doc/images/
   Copy-Item "$c/app-icon.png", "$c/og-image.png" web/public/
   git mv "$c/key2gksrmf-ppu-source.png" assets/icons/
   ```

   `key2gksrmf.rc`, `tray_manager.rs`의 리소스 번호(101/102), 웹 `index.html`, README 이미지 경로는 파일 이름이 같아 바꿀 필요 없다.

2. '나'라고 적힌 주석·문서 문구를 '뿌'로 바꾼다:
   - `src/tray_manager.rs` `IDI_GKSRMF_LIGHT` 위 주석
   - `src/main.rs` 창 아이콘 설정 함수 문서 주석 (`/// 제목 표시줄 배경에 맞는 '나' 아이콘…`)
   - `doc/app-detailed-design.md`: 창 아이콘, 트레이 아이콘 항목
   - `doc/issues/issues.md` "UI 레이아웃·아이콘 보정"의 아이콘 줄 (원본 경로도 `key2gksrmf-ppu-source.png`로)
   - 이 문서 상태를 **해결**로 바꾸고 `issues.md` 목차를 옮긴다.

3. `CHANGELOG.md` `## [Unreleased]`에 추가:

   ```markdown
   ### Changed

   - 앱 아이콘을 '나'에서 '뿌'로 바꿨다 (트레이·창·exe·웹).
   ```

4. 확인:
   - `cargo build --release` 후 탐색기에서 exe 아이콘, 실행해서 트레이·제목 표시줄 아이콘을 라이트/다크 모드 각각 확인.
   - 탐색기 아이콘 캐시 때문에 exe 아이콘이 바로 안 바뀌어 보일 수 있다 (파일 이름을 바꿔 보거나 `ie4uinit.exe -show`).
   - 웹은 `web`에서 `npm run dev`로 파비콘·헤더 확인.

5. 필요 없어진 `assets/icons/candidate-ppu/` 폴더는 지우거나, 스크립트만 남긴다.

## 되돌리기

적용 커밋을 `git revert`하면 '나'로 돌아온다. '나' 원본·ICO는 git 이력에 있다 (`2ea670d`).

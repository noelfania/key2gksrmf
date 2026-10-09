# 개발·빌드·릴리스 가이드

개발자용 내부 문서입니다. 사용자용 소개는 [README.md](../README.md)（일본어）를 참고하세요.

---

## 릴리스 만들기

태그를 push하면 GitHub Actions가 자동으로 빌드하고, [GitHub Releases](https://github.com/noelfania/key2gksrmf/releases)에 zip을 첨부합니다.

워크플로: [.github/workflows/release.yml](../.github/workflows/release.yml)

```bash
# 예: Cargo.toml version 이 0.1.1226 이면
git tag v0.1.1226
git push origin v0.1.1226
```

- `Cargo.toml`의 `version`과 태그(`vX.Y.Z`)는 맞춰 둡니다.
- Releases에는 `key2gksrmf-v*-win64.zip`만 올립니다（Edge SmartScreen으로 exe 단독 다운로드가 막히는 경우가 있어 zip만 배포）.

## GitHub Pages (웹 데모)

- 워크플로: [.github/workflows/pages.yml](../.github/workflows/pages.yml)
- 배포 URL: `https://noelfania.github.io/key2gksrmf/`
- `main` 브랜치 push 시 자동 배포
- SEO: `web/index.html`（메타·OG·JSON-LD）, `web/public/robots.txt`·`sitemap.xml`, 영·한 본문은 `web/src/App.tsx`의 `.seo-intro`

로컬 실행/빌드:

```bash
cd web
npm install
npm run dev
npm run build
```

### 배포 후 SEO（Google Search Console）

Pages 배포가 끝난 뒤, Google 검색 노출을 위해 아래를 한 번 수행한다.

1. [Google Search Console](https://search.google.com/search-console)에 속성 등록  
   - URL: `https://noelfania.github.io/key2gksrmf/`
2. 소유권 확인（HTML 파일 방식）  
   - 파일: `web/public/google19a206f5b83a8f38.html`  
   - 공개 URL: `https://noelfania.github.io/key2gksrmf/google19a206f5b83a8f38.html`  
   - **확인 후에도 삭제하지 않는다**（검증 유지용）
3. sitemap 제출  
   - `https://noelfania.github.io/key2gksrmf/sitemap.xml`
4. URL 검사 → 색인 요청  
   - 대상: `https://noelfania.github.io/key2gksrmf/`

참고: 색인 반영까지 수일~수주 걸릴 수 있다. SEO 메타·본문 소스는 위「SEO」항목을 참고.

---

## 빌드 환경

| 항목 | 내용 |
|---|---|
| Rust | edition 2021 |
| 플랫폼 | Windows 10/11 |
| SDK | Win32 빌드 환경（Windows SDK） |

### Rust 설치（Windows）

```bash
winget install Rustlang.Rustup
rustc --version
cargo --version
```

---

## 빌드·테스트

```bash
# 개발 빌드
cargo build

# 릴리스 빌드（권장）
cargo build --release

# 테스트
cargo test
```

출력: `target/release/key2gksrmf.exe`

릴리스 프로필은 `Cargo.toml`에서 `lto`, `strip`, `opt-level = "z"`로 경량화합니다.

---

## 관련 문서

- [상세설계](app-detailed-design.md) — 모듈·API·구현 흐름
- [개발 이슈 정리](issues/issues.md) — 고민·결정·미확인 항목
- [서비스사양](../.cursor/rules/project-standards.mdc) — 목적·범위·기능 요약

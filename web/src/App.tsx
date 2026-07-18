import { createSignal, onMount } from "solid-js";
import type { JSX } from "solid-js";
import { HangulComposer } from "./hangulEngine";

type InputMode = "han" | "eng";

const composer = new HangulComposer();

function mapPhysicalCodeToEng(code: string, shift: boolean): string | null {
  if (!code.startsWith("Key")) return null;
  const alpha = code.slice(3);
  if (alpha.length !== 1) return null;
  const base = alpha.toLowerCase();
  if (!/^[a-z]$/.test(base)) return null;
  if (shift && "reqtwop".includes(base)) return base.toUpperCase();
  return base;
}

export default function App() {
  let textareaRef: HTMLTextAreaElement | undefined;
  const [mode, setMode] = createSignal<InputMode>("han");
  const [text, setText] = createSignal("");
  let composeStart: number | null = null;
  let composeLen = 0;

  const utf16Len = (s: string) => s.length;

  const getSelection = (): [number, number] => {
    const ta = textareaRef!;
    return [ta.selectionStart ?? 0, ta.selectionEnd ?? 0];
  };

  const setSelection = (start: number, end: number) => {
    textareaRef?.setSelectionRange(start, end);
  };

  const replaceRange = (start: number, end: number, insert: string) => {
    const current = text();
    const next = current.slice(0, start) + insert + current.slice(end);
    setText(next);
    const pos = start + utf16Len(insert);
    setSelection(pos, pos);
  };

  const clearCompositionTracking = () => {
    composeStart = null;
    composeLen = 0;
    composer.clear();
  };

  const commitComposition = () => {
    if (composer.hasPending()) clearCompositionTracking();
  };

  const startCompositionAtCaret = () => {
    const [start, end] = getSelection();
    if (start !== end) replaceRange(start, end, "");
    composeStart = start;
    composeLen = 0;
    composer.clear();
  };

  const renderCompositionText = (value: string) => {
    const start = composeStart ?? 0;
    const end = start + composeLen;
    replaceRange(start, end, value);
    composeLen = utf16Len(value);
  };

  const appendCommittedText = (value: string) => {
    if (!value) return;
    const start = composeStart ?? 0;
    const end = start + composeLen;
    replaceRange(start, end, value);
    composeStart = start + utf16Len(value);
    composeLen = 0;
  };

  const toggleMode = () => {
    commitComposition();
    setMode((m) => (m === "han" ? "eng" : "han"));
    textareaRef?.focus();
  };

  const handleHanBackspace = () => {
    const composing = composer.popKey();
    renderCompositionText(composing);
    if (!composing) clearCompositionTracking();
  };

  const handleKeyDown: JSX.EventHandler<HTMLTextAreaElement, KeyboardEvent> = (e) => {
    const key = e.key;

    if (key === "F1") {
      e.preventDefault();
      toggleMode();
      return;
    }

    if (key === "Shift" || key === "Control" || key === "Alt" || key === "Meta") {
      return;
    }

    if (e.ctrlKey && key === "Backspace") {
      if (composer.hasPending()) {
        e.preventDefault();
        handleHanBackspace();
      } else {
        commitComposition();
      }
      return;
    }

    if (e.ctrlKey || e.altKey || e.metaKey) {
      commitComposition();
      return;
    }

    if (key === "Backspace") {
      if (composer.hasPending()) {
        e.preventDefault();
        handleHanBackspace();
      }
      return;
    }

    if (mode() === "eng") {
      commitComposition();
      return;
    }

    const mapped = mapPhysicalCodeToEng(e.code, e.shiftKey);
    if (!mapped) {
      commitComposition();
      return;
    }

    e.preventDefault();
    const [start, end] = getSelection();
    if (composeStart == null || start !== end) {
      startCompositionAtCaret();
    }
    const step = composer.feedKey(mapped);
    appendCommittedText(step.committed);
    renderCompositionText(step.composing);
  };

  onMount(() => {
    textareaRef?.focus();
  });

  return (
    <main class="page">
      <header class="hero">
        <div class="hero-header">
          <div class="hero-brand">
            <img
              class="hero-icon"
              src={`${import.meta.env.BASE_URL}app-icon.png`}
              alt="key2gksrmf Hangul keyboard icon"
            />
            <h1>key2gksrmf</h1>
            <span class="hero-subtitle">Type Hangul with physical keys</span>
          </div>

          <nav class="hero-links" aria-label="Project links">
            <a
              class="release-button pages-button"
              href="https://github.com/noelfania/key2gksrmf"
              target="_blank"
              rel="noopener noreferrer"
            >
              View on GitHub
            </a>
            <a
              class="release-button"
              href="https://github.com/noelfania/key2gksrmf/releases"
              target="_blank"
              rel="noopener noreferrer"
            >
              Download Windows App
            </a>
          </nav>
        </div>

        <section class="seo-intro" aria-labelledby="seo-en-title">
          <h2 id="seo-en-title" class="seo-heading">
            Hangul keyboard online — no Korean IME required
          </h2>
          <p>
            Type Korean Hangul using your QWERTY keyboard with 2-set (dubeolsik) layout
            mapping. Example: <code>gksrmf</code> → <strong>한글</strong>. Useful when a
            Korean keyboard layout is not installed. Press <kbd>F1</kbd> to switch Hangul /
            English mode.
          </p>
          <h2 class="seo-heading" lang="ko">
            한글 자판·IME 없이 두벌식 한글 입력
          </h2>
          <p lang="ko">
            영문 키보드 물리키로 두벌식 한글을 바로 입력할 수 있는 웹 데모입니다. 한국어
            키보드 레이아웃이 없는 Windows·브라우저 환경에서도 <code>gksrmf</code> →{" "}
            <strong>한글</strong>처럼 입력할 수 있습니다. <kbd>F1</kbd> 또는 ㅎ/A로 한/영을
            전환하세요. Windows용 포터블 앱은 Releases에서 다운로드할 수 있습니다.
          </p>
        </section>
      </header>

      <section class="editor-card" aria-label="Hangul text editor">
        <div class="toolbar">
          <fieldset class="mode-box">
            <legend title="F1">F1</legend>
            <label>
              <input
                type="radio"
                name="mode"
                checked={mode() === "han"}
                onChange={() => {
                  commitComposition();
                  setMode("han");
                  textareaRef?.focus();
                }}
              />
              ㅎ
            </label>
            <label>
              <input
                type="radio"
                name="mode"
                checked={mode() === "eng"}
                onChange={() => {
                  commitComposition();
                  setMode("eng");
                  textareaRef?.focus();
                }}
              />
              A
            </label>
          </fieldset>

          <label class="aot">
            <input type="checkbox" disabled />
            Always on Top (native app only)
          </label>
        </div>

        <textarea
          ref={textareaRef}
          value={text()}
          onInput={(e) => setText(e.currentTarget.value)}
          onKeyDown={handleKeyDown}
          onBlur={commitComposition}
          class="editor"
          aria-label="Hangul editor — type with physical keys"
          placeholder="Type here (e.g. gksrmf → 한글) / 여기에 입력 (예: gksrmf → 한글)"
        />
      </section>

      <footer class="seo-footer">
        <p>
          <strong>key2gksrmf</strong> — open-source Hangul input helper for Windows and the
          web. Keywords: Hangul keyboard, Korean typing without IME, dubeolsik, 두벌식 한글
          입력, 영타 한글 변환.
        </p>
      </footer>
    </main>
  );
}

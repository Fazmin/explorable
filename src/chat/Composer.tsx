import { useEffect, useRef, useState } from "react";
import { ArrowUp, Square } from "lucide-react";
import { useStore, selectActive } from "../lib/store";

export function Composer() {
  const mode = useStore((s) => s.mode);
  const activeId = useStore((s) => s.activeId);
  const running = useStore((s) => (s.activeId ? s.convs[s.activeId]?.running ?? false : false));
  const pending = useStore((s) => (s.activeId ? s.convs[s.activeId]?.pending ?? null : null));
  const active = useStore(selectActive);
  const draftStyle = useStore((s) => s.draftStyle);
  const { send, stop, setStyle } = useStore.getState();
  const [text, setText] = useState("");
  const ref = useRef<HTMLTextAreaElement>(null);
  const style = active?.style ?? draftStyle;

  useEffect(() => {
    ref.current?.focus();
  }, [activeId]);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 240)}px`;
  }, [text]);

  const submit = () => {
    const t = text.trim();
    if (!t || running) return;
    setText("");
    void send(t);
  };

  const placeholder = pending
    ? "Pick an option above, or type a different answer…"
    : mode === "learn"
      ? "Ask about anything you're studying…"
      : "Describe what you're teaching or what you need…";

  return (
    <div className="composer-wrap">
      <div className="composer">
        <label className="visually-hidden" htmlFor="composer-input">
          Message
        </label>
        <textarea
          id="composer-input"
          ref={ref}
          rows={1}
          value={text}
          placeholder={placeholder}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
              e.preventDefault();
              submit();
            }
          }}
          maxLength={20000}
        />
        <div className="composer-bar">
          {mode === "learn" && (
            <div className="segmented style-toggle" role="group" aria-label="Tutoring style">
              <button type="button" aria-pressed={style === "direct"} onClick={() => setStyle("direct")} title="Clear, complete explanations">
                Explain
              </button>
              <button type="button" aria-pressed={style === "socratic"} onClick={() => setStyle("socratic")} title="Guiding questions and hints before answers">
                Guide me
              </button>
            </div>
          )}
          <span className="spacer" />
          <span className="composer-hint">Shift + Enter for a new line</span>
          {running ? (
            <button type="button" className="send-btn stop-btn" onClick={() => stop()} aria-label="Stop answering" title="Stop">
              <Square aria-hidden />
            </button>
          ) : (
            <button type="button" className="send-btn" onClick={submit} disabled={!text.trim()} aria-label="Send" title="Send">
              <ArrowUp aria-hidden />
            </button>
          )}
        </div>
      </div>
      <p className="composer-foot">Answers can be wrong. Check important details against your course materials.</p>
    </div>
  );
}

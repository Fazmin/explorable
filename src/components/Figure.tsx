import { useEffect, useRef, useState, type ReactNode } from "react";
import { BookmarkCheck, BookmarkPlus, Download, Maximize2 } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import { useStore } from "../lib/store";
import { EXPORTS, KIND_ICON, KIND_LABEL } from "../lib/kinds";
import { exportArtifact } from "../lib/exporting";
import type { ArtifactKind } from "../lib/types";

/** Where a rendered component lives; decides which actions make sense. */
export interface FigureContext {
  courseId: string | null;
  conversationId: string | null;
  /** Stable key: the tool-use id, or the library item id. */
  key: string;
  surface: "chat" | "library" | "present";
  /** Sends a follow-up question to the chat, or undefined where that isn't possible. */
  onPrompt?: (text: string) => Promise<void>;
  mode: "learn" | "teach";
}

interface Props {
  kind: ArtifactKind;
  title: string;
  payload: Record<string, unknown>;
  ctx: FigureContext;
  streaming?: boolean;
  status?: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
  bodyClass?: string;
  children: ReactNode;
}

export function Figure({ kind, title, payload, ctx, streaming, status, footer, wide, bodyClass, children }: Props) {
  const Icon = KIND_ICON[kind];
  const [saved, setSaved] = useState(false);
  const { toast, setPresent } = useStore.getState();

  const save = async () => {
    try {
      await api.saveArtifact({ courseId: ctx.courseId, conversationId: ctx.conversationId, kind, title, payload });
      setSaved(true);
      toast(`Saved “${title}” to the Library`, "success");
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  };

  const ready = !streaming;
  return (
    <section className={`figure${wide && ctx.surface === "chat" ? " figure-wide" : ""}`} aria-label={`${KIND_LABEL[kind]}: ${title}`}>
      {ctx.surface !== "present" && (
        <header className="figure-head">
          <span className="figure-kind" title={KIND_LABEL[kind]}>
            <Icon aria-hidden />
          </span>
          <h3 className="figure-title">{title}</h3>
          {status}
          {ready && (
            <div className="figure-actions">
              {ctx.surface === "chat" && (
                <button type="button" className="icon-btn" onClick={save} disabled={saved} aria-label={saved ? "Saved to Library" : "Save to Library"} title={saved ? "Saved to Library" : "Save to Library"}>
                  {saved ? <BookmarkCheck aria-hidden /> : <BookmarkPlus aria-hidden />}
                </button>
              )}
              <button
                type="button"
                className="icon-btn"
                aria-label="Present full screen"
                title="Present full screen"
                onClick={() => setPresent({ kind, title, payload, key: ctx.key })}
              >
                <Maximize2 aria-hidden />
              </button>
              <ExportMenu kind={kind} title={title} payload={payload} />
            </div>
          )}
        </header>
      )}
      <div className={bodyClass ?? "figure-body"}>{children}</div>
      {footer}
    </section>
  );
}

export function ExportMenu({ kind, title, payload }: { kind: ArtifactKind; title: string; payload: Record<string, unknown> }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const options = EXPORTS[kind];
  const toast = useStore((s) => s.toast);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setOpen(false);
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  if (options.length === 0) return null;
  const run = async (format: string) => {
    setOpen(false);
    try {
      if (await exportArtifact(kind, title, payload, format)) toast("Exported", "success");
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  };

  return (
    <div className="export-menu" ref={ref}>
      <button type="button" className="icon-btn" aria-haspopup="menu" aria-expanded={open} aria-label="Export" title="Export" onClick={() => setOpen(!open)}>
        <Download aria-hidden />
      </button>
      {open && (
        <div className="menu" role="menu">
          {options.map((o) => (
            <button key={o.format} type="button" role="menuitem" className="menu-item" onClick={() => run(o.format)}>
              <span className="grow">{o.label}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

import { useCallback, useEffect, useMemo, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { Library, MessageSquare, Trash2 } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import { useStore } from "../lib/store";
import { KIND_ICON, KIND_LABEL } from "../lib/kinds";
import { formatDate } from "../lib/exporting";
import type { Artifact, ArtifactKind } from "../lib/types";
import type { FigureContext } from "../components/Figure";
import { ArtifactRenderer } from "../renderers/ArtifactRenderer";

const FILTERS: ArtifactKind[] = ["interactive", "quiz", "flashcards", "worked_example", "lesson_plan", "rubric", "table"];

export function LibraryView() {
  const courseId = useStore((s) => s.courseId);
  const courses = useStore((s) => s.courses);
  const mode = useStore((s) => s.mode);
  const { toast, openConversation, newChat, send } = useStore.getState();
  const [items, setItems] = useState<Artifact[]>([]);
  const [kind, setKind] = useState<ArtifactKind | null>(null);
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [allCourses, setAllCourses] = useState(false);
  const [title, setTitle] = useState("");

  const load = useCallback(async () => {
    try {
      const list = await api.listArtifacts(allCourses ? null : courseId, kind, query.trim() || null);
      setItems(list);
      setSelectedId((id) => (id && list.some((a) => a.id === id) ? id : list[0]?.id ?? null));
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  }, [allCourses, courseId, kind, query, toast]);

  useEffect(() => {
    const t = setTimeout(load, 150);
    return () => clearTimeout(t);
  }, [load]);

  const selected = items.find((a) => a.id === selectedId) ?? null;
  useEffect(() => setTitle(selected?.title ?? ""), [selected?.id, selected?.title]);

  const ctx = useMemo<FigureContext | null>(
    () =>
      selected
        ? {
            courseId: selected.courseId,
            conversationId: selected.conversationId,
            key: `library:${selected.id}`,
            surface: "library",
            mode,
            onPrompt: async (text: string) => {
              newChat();
              if (!(await send(text))) throw new Error("The message couldn't be sent.");
            },
          }
        : null,
    [selected, mode, newChat, send],
  );

  const rename = async () => {
    if (!selected || !title.trim() || title.trim() === selected.title) return;
    try {
      await api.renameArtifact(selected.id, title.trim());
      setItems((list) => list.map((a) => (a.id === selected.id ? { ...a, title: title.trim() } : a)));
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  };

  const remove = async () => {
    if (!selected) return;
    const ok = await ask(`Remove “${selected.title}” from the Library?`, { title: "Remove item", kind: "warning", okLabel: "Remove", cancelLabel: "Cancel" });
    if (!ok) return;
    try {
      await api.deleteArtifact(selected.id);
      setSelectedId(null);
      await load();
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  };

  const courseName = (id: string | null) => courses.find((c) => c.id === id)?.name ?? "General";

  return (
    <>
      <header className="topbar">
        <h1>Library</h1>
        <span className="crumb">{allCourses ? "All courses" : courseName(courseId)}</span>
      </header>
      <div className="library view-scroll" style={{ overflow: "hidden" }}>
        <div className="library-list">
          <div className="library-filters">
            <input className="input" type="search" placeholder="Search saved items" value={query} onChange={(e) => setQuery(e.target.value)} aria-label="Search saved items" />
            <div className="kind-filter" role="group" aria-label="Filter by type">
              <button type="button" aria-pressed={kind === null} onClick={() => setKind(null)}>
                All
              </button>
              {FILTERS.map((k) => (
                <button key={k} type="button" aria-pressed={kind === k} onClick={() => setKind(kind === k ? null : k)}>
                  {KIND_LABEL[k]}
                </button>
              ))}
            </div>
            <label className="check-row">
              <input type="checkbox" checked={allCourses} onChange={(e) => setAllCourses(e.target.checked)} />
              <span>Show items from every course</span>
            </label>
          </div>
          <div className="library-items">
            {items.length === 0 && (
              <div className="empty">
                <Library aria-hidden />
                <h3>Nothing saved yet</h3>
                <p>Use the bookmark button on any interactive, quiz, lesson plan, or rubric in a chat to keep it here.</p>
              </div>
            )}
            {items.map((a) => {
              const Icon = KIND_ICON[a.kind] ?? Library;
              return (
                <button key={a.id} type="button" className="library-item" aria-current={a.id === selectedId} onClick={() => setSelectedId(a.id)}>
                  <Icon aria-hidden />
                  <span className="li-title">{a.title}</span>
                  <span className="li-meta">
                    {KIND_LABEL[a.kind]}, {allCourses ? `${courseName(a.courseId)}, ` : ""}
                    {formatDate(a.updatedAt)}
                  </span>
                </button>
              );
            })}
          </div>
        </div>
        <div className="library-viewer">
          {selected && ctx ? (
            <>
              <div className="viewer-head">
                <input value={title} onChange={(e) => setTitle(e.target.value)} onBlur={rename} onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()} aria-label="Title" />
                {selected.conversationId && (
                  <button type="button" className="btn btn-sm" onClick={() => openConversation(selected.conversationId!)}>
                    <MessageSquare aria-hidden /> Open chat
                  </button>
                )}
                <button type="button" className="btn btn-sm btn-ghost btn-danger" onClick={remove}>
                  <Trash2 aria-hidden /> Remove
                </button>
              </div>
              <ArtifactRenderer key={selected.id} kind={selected.kind} payload={selected.payload} ctx={ctx} />
            </>
          ) : (
            items.length > 0 && <p className="field-hint">Select an item to view it.</p>
          )}
        </div>
      </div>
    </>
  );
}

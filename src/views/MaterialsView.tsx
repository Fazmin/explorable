import { useCallback, useEffect, useState } from "react";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { ClipboardPaste, Eye, FileText, FileUp, Search, Trash2, X } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import { useStore } from "../lib/store";
import { formatDate } from "../lib/exporting";
import type { Material, MaterialHit } from "../lib/types";

export function MaterialsView() {
  const courseId = useStore((s) => s.courseId);
  const courses = useStore((s) => s.courses);
  const toast = useStore((s) => s.toast);
  const course = courses.find((c) => c.id === courseId);
  const [items, setItems] = useState<Material[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [pasting, setPasting] = useState(false);
  const [paste, setPaste] = useState({ title: "", content: "" });
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<MaterialHit[] | null>(null);
  const [preview, setPreview] = useState<{ title: string; content: string } | null>(null);

  const load = useCallback(async () => {
    if (!courseId) return;
    try {
      setItems(await api.listMaterials(courseId));
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  }, [courseId, toast]);

  useEffect(() => {
    void load();
    setHits(null);
  }, [load]);

  if (!courseId || !course) {
    return (
      <div className="empty">
        <FileText aria-hidden />
        <h3>Choose a course first</h3>
        <p>Course materials belong to a course. Pick or create one in the sidebar.</p>
      </div>
    );
  }

  const importFiles = async () => {
    const picked = await open({
      multiple: true,
      filters: [{ name: "Documents", extensions: ["pdf", "txt", "md", "markdown", "tex", "html", "htm", "csv", "rst"] }],
    });
    const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
    for (const path of paths) {
      const name = path.split(/[\\/]/).pop() ?? path;
      setBusy(`Importing ${name}`);
      try {
        const m = await api.importMaterial(courseId, path);
        toast(`Imported ${m.title} (${m.wordCount.toLocaleString()} words)`, "success");
      } catch (err) {
        toast(`${name}: ${errorMessage(err)}`, "error");
      }
    }
    setBusy(null);
    await load();
  };

  const savePaste = async () => {
    setBusy("Saving");
    try {
      await api.addMaterialText(courseId, paste.title, paste.content);
      setPaste({ title: "", content: "" });
      setPasting(false);
      await load();
      toast("Added to course materials", "success");
    } catch (err) {
      toast(errorMessage(err), "error");
    } finally {
      setBusy(null);
    }
  };

  const remove = async (m: Material) => {
    const ok = await ask(`Remove “${m.title}” from ${course.name}? The assistant will stop citing it.`, { title: "Remove material", kind: "warning", okLabel: "Remove", cancelLabel: "Cancel" });
    if (!ok) return;
    try {
      await api.deleteMaterial(m.id);
      await load();
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  };

  const search = async () => {
    if (!query.trim()) return;
    try {
      setHits(await api.searchMaterials(courseId, query));
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  };

  return (
    <>
      <header className="topbar">
        <h1>Course materials</h1>
        <span className="crumb">{course.name}</span>
      </header>
      <div className="view-scroll">
        <div className="view-pad" style={{ maxWidth: 860 }}>
          <div className="view-header">
            <div>
              <h2>Materials for {course.name}</h2>
              <p>The assistant searches these when answering questions in this course and cites them by title. Files stay on this computer.</p>
            </div>
            <div className="materials-actions">
              <button type="button" className="btn btn-primary" onClick={importFiles} disabled={!!busy}>
                <FileUp aria-hidden /> Import files
              </button>
              <button type="button" className="btn" onClick={() => setPasting(!pasting)} aria-pressed={pasting}>
                <ClipboardPaste aria-hidden /> Paste text
              </button>
            </div>
          </div>
          {busy && (
            <p className="thinking-line" role="status">
              <span className="spinner" /> {busy}…
            </p>
          )}
          {pasting && (
            <div className="card-editor" style={{ marginBottom: 18 }}>
              <label className="field">
                <span>Title</span>
                <input className="input" value={paste.title} onChange={(e) => setPaste({ ...paste, title: e.target.value })} placeholder="Week 4 lecture notes" />
              </label>
              <label className="field">
                <span>Text</span>
                <textarea className="textarea" style={{ minHeight: 180 }} value={paste.content} onChange={(e) => setPaste({ ...paste, content: e.target.value })} placeholder="Paste notes, a syllabus, or a reading" />
              </label>
              <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
                <button type="button" className="btn" onClick={() => setPasting(false)}>
                  Cancel
                </button>
                <button type="button" className="btn btn-primary" onClick={savePaste} disabled={!paste.content.trim() || !!busy}>
                  Add to materials
                </button>
              </div>
            </div>
          )}

          {items.length === 0 ? (
            <div className="empty">
              <FileText aria-hidden />
              <h3>No materials yet</h3>
              <p>Import PDFs, Markdown, or text files such as lecture notes, the syllabus, or readings. Scanned PDFs need OCR first.</p>
            </div>
          ) : (
            <>
              <div>
                {items.map((m) => (
                  <div key={m.id} className="material-row">
                    <FileText aria-hidden />
                    <span className="mr-title">{m.title}</span>
                    <span style={{ display: "flex", gap: 2, gridRow: "span 2" }}>
                      <button
                        type="button"
                        className="icon-btn"
                        aria-label={`Preview ${m.title}`}
                        onClick={async () => setPreview(await api.getMaterial(m.id).catch(() => null))}
                      >
                        <Eye aria-hidden />
                      </button>
                      <button type="button" className="icon-btn" aria-label={`Remove ${m.title}`} onClick={() => remove(m)}>
                        <Trash2 aria-hidden />
                      </button>
                    </span>
                    <span className="mr-meta">
                      {m.sourceName}, {m.wordCount.toLocaleString()} words, {m.chunkCount} passages, added {formatDate(m.createdAt)}
                    </span>
                  </div>
                ))}
              </div>

              <section className="panel" style={{ marginTop: 26 }}>
                <div className="panel-head">
                  <h3>Test a search</h3>
                  <span className="field-hint">See which passages the assistant would read for a question</span>
                </div>
                <div className="panel-body">
                  <div className="search-test" style={{ margin: 0 }}>
                    <input
                      className="input"
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                      onKeyDown={(e) => e.key === "Enter" && search()}
                      placeholder="What does the syllabus say about late work?"
                      aria-label="Search query"
                    />
                    <button type="button" className="btn" onClick={search} disabled={!query.trim()}>
                      <Search aria-hidden /> Search
                    </button>
                  </div>
                  {hits && (
                    <div className="sources" style={{ paddingLeft: 0, marginTop: 12 }}>
                      {hits.length === 0 && <p className="field-hint">No passages matched. Try key terms from the material.</p>}
                      {hits.map((h, i) => (
                        <div className="source" key={i}>
                          <strong>
                            {h.title}, section {h.ordinal + 1}
                          </strong>
                          <span>
                            {h.snippet.split(/(«[^»]*»)/g).map((p, j) =>
                              p.startsWith("«") ? <mark key={j}>{p.slice(1, -1)}</mark> : <span key={j}>{p}</span>,
                            )}
                          </span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </section>
            </>
          )}
        </div>
      </div>
      {preview && (
        <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && setPreview(null)}>
          <div className="dialog" role="dialog" aria-modal="true" aria-label={preview.title} style={{ width: "min(760px, 100%)" }}>
            <div className="dialog-head">
              <h2>{preview.title}</h2>
              <button type="button" className="icon-btn" aria-label="Close" onClick={() => setPreview(null)}>
                <X aria-hidden />
              </button>
            </div>
            <div className="dialog-body" style={{ paddingBottom: 20 }}>
              <div className="material-preview">{preview.content}</div>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

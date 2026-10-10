import { useEffect, useRef, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { X } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import { useStore } from "../lib/store";
import { COURSE_COLORS, COURSE_LEVELS } from "../lib/kinds";
import type { CourseInput } from "../lib/types";

const EMPTY: CourseInput = { name: "", code: "", level: "undergraduate", discipline: "", description: "", color: "blue" };

export function CourseDialog() {
  const dialog = useStore((s) => s.courseDialog);
  const { openCourseDialog, refreshCourses, setCourse, toast } = useStore.getState();
  const [form, setForm] = useState<CourseInput>(EMPTY);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const nameRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!dialog) return;
    setError(null);
    setForm(dialog.mode === "edit" ? { ...dialog.course } : EMPTY);
    setTimeout(() => nameRef.current?.focus(), 0);
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && openCourseDialog(null);
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [dialog, openCourseDialog]);

  if (!dialog) return null;
  const editing = dialog.mode === "edit";
  const set = (patch: Partial<CourseInput>) => setForm((f) => ({ ...f, ...patch }));

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const course = editing ? await api.updateCourse(dialog.course.id, form) : await api.createCourse(form);
      await refreshCourses();
      if (!editing) setCourse(course.id);
      toast(editing ? "Course updated" : `Created ${course.name}`, "success");
      openCourseDialog(null);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!editing) return;
    const ok = await ask(
      `Delete ${dialog.course.name}? Its chats, course materials, decks, quiz history, and saved Library items are deleted too. This can't be undone.`,
      { title: "Delete course", kind: "warning", okLabel: "Delete course", cancelLabel: "Cancel" },
    );
    if (!ok) return;
    try {
      await api.deleteCourse(dialog.course.id);
      setCourse(null);
      await refreshCourses();
      openCourseDialog(null);
      toast("Course deleted", "success");
    } catch (err) {
      setError(errorMessage(err));
    }
  };

  return (
    <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && openCourseDialog(null)}>
      <form className="dialog" role="dialog" aria-modal="true" aria-labelledby="course-dialog-title" onSubmit={submit}>
        <div className="dialog-head">
          <h2 id="course-dialog-title">{editing ? "Edit course" : "New course"}</h2>
          <button type="button" className="icon-btn" aria-label="Close" onClick={() => openCourseDialog(null)}>
            <X aria-hidden />
          </button>
        </div>
        <div className="dialog-body">
          <p>The assistant adapts its explanations to the course level and can search the materials you add to the course.</p>
          <label className="field">
            <span>Course name</span>
            <input ref={nameRef} className="input" value={form.name} onChange={(e) => set({ name: e.target.value })} placeholder="Classical Mechanics" required maxLength={120} />
          </label>
          <div className="form-row">
            <label className="field">
              <span>Course code</span>
              <input className="input" value={form.code} onChange={(e) => set({ code: e.target.value })} placeholder="PHYS 201" maxLength={24} />
            </label>
            <label className="field">
              <span>Discipline</span>
              <input className="input" value={form.discipline} onChange={(e) => set({ discipline: e.target.value })} placeholder="Physics" maxLength={80} />
            </label>
          </div>
          <label className="field">
            <span>Level</span>
            <select className="select" value={form.level} onChange={(e) => set({ level: e.target.value })}>
              {COURSE_LEVELS.map((l) => (
                <option key={l.value} value={l.value}>
                  {l.label}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span>What the course covers</span>
            <textarea
              className="textarea"
              value={form.description}
              onChange={(e) => set({ description: e.target.value })}
              placeholder="Newtonian mechanics, energy and momentum, oscillations, and rigid-body rotation. Assumes single-variable calculus."
              maxLength={4000}
            />
            <span className="field-hint">Mention prerequisites and the topics students find hardest.</span>
          </label>
          <div className="field">
            <span className="field-label">Color</span>
            <div className="color-choices" role="group" aria-label="Course color">
              {Object.entries(COURSE_COLORS).map(([name, hex]) => (
                <button
                  key={name}
                  type="button"
                  aria-label={name}
                  aria-pressed={form.color === name}
                  style={{ ["--swatch" as string]: hex }}
                  onClick={() => set({ color: name })}
                />
              ))}
            </div>
          </div>
          {error && <p role="alert" style={{ color: "var(--danger)" }}>{error}</p>}
        </div>
        <div className="dialog-actions">
          {editing && (
            <button type="button" className="btn btn-ghost btn-danger left" onClick={remove}>
              Delete course
            </button>
          )}
          <button type="button" className="btn" onClick={() => openCourseDialog(null)}>
            Cancel
          </button>
          <button type="submit" className="btn btn-primary" disabled={busy || !form.name.trim()}>
            {editing ? "Save changes" : "Create course"}
          </button>
        </div>
      </form>
    </div>
  );
}

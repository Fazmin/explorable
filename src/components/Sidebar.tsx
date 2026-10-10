import { useEffect, useRef, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import {
  AArrowDown,
  AArrowUp,
  Accessibility,
  Check,
  ChevronsUpDown,
  FileText,
  GraduationCap,
  Layers,
  Library,
  MessagesSquare,
  Moon,
  Pencil,
  Plus,
  Presentation,
  Settings,
  SquarePen,
  Sun,
  TrendingUp,
  Trash2,
} from "lucide-react";
import { useStore } from "../lib/store";
import { COURSE_COLORS, levelLabel } from "../lib/kinds";
import { resolveTheme } from "../lib/theme";
import { scaleLabel, TEXT_SCALES } from "../lib/a11y";
import type { Settings as SettingsData, View } from "../lib/types";

export function Sidebar() {
  const mode = useStore((s) => s.mode);
  const view = useStore((s) => s.view);
  const courseId = useStore((s) => s.courseId);
  const dueCount = useStore((s) => s.dueCount);
  const conversations = useStore((s) => s.conversations);
  const activeId = useStore((s) => s.activeId);
  const convs = useStore((s) => s.convs);
  const { setMode, setView, newChat, openConversation, deleteConversation } = useStore.getState();

  const navItems: { view: View; label: string; icon: typeof Library; badge?: number; disabled?: boolean; title?: string }[] = [
    { view: "chat", label: "Chat", icon: MessagesSquare },
    { view: "library", label: "Library", icon: Library },
    ...(mode === "learn"
      ? [
          { view: "review" as View, label: "Review", icon: Layers, badge: dueCount },
          { view: "progress" as View, label: "Progress", icon: TrendingUp },
        ]
      : [{ view: "review" as View, label: "Decks", icon: Layers }]),
    {
      view: "materials",
      label: "Course materials",
      icon: FileText,
      disabled: !courseId,
      title: courseId ? undefined : "Choose a course to add materials",
    },
  ];

  const remove = async (id: string, title: string) => {
    const ok = await ask(`Delete “${title}”? This removes the chat and its answers. Saved Library items and flashcard decks stay.`, {
      title: "Delete chat",
      kind: "warning",
      okLabel: "Delete",
      cancelLabel: "Cancel",
    });
    if (ok) await deleteConversation(id);
  };

  return (
    <aside className="sidebar" aria-label="Navigation">
      <div className="brand">
        <img src="/favicon.svg" alt="" />
        <span>Explorable</span>
      </div>

      <div className="mode-switch" role="group" aria-label="Mode">
        <button type="button" aria-pressed={mode === "learn"} onClick={() => setMode("learn")}>
          <GraduationCap aria-hidden /> Learn
        </button>
        <button type="button" aria-pressed={mode === "teach"} onClick={() => setMode("teach")}>
          <Presentation aria-hidden /> Teach
        </button>
      </div>

      <CoursePicker />

      <div className="nav">
        <button type="button" className="btn btn-ghost new-chat" onClick={newChat}>
          <SquarePen aria-hidden /> New chat
        </button>
        {navItems.map((item) => {
          const Icon = item.icon;
          const current = view === item.view;
          return (
            <button
              key={item.view}
              type="button"
              className="nav-item"
              aria-current={current ? "page" : undefined}
              disabled={item.disabled}
              title={item.title}
              onClick={() => setView(item.view)}
            >
              <Icon aria-hidden />
              <span className="grow">
                <span className={current ? "label marker" : "label"}>{item.label}</span>
              </span>
              {item.badge ? <span className="badge" aria-label={`${item.badge} due`}>{item.badge}</span> : null}
            </button>
          );
        })}
      </div>

      <div className="history" aria-label="Recent chats">
        <div className="sidebar-section-title">{mode === "learn" ? "Study chats" : "Teaching chats"}</div>
        {conversations.length === 0 && <p className="history-empty">No chats here yet.</p>}
        {conversations.map((c) => (
          <div key={c.id} className="history-item" aria-current={c.id === activeId && view === "chat" ? "true" : undefined}>
            <button type="button" onClick={() => openConversation(c.id)} title={c.title}>
              {c.title}
            </button>
            {convs[c.id]?.running ? (
              <span className="running-dot" aria-label="Answering" />
            ) : (
              <button type="button" className="icon-btn" aria-label={`Delete ${c.title}`} onClick={() => remove(c.id, c.title)}>
                <Trash2 aria-hidden />
              </button>
            )}
          </div>
        ))}
      </div>

      <div className="sidebar-footer">
        <button
          type="button"
          className="nav-item"
          aria-current={view === "settings" ? "page" : undefined}
          onClick={() => setView("settings")}
        >
          <Settings aria-hidden />
          <span className={view === "settings" ? "label marker" : "label"}>Settings</span>
        </button>
        <A11yMenu />
        <ThemeButton />
      </div>
    </aside>
  );
}

/** One button: switches between light and dark ("Match system" lives in Settings). */
function ThemeButton() {
  const theme = useStore((s) => s.settings?.theme ?? "system");
  const setTheme = useStore((s) => s.setTheme);
  const [, rerender] = useState(0);
  useEffect(() => {
    // Follow OS changes while the preference is "system".
    const onChange = () => rerender((n) => n + 1);
    window.addEventListener("edu-theme", onChange);
    return () => window.removeEventListener("edu-theme", onChange);
  }, []);
  const dark = resolveTheme(theme) === "dark";
  const label = dark ? "Switch to light theme" : "Switch to dark theme";
  return (
    <button type="button" className="icon-btn footer-btn" aria-label={label} title={label} onClick={() => setTheme(dark ? "light" : "dark")}>
      {dark ? <Sun aria-hidden /> : <Moon aria-hidden />}
    </button>
  );
}

function nearestScale(scale: number) {
  return TEXT_SCALES.reduce((best, s) => (Math.abs(s - scale) < Math.abs(best - scale) ? s : best), TEXT_SCALES[0]);
}

/** Quick accessibility options next to the theme button. */
function A11yMenu() {
  const settings = useStore((s) => s.settings);
  const updatePrefs = useStore((s) => s.updatePrefs);
  const setView = useStore((s) => s.setView);
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

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

  if (!settings) return null;
  const index = TEXT_SCALES.indexOf(nearestScale(settings.textScale));
  const toggle = (key: keyof SettingsData) => (e: React.ChangeEvent<HTMLInputElement>) => updatePrefs({ [key]: e.target.checked } as never);

  return (
    <div className="a11y-wrap" ref={ref}>
      <button
        type="button"
        className="icon-btn footer-btn"
        aria-label="Accessibility options"
        title="Accessibility options"
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        <Accessibility aria-hidden />
      </button>
      {open && (
        <div className="popover" role="dialog" aria-label="Accessibility options">
          <h3>Accessibility</h3>
          <div className="pop-row">
            <span id="text-size-label">Text size</span>
            <div className="stepper" role="group" aria-labelledby="text-size-label">
              <button type="button" className="icon-btn" aria-label="Smaller text" disabled={index <= 0} onClick={() => updatePrefs({ textScale: TEXT_SCALES[index - 1] })}>
                <AArrowDown aria-hidden />
              </button>
              <output aria-live="polite">{scaleLabel(TEXT_SCALES[index])}</output>
              <button type="button" className="icon-btn" aria-label="Larger text" disabled={index >= TEXT_SCALES.length - 1} onClick={() => updatePrefs({ textScale: TEXT_SCALES[index + 1] })}>
                <AArrowUp aria-hidden />
              </button>
            </div>
          </div>
          <label className="pop-check">
            <input type="checkbox" checked={settings.highContrast} onChange={toggle("highContrast")} /> Increase contrast
          </label>
          <label className="pop-check">
            <input type="checkbox" checked={settings.reduceMotion} onChange={toggle("reduceMotion")} /> Reduce motion
          </label>
          <label className="pop-check">
            <input type="checkbox" checked={settings.relaxedSpacing} onChange={toggle("relaxedSpacing")} /> Relaxed spacing
          </label>
          <label className="pop-row">
            <span>Reading font</span>
            <select className="select" value={settings.readingFont} onChange={(e) => updatePrefs({ readingFont: e.target.value as SettingsData["readingFont"] })}>
              <option value="hyperlegible">Hyperlegible</option>
              <option value="serif">Serif</option>
              <option value="system">System</option>
            </select>
          </label>
          <button
            type="button"
            className="link-btn"
            onClick={() => {
              setOpen(false);
              setView("settings");
              setTimeout(() => document.getElementById("accessibility-settings")?.scrollIntoView({ block: "start" }), 60);
            }}
          >
            All accessibility settings
          </button>
        </div>
      )}
    </div>
  );
}

function CoursePicker() {
  const courses = useStore((s) => s.courses);
  const courseId = useStore((s) => s.courseId);
  const { setCourse, openCourseDialog } = useStore.getState();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const course = courses.find((c) => c.id === courseId) ?? null;

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const choose = (id: string | null) => {
    setCourse(id);
    setOpen(false);
  };

  return (
    <div className="course-picker" ref={ref}>
      <button type="button" className="course-button" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(!open)}>
        <span className="course-dot" style={{ ["--course-color" as string]: course ? COURSE_COLORS[course.color] : undefined }} />
        <span className="course-meta">
          <span className="course-name">{course ? course.name : "General"}</span>
          <span className="course-sub">
            {course ? [course.code, levelLabel(course.level)].filter(Boolean).join(", ") : "No course selected"}
          </span>
        </span>
        <ChevronsUpDown aria-hidden />
      </button>
      {open && (
        <div className="menu" role="menu">
          <button type="button" role="menuitemradio" aria-checked={!courseId} className="menu-item" onClick={() => choose(null)}>
            <span className="course-dot" />
            <span className="grow">General</span>
            {!courseId && <Check aria-hidden />}
          </button>
          {courses.map((c) => (
            <button
              key={c.id}
              type="button"
              role="menuitemradio"
              aria-checked={c.id === courseId}
              className="menu-item"
              onClick={() => choose(c.id)}
            >
              <span className="course-dot" style={{ ["--course-color" as string]: COURSE_COLORS[c.color] }} />
              <span className="grow">{c.code ? `${c.code}: ${c.name}` : c.name}</span>
              {c.id === courseId && <Check aria-hidden />}
            </button>
          ))}
          <div className="menu-sep" />
          <button
            type="button"
            role="menuitem"
            className="menu-item"
            onClick={() => {
              setOpen(false);
              openCourseDialog({ mode: "create" });
            }}
          >
            <Plus aria-hidden />
            <span className="grow">New course</span>
          </button>
          {course && (
            <button
              type="button"
              role="menuitem"
              className="menu-item"
              onClick={() => {
                setOpen(false);
                openCourseDialog({ mode: "edit", course });
              }}
            >
              <Pencil aria-hidden />
              <span className="grow">Edit {course.name}</span>
            </button>
          )}
        </div>
      )}
    </div>
  );
}

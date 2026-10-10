import { useEffect } from "react";
import { AlertTriangle } from "lucide-react";
import { useStore } from "./lib/store";
import { applyA11y } from "./lib/a11y";
import { Sidebar } from "./components/Sidebar";
import { CourseDialog } from "./components/CourseDialog";
import { Toasts } from "./components/Toasts";
import { Present } from "./components/Present";
import { ChatView } from "./chat/ChatView";
import { LibraryView } from "./views/LibraryView";
import { ReviewView } from "./views/ReviewView";
import { ProgressView } from "./views/ProgressView";
import { MaterialsView } from "./views/MaterialsView";
import { SettingsView } from "./views/SettingsView";

let booted = false;

export default function App() {
  const ready = useStore((s) => s.ready);
  const bootError = useStore((s) => s.bootError);
  const mode = useStore((s) => s.mode);
  const view = useStore((s) => s.view);
  const present = useStore((s) => s.present);

  useEffect(() => {
    if (booted) return;
    booted = true;
    void useStore.getState().boot();
  }, []);

  useEffect(() => {
    document.documentElement.dataset.mode = mode;
  }, [mode]);

  const settings = useStore((s) => s.settings);
  useEffect(() => {
    if (!settings) return;
    const { textScale, highContrast, reduceMotion, relaxedSpacing, readingFont } = settings;
    applyA11y({ textScale, highContrast, reduceMotion, relaxedSpacing, readingFont });
  }, [settings]);

  if (!ready) {
    return (
      <div className="boot" role="status">
        <div className="spinner" />
      </div>
    );
  }
  if (bootError) {
    return (
      <div className="boot" role="alert">
        <AlertTriangle aria-hidden />
        <p>Explorable couldn't start: {bootError}</p>
        <button className="btn" type="button" onClick={() => window.location.reload()}>
          Try again
        </button>
      </div>
    );
  }

  return (
    <div className="app" data-mode={mode}>
      <Sidebar />
      <main className="main" id="main">
        {view === "chat" && <ChatView />}
        {view === "library" && <LibraryView />}
        {view === "review" && <ReviewView />}
        {view === "progress" && <ProgressView />}
        {view === "materials" && <MaterialsView />}
        {view === "settings" && <SettingsView />}
      </main>
      <CourseDialog />
      <Toasts />
      {present && <Present item={present} />}
    </div>
  );
}

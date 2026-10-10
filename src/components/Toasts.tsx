import { AlertCircle, CheckCircle2, Info, X } from "lucide-react";
import { useStore } from "../lib/store";

export function Toasts() {
  const toasts = useStore((s) => s.toasts);
  const dismiss = useStore((s) => s.dismissToast);
  return (
    <div className="toasts" aria-live="polite">
      {toasts.map((t) => {
        const Icon = t.kind === "success" ? CheckCircle2 : t.kind === "error" ? AlertCircle : Info;
        return (
          <div key={t.id} className={`toast ${t.kind}`} role={t.kind === "error" ? "alert" : "status"}>
            <Icon aria-hidden />
            <span className="grow">{t.text}</span>
            <button type="button" className="icon-btn" aria-label="Dismiss" onClick={() => dismiss(t.id)} style={{ width: 22, height: 22 }}>
              <X aria-hidden />
            </button>
          </div>
        );
      })}
    </div>
  );
}

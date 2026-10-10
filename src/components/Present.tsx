import { useEffect, useRef } from "react";
import { Minimize2 } from "lucide-react";
import { useStore, type PresentItem } from "../lib/store";
import { ArtifactRenderer } from "../renderers/ArtifactRenderer";

/** Full-window view for projecting an item in class. Escape closes it. */
export function Present({ item }: { item: PresentItem }) {
  const setPresent = useStore((s) => s.setPresent);
  const mode = useStore((s) => s.mode);
  const closeRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    closeRef.current?.focus();
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setPresent(null);
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [setPresent]);

  return (
    <div className="present" role="dialog" aria-modal="true" aria-label={`Presenting ${item.title}`}>
      <div className="present-bar">
        <h2>{item.title}</h2>
        <span className="field-hint">Press Esc to exit</span>
        <button ref={closeRef} type="button" className="btn" onClick={() => setPresent(null)}>
          <Minimize2 aria-hidden /> Exit
        </button>
      </div>
      <div className="present-body">
        <ArtifactRenderer
          kind={item.kind}
          payload={item.payload}
          ctx={{ courseId: null, conversationId: null, key: `present:${item.key}`, surface: "present", mode }}
        />
      </div>
    </div>
  );
}

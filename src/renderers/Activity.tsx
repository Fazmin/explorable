import { useState } from "react";
import { BookOpen, ChevronRight, LineChart, Search } from "lucide-react";
import { useStore } from "../lib/store";

interface Hit {
  title: string;
  excerpt?: string;
  section?: number;
}

function parseJson(content: unknown): Record<string, unknown> | null {
  if (typeof content !== "string") return null;
  try {
    const v = JSON.parse(content);
    return v && typeof v === "object" ? v : null;
  } catch {
    return null;
  }
}

/** Renders «highlighted» excerpt markers as <mark>. */
function Excerpt({ text }: { text: string }) {
  const parts = text.split(/(«[^»]*»)/g);
  return (
    <span>
      {parts.map((p, i) => (p.startsWith("«") && p.endsWith("»") ? <mark key={i}>{p.slice(1, -1)}</mark> : <span key={i}>{p}</span>))}
    </span>
  );
}

export function SearchActivity({ input, result, streaming }: { input: Record<string, unknown>; result?: { content: unknown; isError: boolean }; streaming: boolean }) {
  const query = typeof input.query === "string" ? input.query : "";
  const data = parseJson(result?.content);
  const hits = (Array.isArray(data?.results) ? data!.results : []) as Hit[];
  if (streaming || !result) {
    return (
      <div className="activity">
        <span className="activity-row">
          <span className="spinner" /> Searching course materials{query ? ` for “${query}”` : ""}
        </span>
      </div>
    );
  }
  if (result.isError || hits.length === 0) {
    return (
      <div className="activity">
        <span className="activity-row">
          <Search aria-hidden /> Searched course materials for “{query}”: no matching passages
        </span>
      </div>
    );
  }
  return (
    <div className="activity">
      <details>
        <summary>
          <BookOpen aria-hidden />
          <span>
            Read {hits.length} {hits.length === 1 ? "passage" : "passages"} from course materials for “{query}”
          </span>
          <ChevronRight aria-hidden style={{ width: 14, height: 14 }} />
        </summary>
        <div className="sources">
          {hits.map((h, i) => (
            <div className="source" key={i}>
              <strong>
                {h.title}
                {h.section ? `, section ${h.section}` : ""}
              </strong>
              {h.excerpt && <Excerpt text={h.excerpt} />}
            </div>
          ))}
        </div>
      </details>
    </div>
  );
}

export function ProgressActivity({ streaming }: { streaming: boolean }) {
  return (
    <div className="activity">
      <span className="activity-row">
        {streaming ? <span className="spinner" /> : <LineChart aria-hidden />}
        {streaming ? "Checking your practice record" : "Checked your practice record"}
      </span>
    </div>
  );
}

export function PlanCard({ input }: { input: Record<string, unknown> }) {
  const elements = Array.isArray(input.key_elements) ? (input.key_elements as string[]) : [];
  return (
    <div className="plan-card">
      <strong>Plan{typeof input.technology === "string" ? `: ${input.technology}` : ""}</strong>
      {typeof input.approach === "string" && <p>{input.approach}</p>}
      {elements.length > 0 && (
        <ul>
          {elements.map((e, i) => (
            <li key={i}>{e}</li>
          ))}
        </ul>
      )}
    </div>
  );
}

interface ChoiceInput {
  question?: string;
  options?: { label?: string; description?: string }[];
  allow_other?: boolean;
}

export function ChoicePrompt({ toolId, input, pending, answer, streaming }: { toolId: string; input: ChoiceInput; pending: boolean; answer?: string; streaming: boolean }) {
  const [other, setOther] = useState("");
  const [sent, setSent] = useState<string | null>(null);
  const answerChoice = useStore((s) => s.answerChoice);
  const options = (input.options ?? []).filter((o) => o?.label);
  const chosen = answer ?? sent;
  const active = pending && !chosen && !streaming;

  const pick = async (value: string) => {
    if (!active || !value.trim()) return;
    setSent(value.trim());
    // Re-enable the options if the answer was rejected.
    if (!(await answerChoice(toolId, value.trim()))) setSent(null);
  };

  return (
    <div className="choice" role="group" aria-label={input.question ?? "Question"}>
      <div className="choice-q">{input.question}</div>
      <div className="choice-options">
        {options.map((o, i) => (
          <button key={i} type="button" className="choice-option" aria-pressed={chosen === o.label} disabled={!active} onClick={() => pick(o.label!)}>
            <strong>{o.label}</strong>
            {o.description && <span>{o.description}</span>}
          </button>
        ))}
      </div>
      {input.allow_other && active && (
        <div className="choice-other">
          <label className="visually-hidden" htmlFor={`other-${toolId}`}>
            Your own answer
          </label>
          <input id={`other-${toolId}`} className="input" value={other} onChange={(e) => setOther(e.target.value)} placeholder="Or type your own answer" maxLength={2000} onKeyDown={(e) => e.key === "Enter" && pick(other)} />
          <button type="button" className="btn" onClick={() => pick(other)} disabled={!other.trim()}>
            Send
          </button>
        </div>
      )}
      {chosen && !options.some((o) => o.label === chosen) && <p className="choice-done">You answered: {chosen}</p>}
      {!pending && !chosen && !streaming && <p className="choice-done">This question was skipped.</p>}
    </div>
  );
}

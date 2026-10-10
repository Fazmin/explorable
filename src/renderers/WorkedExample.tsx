import { useState } from "react";
import { Eye, ListOrdered, Send } from "lucide-react";
import { useStore } from "../lib/store";
import { errorMessage } from "../lib/api";
import { Figure, type FigureContext } from "../components/Figure";
import { Markdown } from "../components/Markdown";

export interface WorkedExampleInput {
  title?: string;
  problem?: string;
  steps?: { title?: string; detail?: string }[];
  answer?: string;
  practice_problem?: { prompt?: string; answer?: string };
}

export function WorkedExample({ input, streaming, ctx }: { input: WorkedExampleInput; streaming: boolean; ctx: FigureContext }) {
  const steps = (input.steps ?? []).filter((s) => s && (s.title || s.detail));
  // Instructors see the full solution; learners reveal it step by step.
  const [shown, setShown] = useState(ctx.mode === "teach" || ctx.surface === "present" ? Infinity : 0);
  const [practiceOpen, setPracticeOpen] = useState(false);
  const [attempt, setAttempt] = useState("");
  const toast = useStore((s) => s.toast);
  const title = input.title?.trim() || "Worked example";
  const visible = streaming ? steps.length : Math.min(shown, steps.length);
  const allShown = visible >= steps.length;

  const check = async () => {
    if (!ctx.onPrompt || !attempt.trim() || !input.practice_problem?.prompt) return;
    try {
      await ctx.onPrompt(`For the practice problem “${input.practice_problem.prompt}”, my answer is: ${attempt.trim()}\nIs it right? If not, show me where I went wrong without just giving the answer.`);
      setAttempt("");
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  };

  return (
    <Figure kind="worked_example" title={title} payload={input as Record<string, unknown>} ctx={ctx} streaming={streaming}>
      <div className="problem">
        <div className="problem-label">Problem</div>
        <Markdown text={input.problem ?? ""} />
      </div>
      <ol className="steps">
        {steps.map((s, i) => {
          const open = i < visible;
          return (
            <li key={i} className={open ? "step" : "step locked"}>
              <div className="step-title">{s.title}</div>
              {open ? <Markdown text={s.detail ?? ""} /> : <p className="field-hint">Try this step yourself, then reveal it.</p>}
            </li>
          );
        })}
      </ol>
      {!streaming && !allShown && (
        <div className="step-actions">
          <button type="button" className="btn btn-primary" onClick={() => setShown(visible + 1)}>
            <Eye aria-hidden /> Reveal step {visible + 1}
          </button>
          <button type="button" className="btn btn-ghost" onClick={() => setShown(Infinity)}>
            <ListOrdered aria-hidden /> Show the full solution
          </button>
        </div>
      )}
      {!streaming && allShown && input.answer && (
        <div className="final-answer">
          <div className="final-answer-label">Answer</div>
          <Markdown text={input.answer} />
        </div>
      )}
      {!streaming && input.practice_problem?.prompt && (
        <div className="practice">
          <h4>Try one yourself</h4>
          <Markdown text={input.practice_problem.prompt} />
          {ctx.onPrompt && ctx.mode === "learn" && (
            <div className="choice-other">
              <label className="visually-hidden" htmlFor={`practice-${ctx.key}`}>
                Your answer to the practice problem
              </label>
              <input
                id={`practice-${ctx.key}`}
                className="input"
                value={attempt}
                onChange={(e) => setAttempt(e.target.value)}
                placeholder="Your answer"
                onKeyDown={(e) => e.key === "Enter" && check()}
              />
              <button type="button" className="btn" onClick={check} disabled={!attempt.trim()}>
                <Send aria-hidden /> Check with tutor
              </button>
            </div>
          )}
          <button type="button" className="btn btn-ghost btn-sm" style={{ marginTop: 8 }} onClick={() => setPracticeOpen(!practiceOpen)} aria-expanded={practiceOpen}>
            {practiceOpen ? "Hide answer" : "Show answer"}
          </button>
          {practiceOpen && <Markdown text={input.practice_problem.answer ?? ""} />}
        </div>
      )}
    </Figure>
  );
}

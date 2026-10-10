import { Figure, type FigureContext } from "../components/Figure";
import { Markdown } from "../components/Markdown";

export interface LessonPlanInput {
  title?: string;
  level?: string;
  duration_minutes?: number;
  overview?: string;
  objectives?: { text?: string; bloom?: string }[];
  prerequisites?: string[];
  materials?: string[];
  sections?: { title?: string; minutes?: number; kind?: string; description?: string; instructor_notes?: string }[];
  assessment?: string;
  differentiation?: string;
  follow_up?: string;
}

// Section kinds use the categorical palette in fixed order (see tokens.css).
const KIND_SLOT: Record<string, number> = {
  "warm-up": 4,
  lecture: 1,
  discussion: 3,
  activity: 2,
  demonstration: 7,
  assessment: 8,
  "wrap-up": 6,
  break: 0,
};

const kindColor = (kind?: string) => {
  const slot = KIND_SLOT[kind ?? ""];
  return slot ? `var(--cat-${slot})` : "var(--rule-strong)";
};

export function LessonPlan({ input, streaming, ctx }: { input: LessonPlanInput; streaming: boolean; ctx: FigureContext }) {
  const sections = (input.sections ?? []).filter((s) => s && s.title);
  const total = sections.reduce((sum, s) => sum + (Number.isFinite(s.minutes) ? Number(s.minutes) : 0), 0);
  const duration = Number(input.duration_minutes) || total;
  const mismatch = !streaming && duration > 0 && Math.abs(total - duration) > 0.5;
  let clock = 0;

  return (
    <Figure kind="lesson_plan" title={input.title?.trim() || "Lesson plan"} payload={input as Record<string, unknown>} ctx={ctx} streaming={streaming} wide>
      <div className="lesson-facts">
        {input.level && (
          <span>
            Level: <strong>{input.level}</strong>
          </span>
        )}
        <span>
          Duration: <strong>{duration} min</strong>
        </span>
        <span>
          Sections: <strong>{sections.length}</strong>
        </span>
        {mismatch && <span className="mismatch">Sections add up to {total} min, not {duration}.</span>}
      </div>
      {input.overview && <Markdown text={input.overview} />}

      {(input.objectives?.length ?? 0) > 0 && (
        <>
          <h4 className="lesson-section-title">By the end, students will be able to</h4>
          <ul className="objectives">
            {input.objectives!.filter((o) => o?.text).map((o, i) => (
              <li key={i}>
                {o.text}
                {o.bloom && <span className="bloom-tag">{o.bloom}</span>}
              </li>
            ))}
          </ul>
        </>
      )}

      {sections.length > 0 && (
        <>
          <h4 className="lesson-section-title">Session outline</h4>
          <div className="timeline-bar" aria-hidden>
            {sections.map((s, i) => (
              <span key={i} style={{ flexGrow: Math.max(Number(s.minutes) || 1, 1), background: kindColor(s.kind) }} title={`${s.title}: ${s.minutes} min`} />
            ))}
          </div>
          <div className="timeline">
            {sections.map((s, i) => {
              const start = clock;
              clock += Number(s.minutes) || 0;
              return (
                <div className="tl-row" key={i}>
                  <div className="tl-time">
                    <strong>{s.minutes ?? "?"} min</strong>
                    {start}–{clock}
                  </div>
                  <div>
                    {s.kind && (
                      <span className="tl-kind">
                        <i style={{ background: kindColor(s.kind) }} />
                        {s.kind}
                      </span>
                    )}
                    <div className="tl-title">{s.title}</div>
                    {s.description && <Markdown text={s.description} />}
                    {s.instructor_notes && <div className="tl-notes">{s.instructor_notes}</div>}
                  </div>
                </div>
              );
            })}
          </div>
        </>
      )}

      {((input.prerequisites?.length ?? 0) > 0 || (input.materials?.length ?? 0) > 0) && (
        <div className="lesson-columns">
          {(input.prerequisites?.length ?? 0) > 0 && (
            <div>
              <h4 className="lesson-section-title">Prerequisites</h4>
              <ul>{input.prerequisites!.map((p, i) => <li key={i}>{p}</li>)}</ul>
            </div>
          )}
          {(input.materials?.length ?? 0) > 0 && (
            <div>
              <h4 className="lesson-section-title">Materials</h4>
              <ul>{input.materials!.map((m, i) => <li key={i}>{m}</li>)}</ul>
            </div>
          )}
        </div>
      )}

      {(
        [
          ["Assessment", input.assessment],
          ["Differentiation", input.differentiation],
          ["Follow-up", input.follow_up],
        ] as const
      ).map(([label, text]) =>
        text ? (
          <div key={label}>
            <h4 className="lesson-section-title">{label}</h4>
            <Markdown text={text} />
          </div>
        ) : null,
      )}
    </Figure>
  );
}

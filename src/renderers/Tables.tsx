import { Figure, type FigureContext } from "../components/Figure";

export interface RubricInput {
  title?: string;
  assignment?: string;
  levels?: { label?: string; points?: number }[];
  criteria?: { name?: string; weight_percent?: number; descriptors?: string[] }[];
  notes?: string;
}

export function Rubric({ input, streaming, ctx }: { input: RubricInput; streaming: boolean; ctx: FigureContext }) {
  const levels = (input.levels ?? []).filter((l) => l && l.label);
  const criteria = (input.criteria ?? []).filter((c) => c && c.name);
  const weights = criteria.map((c) => c.weight_percent).filter((w): w is number => typeof w === "number");
  const weightTotal = weights.reduce((a, b) => a + b, 0);
  const weightsOff = !streaming && weights.length === criteria.length && criteria.length > 0 && Math.abs(weightTotal - 100) > 0.5;

  return (
    <Figure kind="rubric" title={input.title?.trim() || "Rubric"} payload={input as Record<string, unknown>} ctx={ctx} streaming={streaming} wide>
      {input.assignment && <p className="field-hint" style={{ marginBottom: 10 }}>Assignment: {input.assignment}</p>}
      <div className="table-scroll" tabIndex={0} role="region" aria-label="Rubric table">
        <table className="rubric-table">
          <thead>
            <tr>
              <th scope="col">Criterion</th>
              {levels.map((l, i) => (
                <th key={i} scope="col">
                  {l.label}
                  {typeof l.points === "number" && <span className="pts">{l.points} pts</span>}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {criteria.map((c, i) => (
              <tr key={i}>
                <th scope="row" style={{ position: "static", background: "transparent", borderBottom: "1px solid var(--rule)" }}>
                  {c.name}
                  {typeof c.weight_percent === "number" && <span className="weight">{c.weight_percent}%</span>}
                </th>
                {levels.map((_, j) => (
                  <td key={j}>{c.descriptors?.[j] ?? ""}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {(input.notes || weightsOff) && (
        <p className="source-line">
          {weightsOff && <span className="mismatch">Weights add up to {weightTotal}%, not 100%. </span>}
          {input.notes}
        </p>
      )}
    </Figure>
  );
}

export interface TableInput {
  title?: string;
  columns?: string[];
  rows?: unknown[][];
  source?: string;
}

const NUMERIC = /^[-+−]?[$€£]?\d[\d,]*(\.\d+)?\s?(%|[kKMB])?$/;

export function DataTable({ input, streaming, ctx }: { input: TableInput; streaming: boolean; ctx: FigureContext }) {
  const columns = input.columns ?? [];
  const rows = (input.rows ?? []).filter(Array.isArray);
  const text = (v: unknown) => (v === null || v === undefined ? "" : String(v));
  const numericCol = columns.map((_, j) => rows.length > 0 && rows.every((r) => text(r[j]).trim() === "" || NUMERIC.test(text(r[j]).trim())));

  return (
    <Figure kind="table" title={input.title?.trim() || "Table"} payload={input as Record<string, unknown>} ctx={ctx} streaming={streaming} wide>
      <div className="table-scroll" tabIndex={0} role="region" aria-label={`${input.title ?? "Table"} (scrollable)`}>
        <table className="data-table">
          <thead>
            <tr>
              {columns.map((c, j) => (
                <th key={j} scope="col" style={numericCol[j] ? { textAlign: "right" } : undefined}>
                  {c}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr key={i}>
                {columns.map((_, j) => (
                  <td key={j} className={numericCol[j] ? "num" : undefined}>
                    {text(r[j])}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {input.source && <p className="source-line">Source: {input.source}</p>}
    </Figure>
  );
}

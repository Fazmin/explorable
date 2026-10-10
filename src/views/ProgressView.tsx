import { useEffect, useMemo, useRef, useState } from "react";
import { Sparkles, TrendingUp } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import { useStore } from "../lib/store";
import { formatDate } from "../lib/exporting";
import type { Progress } from "../lib/types";

export function ProgressView() {
  const courseId = useStore((s) => s.courseId);
  const courses = useStore((s) => s.courses);
  const { toast, newChat, send } = useStore.getState();
  const [data, setData] = useState<Progress | null>(null);
  const course = courses.find((c) => c.id === courseId);

  useEffect(() => {
    let alive = true;
    api
      .getProgress(courseId)
      .then((p) => alive && setData(p))
      .catch((err) => toast(errorMessage(err), "error"));
    return () => {
      alive = false;
    };
  }, [courseId, toast]);

  const askNext = async () => {
    newChat();
    await send("Based on my quiz results and flashcard reviews so far, what should I study next, and why?");
  };

  const empty = data && data.quizzesTaken === 0 && data.cardsTotal === 0;
  return (
    <>
      <header className="topbar">
        <h1>Progress</h1>
        <span className="crumb">{course ? course.name : "All courses and General"}</span>
      </header>
      <div className="view-scroll">
        <div className="view-pad">
          <div className="view-header">
            <div>
              <h2>Your practice</h2>
              <p>Built from the quizzes you submit and the flashcards you review in this app.</p>
            </div>
            <button type="button" className="btn" onClick={askNext} disabled={!!empty}>
              <Sparkles aria-hidden /> Ask what to study next
            </button>
          </div>
          {!data ? (
            <div className="spinner" role="status" aria-label="Loading" />
          ) : empty ? (
            <div className="empty">
              <TrendingUp aria-hidden />
              <h3>No practice recorded yet</h3>
              <p>Take a quiz or review flashcards and your progress shows up here, grouped by topic.</p>
            </div>
          ) : (
            <>
              <div className="stat-tiles">
                <Stat label="Day streak" value={String(data.streakDays)} note={data.streakDays === 1 ? "day with practice" : "days in a row with practice"} />
                <Stat label="Cards due now" value={String(data.cardsDue)} note={`${data.cardsLearned} of ${data.cardsTotal} cards started`} />
                <Stat label="Quizzes taken" value={String(data.quizzesTaken)} note={`${data.reviewsLast7} card reviews in the last 7 days`} />
                <Stat
                  label="Average quiz score"
                  value={data.averageScore === null ? "–" : `${Math.round(data.averageScore * 100)}%`}
                  note="across all submitted attempts"
                />
              </div>
              <ActivityChart activity={data.activity} />
              <Mastery topics={data.topics} />
              {data.recentAttempts.length > 0 && (
                <section className="panel">
                  <div className="panel-head">
                    <h3>Recent quizzes</h3>
                  </div>
                  <div className="panel-body table-scroll">
                    <table className="data-table">
                      <thead>
                        <tr>
                          <th scope="col">Quiz</th>
                          <th scope="col">Topic</th>
                          <th scope="col" style={{ textAlign: "right" }}>Score</th>
                          <th scope="col">When</th>
                        </tr>
                      </thead>
                      <tbody>
                        {data.recentAttempts.map((a) => (
                          <tr key={a.id}>
                            <td>{a.title}</td>
                            <td>{a.topic}</td>
                            <td className="num">
                              {a.score} / {a.maxScore}
                            </td>
                            <td>{formatDate(a.createdAt)}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </section>
              )}
            </>
          )}
        </div>
      </div>
    </>
  );
}

function Stat({ label, value, note }: { label: string; value: string; note: string }) {
  return (
    <div className="stat-tile">
      <div className="stat-label">{label}</div>
      <div className="stat-value">{value}</div>
      <div className="stat-note">{note}</div>
    </div>
  );
}

function niceMax(v: number): number {
  if (v <= 4) return 4;
  const pow = 10 ** Math.floor(Math.log10(v));
  for (const m of [1, 2, 2.5, 5, 10]) if (m * pow >= v) return m * pow;
  return 10 * pow;
}

/** Rounded top, square baseline. */
function columnPath(x: number, y: number, w: number, h: number, r: number) {
  const rr = Math.min(r, w / 2, h);
  return `M${x},${y + h}V${y + rr}Q${x},${y} ${x + rr},${y}H${x + w - rr}Q${x + w},${y} ${x + w},${y + rr}V${y + h}Z`;
}

function ActivityChart({ activity }: { activity: Progress["activity"] }) {
  const [hover, setHover] = useState<number | null>(null);
  const [asTable, setAsTable] = useState(false);
  const boxRef = useRef<HTMLDivElement>(null);
  const [W, setW] = useState(720);
  const H = 240;

  // Draw in real pixels so marks and labels keep their specified sizes.
  useEffect(() => {
    const el = boxRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setW(Math.max(320, Math.round(el.clientWidth))));
    ro.observe(el);
    return () => ro.disconnect();
  }, [asTable]);
  const left = 34;
  const right = 6;
  const top = 18;
  const bottom = 26;
  const plotH = H - top - bottom;
  const band = (W - left - right) / Math.max(activity.length, 1);
  const barW = Math.min(18, band * 0.6);
  const totals = activity.map((d) => d.reviews + d.questions);
  const max = niceMax(Math.max(1, ...totals));
  const peak = totals.indexOf(Math.max(...totals));
  const y = (v: number) => top + plotH - (v / max) * plotH;
  const label = (iso: string) => new Date(`${iso}T12:00:00`).toLocaleDateString(undefined, { month: "short", day: "numeric" });
  const GAP = 2;

  const columns = useMemo(
    () =>
      activity.map((d, i) => {
        const x = left + i * band + (band - barW) / 2;
        const rH = (d.reviews / max) * plotH;
        const qH = (d.questions / max) * plotH;
        return { d, i, x, rH, qH };
      }),
    [activity, band, barW, max, plotH],
  );

  return (
    <section className="panel">
      <div className="panel-head">
        <h3>Daily activity, last 30 days</h3>
        <div className="legend" aria-hidden={asTable}>
          <span>
            <i style={{ background: "var(--series-1)" }} />
            Flashcard reviews
          </span>
          <span>
            <i style={{ background: "var(--series-2)" }} />
            Quiz questions answered
          </span>
        </div>
        <button type="button" className="btn btn-sm btn-ghost table-toggle" onClick={() => setAsTable(!asTable)} aria-pressed={asTable}>
          {asTable ? "Show chart" : "Show as table"}
        </button>
      </div>
      <div className="panel-body">
        {asTable ? (
          <div className="table-scroll" style={{ maxHeight: 320 }}>
            <table className="data-table">
              <thead>
                <tr>
                  <th scope="col">Date</th>
                  <th scope="col" style={{ textAlign: "right" }}>Flashcard reviews</th>
                  <th scope="col" style={{ textAlign: "right" }}>Quiz questions</th>
                </tr>
              </thead>
              <tbody>
                {[...activity].reverse().map((d) => (
                  <tr key={d.date}>
                    <td>{label(d.date)}</td>
                    <td className="num">{d.reviews}</td>
                    <td className="num">{d.questions}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="chart" ref={boxRef} onMouseLeave={() => setHover(null)}>
            <svg width={W} height={H} viewBox={`0 0 ${W} ${H}`} role="img" aria-label="Stacked columns of flashcard reviews and quiz questions per day for the last 30 days. Use Show as table for exact values.">
              {[0, max / 2, max].map((v) => (
                <g key={v}>
                  <line className="grid-line" x1={left} x2={W - right} y1={y(v)} y2={y(v)} />
                  <text className="axis-label" x={left - 8} y={y(v) + 4} textAnchor="end">
                    {Number.isInteger(v) ? v : v.toFixed(1)}
                  </text>
                </g>
              ))}
              {columns.map(({ d, i, x, rH, qH }) => {
                const base = top + plotH;
                const hasBoth = rH > 0 && qH > 0;
                const qTop = base - rH - qH;
                return (
                  <g key={d.date}>
                    {hover === i && <rect className="col-hover" x={left + i * band} y={top} width={band} height={plotH} />}
                    {rH > 0 && (
                      <path d={qH > 0 ? `M${x},${base}V${base - rH}H${x + barW}V${base}Z` : columnPath(x, base - rH, barW, rH, 4)} fill="var(--series-1)" />
                    )}
                    {qH > 0 && (
                      <path d={columnPath(x, qTop, barW, Math.max(qH - (hasBoth ? GAP : 0), 1), 4)} fill="var(--series-2)" />
                    )}
                    {i % 7 === activity.length % 7 - 1 || i === activity.length - 1 ? (
                      <text className="axis-label" x={x + barW / 2} y={H - 6} textAnchor="middle">
                        {i === activity.length - 1 ? "Today" : label(d.date)}
                      </text>
                    ) : null}
                    {i === peak && totals[i] > 0 && (
                      <text className="axis-label" x={x + barW / 2} y={qTop - 6} textAnchor="middle" style={{ fill: "var(--ink-2)", fontWeight: 600 }}>
                        {totals[i]}
                      </text>
                    )}
                    <rect
                      className="hit"
                      x={left + i * band}
                      y={top}
                      width={band}
                      height={plotH}
                      tabIndex={0}
                      aria-label={`${label(d.date)}: ${d.reviews} reviews, ${d.questions} quiz questions`}
                      onMouseEnter={() => setHover(i)}
                      onFocus={() => setHover(i)}
                      onBlur={() => setHover(null)}
                    />
                  </g>
                );
              })}
            </svg>
            {hover !== null && activity[hover] && (
              <div
                className="tooltip"
                style={{ left: left + hover * band + band / 2, top: y(totals[hover]) }}
              >
                <div className="tt-date">{label(activity[hover].date)}</div>
                <div className="tt-row">
                  <span className="tt-key" style={{ background: "var(--series-1)" }} />
                  <b>{activity[hover].reviews}</b> flashcard reviews
                </div>
                <div className="tt-row">
                  <span className="tt-key" style={{ background: "var(--series-2)" }} />
                  <b>{activity[hover].questions}</b> quiz questions
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </section>
  );
}

function Mastery({ topics }: { topics: Progress["topics"] }) {
  if (topics.length === 0) return null;
  return (
    <section className="panel">
      <div className="panel-head">
        <h3>Quiz accuracy by topic</h3>
        <span className="field-hint">Share of questions answered correctly across every attempt</span>
      </div>
      <div className="panel-body mastery">
        {topics.map((t) => {
          const pct = Math.round(t.accuracy * 100);
          return (
            <div className="mastery-row" key={t.topic}>
              <span className="m-topic" title={t.topic}>
                {t.topic}
              </span>
              <span className="m-track" role="meter" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct} aria-label={`${t.topic} accuracy`}>
                <span className="m-fill" style={{ width: `${pct}%`, display: "block" }} />
              </span>
              <span className="m-val">
                <strong>{pct}%</strong>, {t.attempts} {t.attempts === 1 ? "attempt" : "attempts"}
              </span>
            </div>
          );
        })}
      </div>
    </section>
  );
}

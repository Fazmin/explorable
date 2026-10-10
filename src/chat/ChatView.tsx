import { memo, useEffect, useMemo, useRef } from "react";
import { AlertCircle, ChevronRight, Info, KeyRound, RotateCcw, Route, TriangleAlert } from "lucide-react";
import { useStore, selectActive, type ConvState } from "../lib/store";
import { PRESENTATION_LABEL, levelLabel } from "../lib/kinds";
import type { Block, Mode, StoredMessage } from "../lib/types";
import { Markdown } from "../components/Markdown";
import type { FigureContext } from "../components/Figure";
import { ToolBlock, type ToolResult } from "../renderers/ToolBlock";
import { Composer } from "./Composer";
import { ReadAloud } from "../components/ReadAloud";
import { STARTERS } from "./starters";

interface Turn {
  user: StoredMessage | null;
  items: StoredMessage[];
}

const hasText = (m: StoredMessage) => m.content.some((b) => b.type === "text");
const EMPTY: unknown[] = [];

function buildTurns(messages: StoredMessage[]): Turn[] {
  const turns: Turn[] = [];
  for (const m of messages) {
    if (m.role === "user" && hasText(m)) turns.push({ user: m, items: [] });
    else {
      if (turns.length === 0) turns.push({ user: null, items: [] });
      turns[turns.length - 1].items.push(m);
    }
  }
  return turns;
}

function toolResults(messages: StoredMessage[], pendingResults: unknown[]) {
  const results: Record<string, ToolResult> = {};
  const answers: Record<string, string> = {};
  // Results for tools called alongside an open question are held until it's answered.
  for (const r of pendingResults as Block[]) {
    if (r?.type === "tool_result" && r.tool_use_id) results[r.tool_use_id] = { content: r.content, isError: !!r.is_error };
  }
  for (const m of messages) {
    if (m.role !== "user") continue;
    for (const b of m.content) {
      if (b.type !== "tool_result" || !b.tool_use_id) continue;
      results[b.tool_use_id] = { content: b.content, isError: !!b.is_error };
      const text = typeof b.content === "string" ? b.content : "";
      if (typeof m.meta.choiceAnswer === "string") answers[b.tool_use_id] = m.meta.choiceAnswer;
      else if (text.startsWith("The user chose: ")) answers[b.tool_use_id] = text.slice("The user chose: ".length);
    }
  }
  return { results, answers };
}

export function ChatView() {
  const activeId = useStore((s) => s.activeId);
  const active = useStore(selectActive);
  const conv = useStore((s) => (s.activeId ? s.convs[s.activeId] : undefined));
  const settings = useStore((s) => s.settings);
  const courses = useStore((s) => s.courses);
  const courseId = useStore((s) => s.courseId);
  const mode = useStore((s) => s.mode);
  const setView = useStore((s) => s.setView);
  const course = courses.find((c) => c.id === (active?.courseId ?? courseId)) ?? null;
  const isDemo = settings?.provider === "demo";
  const showWelcome = !activeId || (conv && conv.messages.length === 0 && !conv.running);

  return (
    <div className="chat">
      <header className="topbar">
        <h1>{active?.title ?? "New chat"}</h1>
        <span className="crumb">{course ? course.name : "General"}</span>
        <span className="spacer" />
      </header>
      {isDemo && (
        <div className="demo-banner" role="note">
          <Info aria-hidden />
          <span>Demo mode: answers are scripted samples, not live AI.</span>
          <button type="button" className="btn btn-sm" onClick={() => setView("settings")}>
            <KeyRound aria-hidden /> Add an API key
          </button>
        </div>
      )}
      {showWelcome ? (
        <div className="chat-scroll">
          <Welcome mode={mode} courseName={course?.name ?? null} courseLevel={course ? levelLabel(course.level) : null} />
        </div>
      ) : (
        conv && activeId && <Conversation id={activeId} conv={conv} courseId={active?.courseId ?? null} mode={active?.mode ?? mode} />
      )}
      <Composer />
    </div>
  );
}

function Welcome({ mode, courseName, courseLevel }: { mode: Mode; courseName: string | null; courseLevel: string | null }) {
  const send = useStore((s) => s.send);
  return (
    <div className="welcome">
      <div className="welcome-head">
        <h2>{mode === "learn" ? "What are you studying?" : "What are you teaching?"}</h2>
        <p className="welcome-sub">
          {courseName
            ? `Answers are pitched at ${courseLevel?.toLowerCase()} level for ${courseName}, and can draw on the materials you add to this course.`
            : mode === "learn"
              ? "Ask a question, explore an idea with an interactive model, or practise with a quiz. Choose a course in the sidebar to ground answers in your own materials."
              : "Plan a session, write an assessment, or build a demo to project in class. Choose a course in the sidebar so materials match its level."}
        </p>
      </div>
      <div className="starter-groups">
        {STARTERS[mode].map((group) => {
          const Icon = group.icon;
          return (
            <section key={group.label} className="starter-group" aria-label={group.label}>
              <h3>
                <Icon aria-hidden /> {group.label}
              </h3>
              {group.starters.map((s) => (
                <button key={s.title} type="button" className="starter" onClick={() => send(s.prompt)}>
                  <span className="starter-title">{s.title}</span>
                  <span className="starter-desc">{s.description}</span>
                </button>
              ))}
            </section>
          );
        })}
      </div>
    </div>
  );
}

function Conversation({ id, conv, courseId, mode }: { id: string; conv: ConvState; courseId: string | null; mode: Mode }) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const { retry } = useStore.getState();
  const turns = useMemo(() => buildTurns(conv.messages), [conv.messages]);
  // Tool results live in user messages, which never change once added.
  const userSignature = conv.messages.filter((m) => m.role === "user").map((m) => m.id).join(",");
  const pendingResults = conv.pending?.otherResults ?? EMPTY;
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const { results, answers } = useMemo(() => toolResults(conv.messages, pendingResults), [userSignature, pendingResults]);
  const lastUserId = turns.length ? turns[turns.length - 1].user?.id : undefined;

  const ctx = useMemo<FigureContext>(
    () => ({
      courseId,
      conversationId: id,
      key: id,
      surface: "chat",
      mode,
      onPrompt: async (text: string) => {
        const state = useStore.getState();
        if (state.convs[id]?.running) throw new Error("Wait for the current answer to finish, then try again.");
        if (state.activeId !== id) await state.openConversation(id);
        if (!(await state.send(text))) {
          throw new Error(useStore.getState().convs[id]?.error?.message ?? "The message couldn't be sent.");
        }
      },
    }),
    [courseId, id, mode],
  );

  // Scroll only when a new question is asked or a chat opens. Streaming text
  // and growing interactives never take the scroll position from the reader.
  useEffect(() => {
    const root = scrollRef.current;
    if (!root || !lastUserId) return;
    const el = root.querySelector<HTMLElement>(`[data-message-id="${lastUserId}"]`);
    el?.scrollIntoView({ block: "start" });
  }, [lastUserId]);

  const lastTurn = turns.length - 1;
  return (
    <div className="chat-scroll" ref={scrollRef}>
      <div className="chat-column">
        {turns.map((t, i) => (
          <TurnView
            key={t.user?.id ?? `t${i}`}
            turn={t}
            results={results}
            answers={answers}
            ctx={ctx}
            pendingId={conv.pending?.toolUseId ?? null}
            thinking={conv.running && i === lastTurn}
          />
        ))}
        {conv.notices.map((n, i) => (
          <p key={i} className={`notice ${n.level}`} role="status">
            {n.level === "warning" ? <TriangleAlert aria-hidden /> : <Info aria-hidden />}
            {n.text}
          </p>
        ))}
        {conv.error && !conv.running && (
          <div className="chat-error" role="alert">
            <AlertCircle aria-hidden />
            <span className="grow">{conv.error.message}</span>
            {conv.error.retryable && (
              <button type="button" className="btn btn-sm" onClick={() => retry()}>
                <RotateCcw aria-hidden /> Retry
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

interface TurnProps {
  turn: Turn;
  results: Record<string, ToolResult>;
  answers: Record<string, string>;
  ctx: FigureContext;
  pendingId: string | null;
  thinking: boolean;
}

const sameTurn = (a: TurnProps, b: TurnProps) =>
  a.turn.user === b.turn.user &&
  a.turn.items.length === b.turn.items.length &&
  a.turn.items.every((m, i) => m === b.turn.items[i]) &&
  a.results === b.results &&
  a.answers === b.answers &&
  a.ctx === b.ctx &&
  a.pendingId === b.pendingId &&
  a.thinking === b.thinking;

const TurnView = memo(function TurnView({ turn, results, answers, ctx, pendingId, thinking }: TurnProps) {
  const route = turn.user?.meta.route;
  const userText = turn.user?.content.filter((b) => b.type === "text").map((b) => b.text ?? "").join("\n");
  const assistant = turn.items.filter((m) => m.role === "assistant");
  // Show a status line whenever the run is active but nothing is streaming.
  const streamingNow = assistant.some((m) => m.content.some((b) => b._done === false));
  const showThinking = thinking && !streamingNow;
  const answerText = assistant
    .flatMap((m) => m.content)
    .filter((b) => b.type === "text")
    .map((b) => b.text ?? "")
    .join("\n\n");

  return (
    <article className="turn">
      {turn.user && (
        <div className="user-msg" data-message-id={turn.user.id}>
          {userText}
        </div>
      )}
      <div className="answer">
        {route && route.presentation !== "text" && <RouteChip presentation={route.presentation} reason={route.reason} source={route.source} />}
        {assistant.map((m) => (
          <AssistantBlocks key={m.id} message={m} results={results} answers={answers} ctx={ctx} pendingId={pendingId} />
        ))}
        {showThinking && (
          <span className="thinking-line" role="status">
            <span className="spinner" /> {route ? "Thinking" : "Choosing how to answer"}
          </span>
        )}
        {!thinking && <ReadAloud text={answerText} />}
      </div>
    </article>
  );
}, sameTurn);

function RouteChip({ presentation, reason, source }: { presentation: string; reason: string; source: string }) {
  const label = PRESENTATION_LABEL[presentation] ?? presentation;
  const by = source === "router" ? "Format chosen by the router" : source === "demo" ? "Format chosen by demo keyword rules" : "Format chosen by the tutor";
  return (
    <span className="route-chip" title={reason ? `${by}: ${reason}` : by}>
      <Route aria-hidden />
      {label}
    </span>
  );
}

const AssistantBlocks = memo(function AssistantBlocks({
  message,
  results,
  answers,
  ctx,
  pendingId,
}: {
  message: StoredMessage;
  results: Record<string, ToolResult>;
  answers: Record<string, string>;
  ctx: FigureContext;
  pendingId: string | null;
}) {
  return (
    <>
      {message.content.map((b: Block, i) => {
        const key = b.id ?? `${message.id}:${i}`;
        if (b.type === "text") return b.text ? <Markdown key={key} text={b.text} streaming={b._done === false} /> : null;
        if (b.type === "thinking") return b.thinking ? <Reasoning key={key} text={b.thinking} /> : null;
        if (b.type === "tool_use")
          return (
            <ToolBlock
              key={key}
              block={b}
              result={b.id ? results[b.id] : undefined}
              ctx={ctx}
              pendingChoice={!!b.id && b.id === pendingId}
              choiceAnswer={b.id ? answers[b.id] : undefined}
            />
          );
        return null;
      })}
    </>
  );
});

function Reasoning({ text }: { text: string }) {
  return (
    <details className="reasoning">
      <summary>
        <ChevronRight aria-hidden /> Reasoning
      </summary>
      <div className="reasoning-body">{text}</div>
    </details>
  );
}

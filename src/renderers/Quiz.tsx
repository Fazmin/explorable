import { useEffect, useMemo, useState } from "react";
import { CheckCircle2, Eye, EyeOff, MessageCircleQuestion, RotateCcw, XCircle } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import { useStore } from "../lib/store";
import { Markdown } from "../components/Markdown";
import { Figure, type FigureContext } from "../components/Figure";
import { answered, correctText, isCorrect, LETTERS, type Answer, type Question, type QuizInput } from "../lib/grading";

export function Quiz({ input, streaming, ctx }: { input: QuizInput; streaming: boolean; ctx: FigureContext }) {
  const questions = useMemo(() => (input.questions ?? []).filter((q) => q && typeof q === "object"), [input.questions]);
  const title = input.title?.trim() || "Quiz";
  const isAssessment = input.purpose === "assessment" || ctx.mode === "teach";
  const [answers, setAnswers] = useState<Answer[]>([]);
  const [submitted, setSubmitted] = useState(false);
  const [showKey, setShowKey] = useState(false);
  const [attempts, setAttempts] = useState<number | null>(null);
  const [best, setBest] = useState<number | null>(null);
  const toast = useStore((s) => s.toast);

  useEffect(() => {
    if (streaming || ctx.surface !== "chat") return;
    api
      .quizAttempts(ctx.key)
      .then((list) => {
        setAttempts(list.length);
        setBest(list.length ? Math.max(...list.map((a) => a.score)) : null);
      })
      .catch(() => undefined);
  }, [streaming, ctx.key, ctx.surface]);

  const setAnswer = (i: number, a: Answer) => {
    if (submitted) return;
    setAnswers((prev) => {
      const next = prev.slice();
      next[i] = a;
      return next;
    });
  };

  const results = questions.map((q, i) => isCorrect(q, answers[i]));
  const score = results.filter(Boolean).length;
  const allAnswered = questions.length > 0 && questions.every((q, i) => answered(q, answers[i]));

  const submit = async () => {
    setSubmitted(true);
    setShowKey(false);
    try {
      await api.recordQuizAttempt({
        courseId: ctx.courseId,
        conversationId: ctx.conversationId,
        quizKey: ctx.key,
        title,
        topic: input.topic?.trim() || title,
        score,
        maxScore: questions.length,
        details: questions.map((q, i) => ({ question: i + 1, correct: results[i], bloom: q.bloom ?? null })),
      });
      setAttempts((n) => (n ?? 0) + 1);
      setBest((b) => (b === null ? score : Math.max(b, score)));
    } catch (err) {
      toast(`Your score couldn't be saved: ${errorMessage(err)}`, "error");
    }
  };

  const retry = () => {
    setAnswers([]);
    setSubmitted(false);
  };

  const discuss = async () => {
    if (!ctx.onPrompt) return;
    const missed = questions
      .map((q, i) => ({ q, i }))
      .filter(({ i }) => !results[i])
      .map(({ q, i }) => `Q${i + 1} (“${(q.prompt ?? "").slice(0, 90)}”)`);
    const text = missed.length
      ? `I scored ${score}/${questions.length} on the quiz “${title}”. I missed ${missed.join(", ")}. Help me understand what I got wrong, one question at a time.`
      : `I scored ${score}/${questions.length} on the quiz “${title}”. What should I study next to go deeper?`;
    try {
      await ctx.onPrompt(text);
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  };

  const revealed = submitted || showKey;
  const status = streaming ? (
    <span className="figure-status" role="status">
      <span className="spinner" /> Writing question {Math.max(1, questions.length)}
    </span>
  ) : null;

  return (
    <Figure kind="quiz" title={title} payload={input as Record<string, unknown>} ctx={ctx} streaming={streaming} status={status} wide>
      <div className="quiz">
        <div className="quiz-meta">
          {input.topic && <span>Topic: {input.topic}</span>}
          <span>
            {questions.length} {questions.length === 1 ? "question" : "questions"}
          </span>
          {attempts ? (
            <span>
              {attempts} saved {attempts === 1 ? "attempt" : "attempts"}
              {best !== null ? `, best ${best}/${questions.length}` : ""}
            </span>
          ) : null}
        </div>
        {questions.map((q, i) => (
          <QuestionView
            key={i}
            index={i}
            q={q}
            answer={answers[i]}
            onAnswer={(a) => setAnswer(i, a)}
            revealed={revealed}
            submitted={submitted}
            correct={results[i]}
            disabled={streaming || submitted}
            showBloom={isAssessment}
          />
        ))}
        {!streaming && questions.length > 0 && (
          <div className="quiz-actions">
            {submitted ? (
              <>
                <span className="score" role="status">
                  Score: <span className="marker">{score} of {questions.length}</span>
                </span>
                <span className="grow" />
                <button type="button" className="btn" onClick={retry}>
                  <RotateCcw aria-hidden /> Try again
                </button>
                {ctx.onPrompt && (
                  <button type="button" className="btn btn-primary" onClick={discuss}>
                    <MessageCircleQuestion aria-hidden /> Discuss my results
                  </button>
                )}
              </>
            ) : (
              <>
                <button type="button" className="btn btn-primary" onClick={submit} disabled={!allAnswered}>
                  Check answers
                </button>
                {!allAnswered && <span className="field-hint">Answer every question to check.</span>}
                <span className="grow" />
                {isAssessment && (
                  <button type="button" className="btn btn-ghost" onClick={() => setShowKey(!showKey)} aria-pressed={showKey}>
                    {showKey ? <EyeOff aria-hidden /> : <Eye aria-hidden />} {showKey ? "Hide answer key" : "Show answer key"}
                  </button>
                )}
              </>
            )}
          </div>
        )}
      </div>
    </Figure>
  );
}

interface QuestionProps {
  index: number;
  q: Question;
  answer: Answer;
  onAnswer: (a: Answer) => void;
  revealed: boolean;
  submitted: boolean;
  correct: boolean;
  disabled: boolean;
  showBloom: boolean;
}

function QuestionView({ index, q, answer, onAnswer, revealed, submitted, correct, disabled, showBloom }: QuestionProps) {
  const id = `q-${index}`;
  const options: { label: string; value: number | boolean; feedback?: string }[] =
    q.type === "true_false"
      ? [
          { label: "True", value: true },
          { label: "False", value: false },
        ]
      : (q.options ?? []).map((o, i) => ({ label: o, value: i, feedback: q.option_feedback?.[i] }));
  const isCorrectOption = (v: number | boolean) => (q.type === "true_false" ? v === q.answer_bool : v === q.answer_index);

  return (
    <div className="question" role="group" aria-labelledby={`${id}-prompt`}>
      <div className="question-head">
        <span className="question-num">{index + 1}.</span>
        <div className="question-prompt" id={`${id}-prompt`}>
          <Markdown text={q.prompt ?? ""} />
        </div>
        {showBloom && q.bloom && <span className="bloom-tag">{q.bloom}</span>}
      </div>

      {(q.type === "multiple_choice" || q.type === "true_false") && (
        <div className="options">
          {options.map((o, i) => {
            const selected = answer === o.value;
            const cls = revealed ? (isCorrectOption(o.value) ? " correct" : selected && submitted ? " incorrect" : "") : "";
            return (
              <button
                key={i}
                type="button"
                className={`option${cls}`}
                aria-pressed={selected}
                disabled={disabled}
                onClick={() => onAnswer(o.value)}
              >
                <span className="opt-letter">{q.type === "true_false" ? (o.value ? "T" : "F") : LETTERS[i]}</span>
                <span className="opt-text">
                  <Markdown text={o.label} className="prose inline" />
                  {submitted && o.feedback && (selected || isCorrectOption(o.value)) && <span className="option-feedback">{o.feedback}</span>}
                </span>
              </button>
            );
          })}
        </div>
      )}

      {(q.type === "numeric" || q.type === "short_answer") && (
        <div className="answer-input">
          <label className="visually-hidden" htmlFor={`${id}-input`}>
            Your answer to question {index + 1}
          </label>
          <input
            id={`${id}-input`}
            className="input"
            inputMode={q.type === "numeric" ? "decimal" : "text"}
            value={typeof answer === "string" ? answer : ""}
            onChange={(e) => onAnswer(e.target.value)}
            disabled={disabled}
            placeholder={q.type === "numeric" ? "Enter a number" : "Type your answer"}
            aria-invalid={submitted && !correct ? true : undefined}
          />
          {q.unit && <span className="unit">{q.unit}</span>}
        </div>
      )}

      {revealed && !submitted && <div className="answer-key">Answer: {correctText(q)}</div>}

      {submitted && (
        <div className={`feedback ${correct ? "right" : "wrong"}`}>
          {correct ? <CheckCircle2 aria-label="Correct" /> : <XCircle aria-label="Incorrect" />}
          <div>
            {!correct && (q.type === "numeric" || q.type === "short_answer") && (
              <p>
                <strong>Answer:</strong> {correctText(q)}
              </p>
            )}
            {q.explanation && <Markdown text={q.explanation} />}
          </div>
        </div>
      )}
    </div>
  );
}

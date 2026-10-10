// Local grading for quiz answers.

export interface Question {
  type?: "multiple_choice" | "true_false" | "numeric" | "short_answer";
  prompt?: string;
  options?: string[];
  answer_index?: number;
  option_feedback?: string[];
  answer_bool?: boolean;
  answer_number?: number;
  tolerance?: number;
  unit?: string;
  accepted_answers?: string[];
  explanation?: string;
  bloom?: string;
}

export interface QuizInput {
  title?: string;
  topic?: string;
  purpose?: "practice" | "assessment";
  questions?: Question[];
}

export type Answer = number | boolean | string | undefined;
export const LETTERS = "ABCDEFGH";

export function normalizeAnswer(s: string): string {
  return s
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[.,;:!?'"`()[\]{}]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/^(the|a|an) /, "");
}

export function parseNumber(s: string): number | null {
  const cleaned = s.trim().replace(/\s/g, "").replace(/,(?=\d{3}(\D|$))/g, "");
  const normalized = /^-?\d+,\d+$/.test(cleaned) ? cleaned.replace(",", ".") : cleaned;
  if (!/^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i.test(normalized)) return null;
  const n = Number(normalized);
  return Number.isFinite(n) ? n : null;
}

export function isCorrect(q: Question, a: Answer): boolean {
  switch (q.type) {
    case "multiple_choice":
      return typeof a === "number" && a === q.answer_index;
    case "true_false":
      return typeof a === "boolean" && a === q.answer_bool;
    case "numeric": {
      if (typeof a !== "string" || typeof q.answer_number !== "number") return false;
      const n = parseNumber(a);
      if (n === null) return false;
      const tol = typeof q.tolerance === "number" && q.tolerance > 0 ? q.tolerance : Math.max(Math.abs(q.answer_number) * 1e-9, 1e-12);
      return Math.abs(n - q.answer_number) <= tol;
    }
    case "short_answer": {
      if (typeof a !== "string" || !a.trim()) return false;
      const given = normalizeAnswer(a);
      return (q.accepted_answers ?? []).some((acc) => normalizeAnswer(acc) === given);
    }
    default:
      return false;
  }
}

export function answered(q: Question, a: Answer) {
  if (q.type === "numeric" || q.type === "short_answer") return typeof a === "string" && a.trim() !== "";
  return a !== undefined;
}

export function correctText(q: Question): string {
  switch (q.type) {
    case "multiple_choice":
      return q.answer_index !== undefined ? `${LETTERS[q.answer_index] ?? "?"}. ${q.options?.[q.answer_index] ?? ""}` : "";
    case "true_false":
      return q.answer_bool ? "True" : "False";
    case "numeric":
      return `${q.answer_number}${q.unit ? ` ${q.unit}` : ""}${q.tolerance ? ` (± ${q.tolerance})` : ""}`;
    default:
      return (q.accepted_answers ?? []).join(" / ");
  }
}


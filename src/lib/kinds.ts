import {
  BookOpenCheck,
  ClipboardList,
  Layers,
  ListChecks,
  NotebookPen,
  Sparkles,
  Table2,
  Footprints,
  type LucideIcon,
} from "lucide-react";
import type { ArtifactKind } from "./types";

export const TOOL_KIND: Record<string, ArtifactKind> = {
  generate_interactive: "interactive",
  render_quiz: "quiz",
  create_flashcards: "flashcards",
  render_worked_example: "worked_example",
  render_lesson_plan: "lesson_plan",
  render_rubric: "rubric",
  render_table: "table",
};

export const KIND_LABEL: Record<ArtifactKind, string> = {
  interactive: "Interactive",
  quiz: "Quiz",
  flashcards: "Flashcards",
  worked_example: "Worked example",
  lesson_plan: "Lesson plan",
  rubric: "Rubric",
  table: "Table",
  note: "Note",
};

export const KIND_ICON: Record<ArtifactKind, LucideIcon> = {
  interactive: Sparkles,
  quiz: ListChecks,
  flashcards: Layers,
  worked_example: Footprints,
  lesson_plan: ClipboardList,
  rubric: BookOpenCheck,
  table: Table2,
  note: NotebookPen,
};

export interface ExportOption {
  format: string;
  label: string;
}

export const EXPORTS: Record<ArtifactKind, ExportOption[]> = {
  interactive: [{ format: "html", label: "Standalone HTML file" }],
  quiz: [
    { format: "markdown", label: "Markdown, questions only" },
    { format: "markdown_key", label: "Markdown with answer key" },
    { format: "gift", label: "Moodle GIFT" },
  ],
  flashcards: [{ format: "tsv", label: "Anki (tab-separated)" }],
  worked_example: [{ format: "markdown", label: "Markdown" }],
  lesson_plan: [{ format: "markdown", label: "Markdown" }],
  rubric: [
    { format: "markdown", label: "Markdown" },
    { format: "csv", label: "CSV spreadsheet" },
  ],
  table: [
    { format: "csv", label: "CSV spreadsheet" },
    { format: "markdown", label: "Markdown" },
  ],
  note: [],
};

export const PRESENTATION_LABEL: Record<string, string> = {
  text: "Text answer",
  table: "Table",
  chart: "Chart",
  diagram: "Diagram",
  interactive_explainer: "Interactive explainer",
  simulation: "Simulation",
  calculator: "Calculator",
  worked_example: "Worked example",
  quiz: "Quiz",
  flashcards: "Flashcards",
  lesson_plan: "Lesson plan",
  rubric: "Rubric",
  auto: "Tutor's choice",
};

export function artifactTitle(payload: Record<string, unknown> | undefined, fallback: string): string {
  const t = payload?.title ?? payload?.deck_title;
  return typeof t === "string" && t.trim() ? t.trim() : fallback;
}

export const COURSE_COLORS: Record<string, string> = {
  blue: "#2d4fd6",
  teal: "#0f7b6c",
  green: "#3b7d23",
  amber: "#b7791f",
  orange: "#d3602a",
  rose: "#c2416b",
  violet: "#6b4fd6",
  slate: "#5b6478",
};

export const COURSE_LEVELS: { value: string; label: string }[] = [
  { value: "introductory", label: "Introductory undergraduate" },
  { value: "undergraduate", label: "Undergraduate" },
  { value: "advanced", label: "Advanced undergraduate" },
  { value: "graduate", label: "Graduate" },
  { value: "professional", label: "Professional / continuing" },
];

export function levelLabel(level: string) {
  return COURSE_LEVELS.find((l) => l.value === level)?.label ?? "Undergraduate";
}

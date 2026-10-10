import { memo } from "react";
import { AlertTriangle } from "lucide-react";
import type { Block } from "../lib/types";
import type { FigureContext } from "../components/Figure";
import { InteractiveFrame, type InteractiveInput } from "./InteractiveFrame";
import { Quiz } from "./Quiz";
import { Flashcards, type FlashcardsInput } from "./Flashcards";
import { WorkedExample, type WorkedExampleInput } from "./WorkedExample";
import { LessonPlan, type LessonPlanInput } from "./LessonPlan";
import { DataTable, Rubric, type RubricInput, type TableInput } from "./Tables";
import { ChoicePrompt, PlanCard, ProgressActivity, SearchActivity } from "./Activity";
import type { QuizInput } from "../lib/grading";

export interface ToolResult {
  content: unknown;
  isError: boolean;
}

interface Props {
  block: Block;
  result?: ToolResult;
  ctx: FigureContext;
  pendingChoice: boolean;
  choiceAnswer?: string;
}

const RENDER_TOOLS = new Set([
  "generate_interactive",
  "render_quiz",
  "create_flashcards",
  "render_worked_example",
  "render_lesson_plan",
  "render_rubric",
  "render_table",
]);

function deckIdFrom(result?: ToolResult): string | null {
  if (!result || result.isError || typeof result.content !== "string") return null;
  try {
    const v = JSON.parse(result.content);
    return typeof v.deck_id === "string" ? v.deck_id : null;
  } catch {
    return null;
  }
}

/** Renders one tool call: a component while it streams, then the finished version. */
export const ToolBlock = memo(function ToolBlock({ block, result, ctx, pendingChoice, choiceAnswer }: Props) {
  const streaming = block._done === false;
  const input = (streaming ? block._partial : block.input) ?? {};
  const name = block.name ?? "";
  const toolCtx: FigureContext = { ...ctx, key: block.id ?? ctx.key };

  // A render call the backend rejected: the model sees the error and retries,
  // so show a short note instead of a half-valid component.
  if (RENDER_TOOLS.has(name) && result?.isError) {
    return (
      <p className="notice">
        <AlertTriangle aria-hidden /> A component didn't pass validation, so the tutor is correcting it.
      </p>
    );
  }

  switch (name) {
    case "generate_interactive":
      return (
        <InteractiveFrame input={input as InteractiveInput} complete={!streaming} completeKeys={block._completeKeys ?? []} ctx={toolCtx} />
      );
    case "render_quiz":
      return <Quiz input={input as QuizInput} streaming={streaming} ctx={toolCtx} />;
    case "create_flashcards":
      return <Flashcards input={input as FlashcardsInput} streaming={streaming || !result} ctx={toolCtx} deckId={deckIdFrom(result)} />;
    case "render_worked_example":
      return <WorkedExample input={input as WorkedExampleInput} streaming={streaming} ctx={toolCtx} />;
    case "render_lesson_plan":
      return <LessonPlan input={input as LessonPlanInput} streaming={streaming} ctx={toolCtx} />;
    case "render_rubric":
      return <Rubric input={input as RubricInput} streaming={streaming} ctx={toolCtx} />;
    case "render_table":
      return <DataTable input={input as TableInput} streaming={streaming} ctx={toolCtx} />;
    case "search_course_materials":
      return <SearchActivity input={input} result={result} streaming={streaming} />;
    case "get_learner_progress":
      return <ProgressActivity streaming={streaming || !result} />;
    case "plan_visualization":
      return streaming ? null : <PlanCard input={input} />;
    case "ask_choice":
      return <ChoicePrompt toolId={block.id ?? ""} input={input} pending={pendingChoice} answer={choiceAnswer} streaming={streaming} />;
    default:
      return null;
  }
});

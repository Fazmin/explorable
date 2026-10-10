import type { ArtifactKind } from "../lib/types";
import type { FigureContext } from "../components/Figure";
import type { QuizInput } from "../lib/grading";
import { InteractiveFrame, type InteractiveInput } from "./InteractiveFrame";
import { Quiz } from "./Quiz";
import { Flashcards, type FlashcardsInput } from "./Flashcards";
import { WorkedExample, type WorkedExampleInput } from "./WorkedExample";
import { LessonPlan, type LessonPlanInput } from "./LessonPlan";
import { DataTable, Rubric, type RubricInput, type TableInput } from "./Tables";

/** Renders a saved item outside a chat: in the Library or the Present view. */
export function ArtifactRenderer({ kind, payload, ctx }: { kind: ArtifactKind; payload: Record<string, unknown>; ctx: FigureContext }) {
  switch (kind) {
    case "interactive":
      return <InteractiveFrame input={payload as InteractiveInput} complete completeKeys={[]} ctx={ctx} />;
    case "quiz":
      return <Quiz input={payload as QuizInput} streaming={false} ctx={ctx} />;
    case "flashcards":
      return <Flashcards input={payload as FlashcardsInput} streaming={false} ctx={ctx} />;
    case "worked_example":
      return <WorkedExample input={payload as WorkedExampleInput} streaming={false} ctx={ctx} />;
    case "lesson_plan":
      return <LessonPlan input={payload as LessonPlanInput} streaming={false} ctx={ctx} />;
    case "rubric":
      return <Rubric input={payload as RubricInput} streaming={false} ctx={ctx} />;
    case "table":
      return <DataTable input={payload as TableInput} streaming={false} ctx={ctx} />;
    default:
      return <p className="field-hint">This item type can't be displayed.</p>;
  }
}

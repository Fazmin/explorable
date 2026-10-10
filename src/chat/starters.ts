import { ClipboardList, Footprints, Lightbulb, ListChecks, Presentation, type LucideIcon } from "lucide-react";
import type { Mode } from "../lib/types";

export interface Starter {
  title: string;
  description: string;
  prompt: string;
}

export interface StarterGroup {
  label: string;
  icon: LucideIcon;
  starters: Starter[];
}

export const STARTERS: Record<Mode, StarterGroup[]> = {
  learn: [
    {
      label: "Understand",
      icon: Lightbulb,
      starters: [
        {
          title: "Build a wave from sines",
          description: "Watch a Fourier series approach a square wave as you add harmonics.",
          prompt:
            "Visualize how a Fourier series builds a square wave from sine waves. Let me change the number of harmonics and compare square, sawtooth, and triangle waves, and explain the overshoot near the jumps.",
        },
        {
          title: "See the central limit theorem",
          description: "Simulate sample means from a skewed population and compare them with the normal curve.",
          prompt:
            "Show me the central limit theorem with a simulation: let me pick a population shape and sample size, draw many samples, and compare the histogram of sample means with the normal curve the theorem predicts.",
        },
      ],
    },
    {
      label: "Practice",
      icon: ListChecks,
      starters: [
        {
          title: "Quiz me on cellular respiration",
          description: "Mixed questions with feedback on every option. Your score goes to Progress.",
          prompt: "Quiz me on cellular respiration with five mixed questions at an intro biology level, including one that makes me reason about an experiment.",
        },
        {
          title: "Flashcards for functional groups",
          description: "A deck for organic chemistry you can review with spaced repetition.",
          prompt: "Make flashcards for the main organic chemistry functional groups: structure, an example compound, and one key property each.",
        },
      ],
    },
    {
      label: "Work through",
      icon: Footprints,
      starters: [
        {
          title: "Solve ∫ x·eˣ dx step by step",
          description: "Integration by parts, one step at a time, with a practice problem.",
          prompt: "Solve the integral of x·e^x step by step using integration by parts. Let me try each step before seeing it, and give me a similar practice problem.",
        },
        {
          title: "Compare study strategies",
          description: "Which techniques the research supports, and how to use them this week.",
          prompt: "Compare evidence-based study strategies in a table: what each involves, how well the research supports it, and how I could try it this week.",
        },
      ],
    },
  ],
  teach: [
    {
      label: "Plan",
      icon: ClipboardList,
      starters: [
        {
          title: "A 50-minute lesson on Bayesian inference",
          description: "Measurable objectives, timed active learning, and an exit ticket.",
          prompt:
            "Plan a 50-minute lesson introducing Bayesian inference to upper-level undergraduates, built around the base-rate fallacy, with active learning and an exit ticket.",
        },
        {
          title: "A rubric for a research paper",
          description: "Weighted criteria with observable descriptors at four levels.",
          prompt: "Create a rubric for an 8–10 page argumentative research paper with four performance levels and weighted criteria.",
        },
      ],
    },
    {
      label: "Assess",
      icon: ListChecks,
      starters: [
        {
          title: "A quiz on cellular respiration",
          description: "Mixed item types with an answer key and Bloom levels. Exports to Moodle.",
          prompt:
            "Write an assessment quiz on cellular respiration for an intro biology course: five mixed item types tagged with Bloom levels, with an answer key and explanations.",
        },
        {
          title: "A review deck for students",
          description: "Flashcards on organic functional groups, exportable to Anki.",
          prompt: "Make flashcards my students can use to review the main organic chemistry functional groups.",
        },
      ],
    },
    {
      label: "Demonstrate",
      icon: Presentation,
      starters: [
        {
          title: "Classroom demo: central limit theorem",
          description: "A simulation to project with Present.",
          prompt:
            "Build a classroom demo simulation of the central limit theorem that I can project: large readable controls for population shape and sample size, and buttons to draw 1, 100, or 1,000 samples.",
        },
        {
          title: "Classroom demo: Fourier series",
          description: "Build a square wave from sines, live in front of the class.",
          prompt: "Build an interactive classroom demo that visualizes a Fourier series building a square wave, with a harmonics slider I can operate live.",
        },
      ],
    },
  ],
};

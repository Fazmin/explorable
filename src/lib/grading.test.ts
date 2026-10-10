import { describe, expect, it } from "vitest";
import { isCorrect, normalizeAnswer, parseNumber } from "./grading";

describe("grading", () => {
  it("normalizes short answers", () => {
    expect(normalizeAnswer("  The Mitochondria! ")).toBe("mitochondria");
    expect(normalizeAnswer("Café")).toBe("cafe");
  });

  it("parses numbers in common formats", () => {
    expect(parseNumber("3.14")).toBe(3.14);
    expect(parseNumber("1,000")).toBe(1000);
    expect(parseNumber("2,5")).toBe(2.5);
    expect(parseNumber("6.02e23")).toBe(6.02e23);
    expect(parseNumber("abc")).toBeNull();
    expect(parseNumber("")).toBeNull();
  });

  it("grades each question type", () => {
    expect(isCorrect({ type: "multiple_choice", answer_index: 2 }, 2)).toBe(true);
    expect(isCorrect({ type: "multiple_choice", answer_index: 2 }, 1)).toBe(false);
    expect(isCorrect({ type: "true_false", answer_bool: false }, false)).toBe(true);
    expect(isCorrect({ type: "numeric", answer_number: 9.81, tolerance: 0.05 }, "9.8")).toBe(true);
    expect(isCorrect({ type: "numeric", answer_number: 9.81, tolerance: 0.005 }, "9.8")).toBe(false);
    expect(isCorrect({ type: "numeric", answer_number: 2 }, "2")).toBe(true);
    expect(isCorrect({ type: "short_answer", accepted_answers: ["oxygen", "O2"] }, " o2 ")).toBe(true);
    expect(isCorrect({ type: "short_answer", accepted_answers: ["oxygen"] }, "nitrogen")).toBe(false);
  });
});

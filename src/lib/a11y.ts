// Accessibility preferences: applied as attributes on <html> (styles key off
// them), mirrored into sandboxed frames, and remembered locally so the first
// paint already uses them.
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { Settings } from "./types";

export type A11yPrefs = Pick<Settings, "textScale" | "highContrast" | "reduceMotion" | "relaxedSpacing" | "readingFont">;

/** Attributes copied into generated UIs; values are fixed vocabularies. */
export interface FrameA11y {
  contrast: "more" | "normal";
  reduceMotion: "true" | "false";
  spacing: "relaxed" | "normal";
  font: "hyperlegible" | "serif" | "system";
}

const KEY = "edu.a11y";
export const TEXT_SCALES = [1, 1.15, 1.3, 1.5];

export function frameA11y(): FrameA11y {
  const d = document.documentElement.dataset;
  return {
    contrast: d.contrast === "more" ? "more" : "normal",
    reduceMotion: d.reduceMotion === "true" ? "true" : "false",
    spacing: d.spacing === "relaxed" ? "relaxed" : "normal",
    font: d.font === "serif" || d.font === "system" ? d.font : "hyperlegible",
  };
}

let appliedZoom = 1;

/** Zooms the whole webview (generated UIs included); falls back to CSS zoom outside the app shell. */
async function applyZoom(scale: number) {
  if (scale === appliedZoom) return;
  appliedZoom = scale;
  try {
    await getCurrentWebview().setZoom(scale);
    document.documentElement.style.removeProperty("zoom");
  } catch {
    document.documentElement.style.setProperty("zoom", String(scale));
  }
}

export function applyA11y(p: A11yPrefs) {
  const d = document.documentElement.dataset;
  const before = JSON.stringify(frameA11y());
  d.contrast = p.highContrast ? "more" : "normal";
  d.reduceMotion = p.reduceMotion ? "true" : "false";
  d.spacing = p.relaxedSpacing ? "relaxed" : "normal";
  d.font = p.readingFont;
  try {
    localStorage.setItem(KEY, JSON.stringify(p));
  } catch {
    // Only affects the first paint of the next launch.
  }
  void applyZoom(p.textScale);
  const after = frameA11y();
  if (JSON.stringify(after) !== before) window.dispatchEvent(new CustomEvent<FrameA11y>("edu-a11y", { detail: after }));
}

export function scaleLabel(scale: number) {
  return `${Math.round(scale * 100)}%`;
}

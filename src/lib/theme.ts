import type { Theme } from "./types";

export type Resolved = "light" | "dark";
const KEY = "edu.theme";
const media = window.matchMedia("(prefers-color-scheme: dark)");
let preference: Theme = "system";

export function resolveTheme(t: Theme): Resolved {
  if (t === "system") return media.matches ? "dark" : "light";
  return t;
}

export function currentTheme(): Resolved {
  return document.documentElement.dataset.theme === "dark" ? "dark" : "light";
}

/** Applies a theme to the app and notifies sandboxed frames. */
export function applyTheme(t: Theme) {
  preference = t;
  const resolved = resolveTheme(t);
  const changed = document.documentElement.dataset.theme !== resolved;
  document.documentElement.dataset.theme = resolved;
  try {
    localStorage.setItem(KEY, t);
  } catch {
    // Storage can be unavailable; the theme still applies for this session.
  }
  if (changed) window.dispatchEvent(new CustomEvent<Resolved>("edu-theme", { detail: resolved }));
}

media.addEventListener("change", () => {
  if (preference === "system") applyTheme("system");
});

export function storedTheme(): Theme {
  try {
    const t = localStorage.getItem(KEY);
    if (t === "light" || t === "dark" || t === "system") return t;
  } catch {
    // ignore
  }
  return "system";
}

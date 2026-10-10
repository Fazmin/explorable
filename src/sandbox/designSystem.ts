// Design system injected into every generated UI. Variable names follow the
// contract the agent is taught; values match this app's palette. Themes switch through :root[data-theme], which the host
// keeps in sync with the app, not the OS.
import atkinson from "@fontsource-variable/atkinson-hyperlegible-next/files/atkinson-hyperlegible-next-latin-wght-normal.woff2?inline";
import literata from "@fontsource-variable/literata/files/literata-latin-wght-normal.woff2?inline";

export const FONT_FACES_CSS = `
@font-face { font-family: "Atkinson Hyperlegible Next"; font-style: normal; font-weight: 200 800; font-display: swap; src: url(${atkinson}) format("woff2"); }
@font-face { font-family: "Literata"; font-style: normal; font-weight: 200 900; font-display: swap; src: url(${literata}) format("woff2"); }
`;

export const THEME_CSS = `
:root {
  color-scheme: light;
  --color-background-primary: #ffffff;
  --color-background-secondary: #f1f3f7;
  --color-background-tertiary: #e7eaf0;
  --color-background-info: #e7ecfc;
  --color-background-success: #e5f3ea;
  --color-background-warning: #fbf1d9;
  --color-background-danger: #fbe9ea;

  --color-text-primary: #18213a;
  --color-text-secondary: #5b6478;
  --color-text-tertiary: #8a92a3;
  --color-text-info: #2d4fd6;
  --color-text-success: #2f7d4f;
  --color-text-warning: #9a6a08;
  --color-text-danger: #b4232f;

  --color-border-primary: rgba(24, 33, 58, 0.45);
  --color-border-secondary: rgba(24, 33, 58, 0.26);
  --color-border-tertiary: #dde2ea;
  --color-border-info: #2d4fd6;
  --color-border-success: #2f7d4f;
  --color-border-warning: #9a6a08;
  --color-border-danger: #b4232f;

  --font-sans: "Atkinson Hyperlegible Next", system-ui, -apple-system, "Segoe UI", sans-serif;
  --font-serif: "Literata", Georgia, "Times New Roman", serif;
  --font-mono: ui-monospace, "SF Mono", Menlo, Consolas, monospace;

  --border-radius-md: 6px;
  --border-radius-lg: 10px;
  --border-radius-xl: 16px;

  --p: var(--color-text-primary);
  --s: var(--color-text-secondary);
  --t: var(--color-text-tertiary);
  --bg2: var(--color-background-secondary);
  --b: var(--color-border-tertiary);
}

:root[data-theme="dark"] {
  color-scheme: dark;
  --color-background-primary: #171c27;
  --color-background-secondary: #1f2533;
  --color-background-tertiary: #283043;
  --color-background-info: #232b4a;
  --color-background-success: #15291f;
  --color-background-warning: #2e2512;
  --color-background-danger: #33171b;

  --color-text-primary: #e7eaf0;
  --color-text-secondary: #9ba4b6;
  --color-text-tertiary: #6d7689;
  --color-text-info: #8fa6ff;
  --color-text-success: #6cc79a;
  --color-text-warning: #e3b654;
  --color-text-danger: #f2858d;

  --color-border-primary: rgba(231, 234, 240, 0.45);
  --color-border-secondary: rgba(231, 234, 240, 0.24);
  --color-border-tertiary: #2a3142;
  --color-border-info: #8fa6ff;
  --color-border-success: #6cc79a;
  --color-border-warning: #e3b654;
  --color-border-danger: #f2858d;
}
`;

const RAMPS: Record<string, { light: [string, string, string, string]; dark: [string, string, string, string] }> = {
  // [fill, stroke, heading text, small text]
  purple: { light: ["#EEEDFE", "#534AB7", "#3C3489", "#534AB7"], dark: ["#3C3489", "#AFA9EC", "#CECBF6", "#AFA9EC"] },
  teal: { light: ["#E1F5EE", "#0F6E56", "#085041", "#0F6E56"], dark: ["#085041", "#5DCAA5", "#9FE1CB", "#5DCAA5"] },
  coral: { light: ["#FAECE7", "#993C1D", "#712B13", "#993C1D"], dark: ["#712B13", "#F0997B", "#F5C4B3", "#F0997B"] },
  pink: { light: ["#FBEAF0", "#993556", "#72243E", "#993556"], dark: ["#72243E", "#ED93B1", "#F4C0D1", "#ED93B1"] },
  gray: { light: ["#F1F2F5", "#5B6478", "#3A4255", "#5B6478"], dark: ["#3A4255", "#B4BAC7", "#DCE0E8", "#B4BAC7"] },
  blue: { light: ["#E7ECFC", "#2D4FD6", "#1D3596", "#2D4FD6"], dark: ["#1D3596", "#8FA6FF", "#C9D4FF", "#8FA6FF"] },
  green: { light: ["#EAF3DE", "#3B6D11", "#27500A", "#3B6D11"], dark: ["#27500A", "#97C459", "#C0DD97", "#97C459"] },
  amber: { light: ["#FAEEDA", "#854F0B", "#633806", "#854F0B"], dark: ["#633806", "#EF9F27", "#FAC775", "#EF9F27"] },
  red: { light: ["#FCEBEB", "#A32D2D", "#791F1F", "#A32D2D"], dark: ["#791F1F", "#F09595", "#F7C1C1", "#F09595"] },
};

function ramp(name: string, [fill, stroke, heading, small]: string[], scope: string) {
  return `${scope}svg .c-${name} > rect, ${scope}svg .c-${name} > circle, ${scope}svg .c-${name} > ellipse, ${scope}svg .c-${name} > path, ${scope}svg .c-${name} > polygon,
${scope}svg rect.c-${name}, ${scope}svg circle.c-${name}, ${scope}svg ellipse.c-${name}, ${scope}svg path.c-${name}, ${scope}svg polygon.c-${name} { fill: ${fill}; stroke: ${stroke}; }
${scope}svg .c-${name} text.th, ${scope}svg .c-${name} text.t { fill: ${heading}; }
${scope}svg .c-${name} text.ts { fill: ${small}; }`;
}

export const SVG_CLASSES_CSS = `
svg text.t  { font: 400 14px var(--font-sans); fill: var(--p); }
svg text.ts { font: 400 12px var(--font-sans); fill: var(--s); }
svg text.th { font: 500 14px var(--font-sans); fill: var(--p); }
svg .box > rect, svg .box > circle, svg .box > ellipse { fill: var(--bg2); stroke: var(--b); }
svg .node { cursor: pointer; }
svg .node:hover { opacity: 0.85; }
svg .arr { stroke: var(--s); stroke-width: 1.5; fill: none; }
svg .leader { stroke: var(--t); stroke-width: 0.5; stroke-dasharray: 4 4; fill: none; }
${Object.entries(RAMPS)
  .map(([n, r]) => ramp(n, r.light, ""))
  .join("\n")}
${Object.entries(RAMPS)
  .map(([n, r]) => ramp(n, r.dark, ':root[data-theme="dark"] '))
  .join("\n")}
`;

export const FORM_STYLES_CSS = `
*, *::before, *::after { box-sizing: border-box; }
html { background: transparent; }
body {
  margin: 0;
  padding: 16px 18px 18px;
  font-family: var(--font-sans);
  font-size: 15px;
  line-height: 1.6;
  color: var(--color-text-primary);
  background: transparent;
  -webkit-font-smoothing: antialiased;
}
h1, h2, h3 { font-family: var(--font-serif); font-weight: 600; line-height: 1.25; margin: 0 0 0.4em; }
h1 { font-size: 1.4rem; } h2 { font-size: 1.15rem; } h3 { font-size: 1rem; }
p { margin: 0 0 0.6em; }
button {
  font: inherit;
  font-size: 14px;
  line-height: 1.2;
  padding: 7px 14px;
  border: 1px solid var(--color-border-secondary);
  border-radius: var(--border-radius-md);
  background: var(--color-background-primary);
  color: var(--color-text-primary);
  cursor: pointer;
  transition: background-color 0.15s, border-color 0.15s;
}
button:hover { background: var(--color-background-secondary); border-color: var(--color-border-primary); }
button:active { transform: translateY(1px); }
button:disabled { opacity: 0.55; cursor: not-allowed; transform: none; }
button.primary, button[data-variant="primary"] {
  background: var(--color-text-info);
  border-color: var(--color-text-info);
  color: var(--color-background-primary);
}
button.primary:hover { filter: brightness(1.08); background: var(--color-text-info); }
button[aria-pressed="true"] { background: var(--color-background-info); border-color: var(--color-border-info); color: var(--color-text-info); }
input[type="text"], input[type="number"], input[type="email"], input[type="search"], input:not([type]), textarea, select {
  font: inherit;
  font-size: 14px;
  padding: 6px 10px;
  min-height: 34px;
  border: 1px solid var(--color-border-secondary);
  border-radius: var(--border-radius-md);
  background: var(--color-background-primary);
  color: var(--color-text-primary);
}
textarea { min-height: 80px; resize: vertical; }
input::placeholder, textarea::placeholder { color: var(--color-text-tertiary); }
input[aria-invalid="true"] { border-color: var(--color-border-danger); }
input[type="range"] { width: 100%; accent-color: var(--color-text-info); }
input[type="checkbox"], input[type="radio"] { width: 16px; height: 16px; accent-color: var(--color-text-info); }
:focus-visible { outline: 2px solid var(--color-text-info); outline-offset: 2px; }
a { color: var(--color-text-info); }
table { border-collapse: collapse; }
th, td { border-bottom: 1px solid var(--color-border-tertiary); padding: 6px 10px; text-align: left; }
.morph-enter { animation: eduFadeIn 0.3s ease-out both; }
@keyframes eduFadeIn { from { opacity: 0; transform: translateY(4px); } to { opacity: 1; transform: none; } }
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after { animation-duration: 0.01ms !important; animation-iteration-count: 1 !important; transition-duration: 0.01ms !important; }
}
`;

// Mirrors the app's accessibility settings (see lib/a11y.ts).
export const A11Y_CSS = `
:root:not([data-theme="dark"])[data-contrast="more"] {
  --color-text-primary: #0d1428;
  --color-text-secondary: #2f384c;
  --color-text-tertiary: #4b5466;
  --color-border-primary: rgba(13, 20, 40, 0.85);
  --color-border-secondary: rgba(13, 20, 40, 0.6);
  --color-border-tertiary: #9ea7b6;
  --color-text-info: #1d38b8;
}
:root[data-theme="dark"][data-contrast="more"] {
  --color-text-primary: #ffffff;
  --color-text-secondary: #d7dce6;
  --color-text-tertiary: #b0b8c8;
  --color-border-primary: rgba(255, 255, 255, 0.8);
  --color-border-secondary: rgba(255, 255, 255, 0.55);
  --color-border-tertiary: #4e576c;
  --color-text-info: #b6c4ff;
}
:root[data-contrast="more"] :focus-visible { outline-width: 3px; }
:root[data-font="serif"] { --font-sans: "Literata", Georgia, serif; }
:root[data-font="system"] { --font-sans: system-ui, -apple-system, "Segoe UI", sans-serif; }
:root[data-spacing="relaxed"] body { line-height: 1.9; letter-spacing: 0.02em; word-spacing: 0.08em; }
:root[data-reduce-motion="true"] *, :root[data-reduce-motion="true"] *::before, :root[data-reduce-motion="true"] *::after {
  animation-duration: 0.01ms !important; animation-iteration-count: 1 !important; transition-duration: 0.01ms !important;
}
`;

export const DESIGN_SYSTEM_CSS = `${FONT_FACES_CSS}\n${THEME_CSS}\n${SVG_CLASSES_CSS}\n${FORM_STYLES_CSS}\n${A11Y_CSS}`;

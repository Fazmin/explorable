// Turns answer Markdown (with LaTeX) into text a speech synthesizer can read.

const GREEK = [
  "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa", "lambda", "mu",
  "nu", "xi", "pi", "rho", "sigma", "tau", "upsilon", "phi", "chi", "psi", "omega",
];

const SYMBOLS: [RegExp, string][] = [
  [/\\times|\\cdot/g, " times "],
  [/\\div/g, " divided by "],
  [/\\pm/g, " plus or minus "],
  [/\\leq?|≤/g, " less than or equal to "],
  [/\\geq?|≥/g, " greater than or equal to "],
  [/\\neq|≠/g, " not equal to "],
  [/\\approx|≈/g, " approximately "],
  [/\\infty|∞/g, " infinity "],
  [/\\to|\\rightarrow/g, " goes to "],
  [/\\sum/g, " the sum of "],
  [/\\int/g, " the integral of "],
  [/\\partial/g, " partial "],
  [/\\mid/g, " given "],
  [/\\ln/g, " natural log of "],
  [/\\log/g, " log of "],
  [/\\(sin|cos|tan|exp|lim)\b/g, " $1 "],
];

/** Speaks a TeX expression in plain words, as well as a simple pass can. */
export function texToSpeech(tex: string): string {
  let s = tex;
  // Innermost first, repeated, so nested fractions read correctly.
  for (let i = 0; i < 5; i++) {
    s = s
      .replace(/\\[dt]?frac\{([^{}]*)\}\{([^{}]*)\}/g, " $1 over $2 ")
      .replace(/\\sqrt\{([^{}]*)\}/g, " the square root of $1 ")
      .replace(/\^\{([^{}]*)\}/g, " to the power of $1 ")
      .replace(/_\{([^{}]*)\}/g, " sub $1 ")
      .replace(/\\(?:mathrm|text|mathbf|mathit|operatorname)\{([^{}]*)\}/g, " $1 ");
  }
  s = s.replace(/\^2\b/g, " squared ").replace(/\^3\b/g, " cubed ").replace(/\^(\w)/g, " to the power of $1 ").replace(/_(\w)/g, " sub $1 ");
  for (const [re, words] of SYMBOLS) s = s.replace(re, words);
  s = s.replace(new RegExp(`\\\\(${GREEK.join("|")})\\b`, "gi"), " $1 ");
  s = s.replace(/\\[a-zA-Z]+/g, " ").replace(/[{}\\]/g, " ").replace(/=/g, " equals ");
  return s.replace(/\s+/g, " ").trim();
}

export function markdownToSpeech(md: string): string {
  return md
    .replace(/```[\s\S]*?```/g, " (code block) ")
    .replace(/\$\$([\s\S]+?)\$\$/g, (_, tex) => ` ${texToSpeech(tex)}. `)
    .replace(/\$([^$\n]+?)\$/g, (_, tex) => ` ${texToSpeech(tex)} `)
    .replace(/`([^`]+)`/g, "$1")
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/^\s{0,3}#{1,6}\s*/gm, "")
    .replace(/^\s*[-*+]\s+/gm, "")
    .replace(/^\s*>\s?/gm, "")
    .replace(/(\*\*|__|\*|_|~~)(?=\S)([^*_~]+?)\1/g, "$2")
    .replace(/\|/g, " ")
    .replace(/[ \t]+/g, " ")
    .replace(/ ([.,;:!?])/g, "$1")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .join("\n");
}

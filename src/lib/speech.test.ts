import { describe, expect, it } from "vitest";
import { markdownToSpeech, texToSpeech } from "./speech";

describe("speech", () => {
  it("reads common TeX", () => {
    expect(texToSpeech("\\frac{a}{b}")).toBe("a over b");
    expect(texToSpeech("x^2 + y^{n}")).toBe("x squared + y to the power of n");
    expect(texToSpeech("\\sqrt{\\pi}")).toBe("the square root of pi");
    expect(texToSpeech("P(H \\mid E) = \\frac{P(E \\mid H) P(H)}{P(E)}")).toBe(
      "P(H given E) equals P(E given H) P(H) over P(E)",
    );
  });

  it("strips Markdown and keeps the words", () => {
    const md = "## Entropy\n\nThe **second law** says $\\Delta S \\ge 0$.\n\n- Read [the notes](https://x.y)\n\n```js\ncode()\n```";
    expect(markdownToSpeech(md)).toBe("Entropy\nThe second law says Delta S greater than or equal to 0.\nRead the notes\n(code block)");
  });
});

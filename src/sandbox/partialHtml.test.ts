import { describe, expect, it } from "vitest";
import { extractCompleteStyles, processPartialHtml } from "./partialHtml";

describe("processPartialHtml", () => {
  it("drops an unfinished trailing tag", () => {
    expect(processPartialHtml("<p>Hello</p><div cla")).toBe("<p>Hello</p>");
  });

  it("removes scripts and styles, complete or not", () => {
    expect(processPartialHtml("<p>a</p><script>alert(1)</script><p>b</p>")).toBe("<p>a</p><p>b</p>");
    expect(processPartialHtml("<p>a</p><style>p{color:red")).toBe("<p>a</p>");
  });

  it("drops an unfinished entity", () => {
    expect(processPartialHtml("<p>Fish &amp; chips &am")).toBe("<p>Fish &amp; chips ");
  });

  it("keeps only body content", () => {
    expect(processPartialHtml("<html><head><title>x</title></head><body><main>hi</main></body></html>")).toBe(
      "<main>hi</main>",
    );
  });

  it("extracts complete style blocks only", () => {
    expect(extractCompleteStyles("<style>a{}</style><p></p><style>b{")).toBe("<style>a{}</style>");
  });
});

// Drives the real frontend in Chrome against the mock IPC layer, checks key
// behaviors (including inside sandboxed frames), and saves screenshots.
//   node tests/e2e/screenshots.mjs            (screenshots in tests/e2e/screenshots)
//   SHOTS_DIR=/tmp/shots node tests/e2e/screenshots.mjs
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import { chromium } from "playwright";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");
const outDir = process.env.SHOTS_DIR || path.join(here, "screenshots");
fs.mkdirSync(outDir, { recursive: true });
const fixtures = fs.readFileSync(path.join(root, "tests/fixtures/demo-events.json"), "utf8");
const mock = fs.readFileSync(path.join(here, "mock-ipc.js"), "utf8");

const failures = [];
const check = (cond, message) => {
  if (!cond) failures.push(message);
  console.log(`${cond ? "✓" : "✗"} ${message}`);
};

const server = await createServer({ root, configFile: path.join(root, "vite.config.ts"), logLevel: "warn", server: { port: 1430, strictPort: true } });
await server.listen();
const browser = await chromium.launch({ channel: "chrome", headless: true });
const context = await browser.newContext({ viewport: { width: 1320, height: 860 }, deviceScaleFactor: 2, colorScheme: "light" });
// Init scripts also run in child frames; keep the mock out of the sandboxes.
await context.addInitScript({ content: `if (window.top === window) { window.__EDU_FIXTURES__ = ${fixtures}; ${mock} }` });
const page = await context.newPage();
const pageErrors = [];
page.on("pageerror", (e) => pageErrors.push(e.message));
page.on("console", (m) => m.type() === "error" && pageErrors.push(m.text()));

const shot = async (name) => {
  await page.waitForTimeout(250);
  await page.screenshot({ path: path.join(outDir, `${name}.png`) });
};

/** The final sandboxed frame (not the preview) whose document contains `selector`. */
async function frameWith(selector, timeout = 15000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    for (const f of page.frames()) {
      if (f === page.mainFrame()) continue;
      try {
        if ((await f.$(selector)) && !(await f.$("#edu-css"))) return f;
      } catch {
        // Frame navigated; try again.
      }
    }
    await page.waitForTimeout(150);
  }
  return null;
}

const starter = (title) => page.locator(".starter", { hasText: title }).click();
const newChat = () => page.getByRole("button", { name: "New chat" }).click();
const waitIdle = () => page.waitForSelector(".send-btn:not(.stop-btn)", { timeout: 30000 });

try {
  await page.goto("http://localhost:1430/");
  await page.waitForSelector(".welcome");
  check(await page.locator(".demo-banner").isVisible(), "demo banner explains scripted answers");
  await shot("01-welcome-learn");

  // ---- Interactive: streaming preview, final sandbox, scripts, bridge, theme
  await starter("Build a wave from sines");
  await page.waitForSelector(".frame-stage iframe", { timeout: 15000 });
  await page.waitForTimeout(1300);
  await shot("02-interactive-streaming");
  const fx = await frameWith("#fx-status");
  check(!!fx, "final interactive frame is created");
  await page.waitForFunction(() => !document.querySelector(".frame-stage iframe.hidden"), null, { timeout: 15000 });
  await waitIdle();
  const status = fx ? await fx.locator("#fx-status").textContent({ timeout: 10000 }) : "";
  check(/harmonics/.test(status ?? ""), `interactive scripts ran inside the sandbox (status: "${status}")`);
  if (fx) {
    await fx.locator("#fx-n").evaluate((el) => {
      el.value = "12";
      el.dispatchEvent(new Event("input", { bubbles: true }));
    });
    const after = await fx.locator("#fx-status").textContent();
    check(/^12 harmonics/.test(after ?? ""), "slider input updates the interactive");
    const isolated = await fx.evaluate(() => {
      const r = {};
      try { localStorage.getItem("x"); r.storage = "open"; } catch { r.storage = "blocked"; }
      try { r.parent = typeof window.parent.document; } catch { r.parent = "blocked"; }
      r.ipc = typeof window.__TAURI_INTERNALS__;
      return r;
    });
    check(isolated.storage === "blocked" && isolated.parent === "blocked" && isolated.ipc === "undefined", `sandbox is isolated ${JSON.stringify(isolated)}`);
    // Playwright's evaluate counts as a user gesture, so call from a timer
    // after the transient activation window (5 s in Chrome) has expired.
    const autoSend = await fx.evaluate(
      () =>
        new Promise((resolve) =>
          setTimeout(() => {
            // Generated code trying to skip the bridge and post to the app directly.
            window.parent.postMessage({ __edu: true, type: "bridge", id: 1, method: "sendPrompt", args: { text: "bypass attempt" } }, "*");
            window.edu.sendPrompt({ text: "auto" }).then(() => resolve("sent"), (e) => resolve(e.message));
          }, 5600),
        ),
    );
    check(/click/.test(autoSend), "sendPrompt without a user gesture is refused");
    await page.waitForTimeout(800);
    check((await page.locator('.user-msg:has-text("bypass attempt")').count()) === 0, "a direct postMessage from generated code is ignored");
    check(!(await fx.evaluate(() => document.documentElement.outerHTML.includes("NONCE"))), "the bridge nonce is not readable from the frame's DOM");
  }
  const height = await page.locator(".frame-stage").evaluate((el) => el.getBoundingClientRect().height);
  check(height > 400, `frame auto-sizes to content (${Math.round(height)}px)`);
  await shot("03-interactive-final");

  await page.getByRole("button", { name: "Switch to dark theme" }).click();
  await page.waitForTimeout(400);
  const frameTheme = fx ? await fx.evaluate(() => document.documentElement.dataset.theme) : null;
  check(frameTheme === "dark", "theme change reaches the sandboxed frame");
  await shot("04-interactive-dark");

  // Accessibility quick menu: each option reaches the app and the sandbox.
  await page.getByRole("button", { name: "Accessibility options" }).click();
  await page.waitForSelector(".popover");
  await page.getByLabel("Increase contrast").check();
  await page.getByLabel("Reduce motion").check();
  await page.getByRole("button", { name: "Larger text" }).click();
  await page.waitForTimeout(400);
  const appA11y = await page.evaluate(() => ({ contrast: document.documentElement.dataset.contrast, zoom: document.documentElement.style.zoom }));
  check(appA11y.contrast === "more" && appA11y.zoom === "1.15", `contrast and text size apply to the app ${JSON.stringify(appA11y)}`);
  if (fx) {
    const frameA11y = await fx.evaluate(() => ({
      contrast: document.documentElement.getAttribute("data-contrast"),
      reduce: window.matchMedia("(prefers-reduced-motion: reduce)").matches,
    }));
    check(frameA11y.contrast === "more" && frameA11y.reduce === true, `contrast and reduced motion reach the interactive ${JSON.stringify(frameA11y)}`);
  }
  await shot("04b-accessibility-menu");
  await page.getByLabel("Increase contrast").uncheck();
  await page.getByLabel("Reduce motion").uncheck();
  await page.getByRole("button", { name: "Smaller text" }).click();
  await page.keyboard.press("Escape");

  if (fx) {
    // Keyboard activation: deterministic, and checks the bridge works without a mouse.
    await fx.locator("#fx-ask").focus();
    await fx.locator("#fx-ask").press("Enter");
    await page.waitForSelector('.user-msg:has-text("In the Fourier demo")', { timeout: 10000 }).catch(() => null);
    const askStatus = await fx.locator("#fx-ask-status").textContent().catch(() => "?");
    check(
      (await page.locator('.user-msg:has-text("In the Fourier demo")').count()) === 1,
      `a key press inside the frame sends a follow-up through the bridge (frame status: "${askStatus}")`,
    );
    await waitIdle();
  }
  await page.getByRole("button", { name: "Switch to light theme" }).click();

  // ---- Quiz
  await newChat();
  await starter("Quiz me on cellular respiration");
  await page.waitForSelector("text=Check answers", { timeout: 20000 });
  await waitIdle();
  const questions = page.locator(".question");
  check((await questions.count()) === 5, "quiz renders five questions");
  await questions.nth(0).locator(".option").nth(1).click();
  await questions.nth(1).locator(".option").nth(1).click();
  await questions.nth(2).locator("input").fill("2");
  await questions.nth(3).locator("input").fill("Oxygen");
  await questions.nth(4).locator(".option").nth(2).click();
  await shot("05-quiz");
  await page.getByRole("button", { name: "Check answers" }).click();
  const score = await page.locator(".score").textContent();
  check(/4 of 5/.test(score ?? ""), `quiz grades locally (${score})`);
  await page.locator(".quiz-actions").scrollIntoViewIfNeeded();
  await shot("06-quiz-results");

  // ---- Flashcards
  await newChat();
  await starter("Flashcards for functional groups");
  await page.waitForSelector(".flashcard", { timeout: 20000 });
  await waitIdle();
  check(await page.locator("text=Study in Review").isVisible(), "deck links to Review once saved");
  await page.locator(".flashcard").click();
  await page.waitForTimeout(600);
  await shot("07-flashcards");

  // ---- Worked example
  await newChat();
  await starter("Solve ∫ x·eˣ dx step by step");
  await page.waitForSelector("text=Reveal step 1", { timeout: 20000 });
  await waitIdle();
  await page.getByRole("button", { name: "Reveal step 1" }).click();
  await page.getByRole("button", { name: "Reveal step 2" }).click();
  check(await page.locator(".katex").count() > 3, "math renders with KaTeX");
  await shot("08-worked-example");

  // ---- Table
  await newChat();
  await starter("Compare study strategies");
  await page.waitForSelector(".data-table", { timeout: 20000 });
  await waitIdle();
  await shot("09-table");

  // ---- Teach mode
  await page.getByRole("button", { name: "Teach", exact: true }).click();
  await page.waitForSelector("text=What are you teaching?");
  await shot("10-welcome-teach");
  await starter("A 50-minute lesson on Bayesian inference");
  await page.waitForSelector(".timeline", { timeout: 20000 });
  await waitIdle();
  await page.getByRole("button", { name: "Save to Library" }).first().click();
  await shot("11-lesson-plan");
  await newChat();
  await starter("A rubric for a research paper");
  await page.waitForSelector(".rubric-table", { timeout: 20000 });
  await waitIdle();
  await shot("12-rubric");

  await newChat();
  await starter("Classroom demo: central limit theorem");
  const clt = await frameWith("#clt-count", 20000);
  await waitIdle();
  if (clt) {
    await clt.getByRole("button", { name: "Draw 1,000 samples" }).click();
    const count = await clt.locator("#clt-count").textContent();
    check(/1,000/.test(count ?? ""), `simulation runs inside the sandbox (${count})`);
  }
  await page.getByRole("button", { name: "Present full screen" }).first().click();
  await page.waitForSelector(".present");
  await page.waitForTimeout(1500);
  await shot("13-present");
  // With focus inside the frame, Escape has to cross the sandbox to close Present view.
  const presentFrame = page.locator(".present iframe:not(.hidden)").last().contentFrame();
  const inFrame = presentFrame.getByRole("button", { name: "Draw 1 sample" });
  const presenting = async () => {
    await page.waitForTimeout(400);
    return page.locator(".present").isVisible();
  };
  await presentFrame.locator("body").evaluate(() => document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })));
  check(await presenting(), "an Escape faked by generated code doesn't close Present view");
  await presentFrame.locator("body").evaluate(() => window.addEventListener("keydown", (e) => e.key === "Escape" && e.preventDefault(), { once: true }));
  await inFrame.press("Escape");
  check(await presenting(), "an Escape the interactive handles itself stays in the interactive");
  await inFrame.press("Escape");
  check(!(await presenting()), "Escape pressed inside an interactive closes Present view");

  // ---- Course, materials, library, review, progress, settings
  await page.locator(".course-button").click();
  await page.getByRole("menuitemradio", { name: /Thermodynamics/ }).click();
  await page.getByRole("button", { name: "Course materials" }).click();
  await page.waitForSelector(".material-row");
  await page.locator(".search-test input").fill("Carnot efficiency");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await shot("14-materials");

  await page.locator(".course-button").click();
  await page.getByRole("menuitemradio", { name: /General/ }).click();
  await page.getByRole("button", { name: "Library" }).click();
  await page.waitForSelector(".library-item");
  await shot("15-library");

  await page.getByRole("button", { name: "Learn", exact: true }).click();
  await page.locator(".course-button").click();
  await page.getByRole("menuitemradio", { name: /Thermodynamics/ }).click();
  await page.getByRole("button", { name: /^Review/ }).click();
  await page.waitForSelector(".flashcard");
  await page.getByRole("button", { name: "Show answer" }).click();
  await page.waitForTimeout(600);
  await shot("16-review");

  await page.getByRole("button", { name: "Progress" }).click();
  await page.waitForSelector(".chart svg");
  await page.locator(".chart .hit").nth(22).hover();
  await shot("17-progress");
  await page.getByRole("button", { name: "Switch to dark theme" }).click();
  await page.mouse.move(5, 5);
  await shot("18-progress-dark");

  await page.getByRole("button", { name: "Settings" }).click();
  await page.waitForSelector(".settings");
  await shot("19-settings-dark");
  await page.locator("#accessibility-settings").scrollIntoViewIfNeeded();
  await page.locator("#accessibility-settings").getByRole("button", { name: "Serif" }).click();
  await page.waitForTimeout(200);
  check((await page.evaluate(() => document.documentElement.dataset.font)) === "serif", "the reading font setting applies");
  await page.locator("#accessibility-settings").screenshot({ path: path.join(outDir, "24-settings-accessibility.png") });
  await page.locator("#accessibility-settings").getByRole("button", { name: "Hyperlegible" }).click();
  await page.getByRole("button", { name: "Switch to light theme" }).click();

  // Provider switching: each provider shows its own key, models, and router.
  await page.locator(".provider-option", { hasText: "OpenAI" }).click();
  await page.waitForSelector('input[aria-label="OpenAI API key"]');
  const openaiModels = await page.locator(".settings select").first().locator("option").allTextContents();
  check(openaiModels.some((t) => t.startsWith("GPT-6.1 Sol")), "OpenAI models are offered after choosing OpenAI");
  await page.locator('input[aria-label="OpenAI API key"]').fill("sk-proj-example");
  await page.getByRole("button", { name: "Save key" }).click();
  await page.waitForSelector("text=Saved in the system keychain");
  await shot("22-settings-openai");
  await page.locator(".provider-option", { hasText: "Gemini" }).click();
  await page.waitForSelector('input[aria-label="Google API key"]');
  const geminiModels = await page.locator(".settings select").first().locator("option").allTextContents();
  check(geminiModels.some((t) => t.startsWith("Gemini 3.8 Flash")), "Gemini models are offered after choosing Gemini");
  check(await page.locator(".provider-option", { hasText: "OpenAI" }).locator("text=Key saved").isVisible(), "the OpenAI card shows its saved key");
  await shot("23-settings-gemini");
  await page.locator(".provider-option", { hasText: "Demo mode" }).click();

  await page.getByRole("button", { name: "Chat", exact: true }).click();
  await page.locator(".course-button").click();
  await page.getByRole("menuitem", { name: "New course" }).click();
  await page.waitForSelector(".dialog");
  await shot("20-course-dialog");
  await page.keyboard.press("Escape");

  // Narrow window: the layout must not overflow horizontally.
  await page.setViewportSize({ width: 980, height: 760 });
  await page.getByRole("button", { name: "Chat", exact: true }).click();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  check(overflow <= 0, `no horizontal overflow at 980px (${overflow}px)`);
  await shot("21-narrow");
} catch (err) {
  failures.push(`Harness error: ${err.message}`);
  console.error(err);
  await shot("zz-failure").catch(() => {});
} finally {
  const relevant = pageErrors.filter((e) => !/Failed to load resource|net::ERR|esm\.sh|jsdelivr/i.test(e));
  check(relevant.length === 0, `no page errors${relevant.length ? `: ${relevant.slice(0, 5).join(" | ")}` : ""}`);
  await browser.close();
  await server.close();
  console.log(`\nScreenshots: ${outDir}`);
  console.log(failures.length ? `\n${failures.length} check(s) failed` : "\nAll checks passed");
  process.exit(failures.length ? 1 : 0);
}

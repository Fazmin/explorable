// Regenerates the README screenshots (docs/images) and the demo tour
// (docs/media) from the real frontend, driven in Chrome against the mock IPC
// layer and recorded agent output, the same setup as tests/e2e/screenshots.mjs.
//   npm run media
//   FFMPEG=/path/to/ffmpeg npm run media
// Needs Google Chrome and an ffmpeg build with libx264.
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import { chromium } from "playwright";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const imagesDir = path.join(root, "docs/images");
const mediaDir = path.join(root, "docs/media");
const ffmpeg = process.env.FFMPEG || "ffmpeg";
const PORT = 1431;

if (spawnSync(ffmpeg, ["-version"]).error) {
  console.error(`Couldn't run ffmpeg ("${ffmpeg}"). Install it, or set FFMPEG to its path.`);
  process.exit(1);
}

const work = fs.mkdtempSync(path.join(os.tmpdir(), "edu-media-"));
fs.mkdirSync(imagesDir, { recursive: true });
fs.mkdirSync(mediaDir, { recursive: true });
const fixtures = fs.readFileSync(path.join(root, "tests/fixtures/demo-events.json"), "utf8");
const mock = fs.readFileSync(path.join(root, "tests/e2e/mock-ipc.js"), "utf8");

function run(args) {
  const r = spawnSync(ffmpeg, ["-hide_banner", "-loglevel", "error", "-y", ...args], { stdio: "inherit" });
  if (r.status !== 0) throw new Error(`ffmpeg failed: ${args.join(" ")}`);
}

// A drawn pointer for the tour, since headless Chrome doesn't render one.
const CURSOR = `
addEventListener("DOMContentLoaded", () => {
  const c = document.createElement("div");
  c.innerHTML = '<svg width="20" height="24" viewBox="0 0 20 24"><path d="M2 2v17l4.6-4.3 3 6.6 3-1.3-3-6.5H16z" fill="#111" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/></svg>';
  c.style.cssText = "position:fixed;left:0;top:0;z-index:2147483647;pointer-events:none;transform:translate(-60px,-60px);filter:drop-shadow(0 1px 1.5px rgba(0,0,0,.35))";
  document.documentElement.appendChild(c);
  window.__demoCursor = (x, y) => { c.style.transform = "translate(" + (x - 2) + "px," + (y - 2) + "px)"; };
  window.__demoClick = (x, y) => {
    const r = document.createElement("div");
    r.style.cssText = "position:fixed;left:" + (x - 14) + "px;top:" + (y - 14) + "px;width:28px;height:28px;border-radius:50%;border:2px solid rgba(45,79,214,.75);z-index:2147483646;pointer-events:none;transition:transform .45s ease-out,opacity .45s ease-out;transform:scale(.3)";
    document.documentElement.appendChild(r);
    requestAnimationFrame(() => requestAnimationFrame(() => { r.style.transform = "scale(1.5)"; r.style.opacity = "0"; }));
    setTimeout(() => r.remove(), 600);
  };
});`;

let lastPage = null;

async function openApp(browser, viewport, { cursor = false } = {}) {
  const context = await browser.newContext({ viewport, deviceScaleFactor: 2, colorScheme: "light" });
  // Init scripts also run in child frames; keep the mock out of the sandboxes.
  await context.addInitScript({ content: `if (window.top === window) { window.__EDU_FIXTURES__ = ${fixtures}; ${mock} ${cursor ? CURSOR : ""} }` });
  const page = await context.newPage();
  lastPage = page;
  page.on("pageerror", (e) => console.warn(`page error: ${e.message}`));
  await page.goto(`http://localhost:${PORT}/`);
  await page.waitForSelector(".welcome");
  await page.evaluate(() => document.fonts.ready);
  return { context, page };
}

/** The final sandboxed frame (not the preview) whose document contains `selector`. */
async function frameWith(page, selector, timeout = 20000) {
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
  throw new Error(`No interactive frame contains ${selector}`);
}

/** The final interactive frame inside Present view. */
const presentFrame = (page) => page.locator(".present iframe:not(.hidden)").last().contentFrame();

/**
 * Page coordinates of an element inside Present view's frame, scrolled into
 * view. Present view enlarges the frame with a CSS transform, which
 * Playwright's boxes and clicks don't account for inside iframes.
 */
async function presentPoint(page, inner) {
  const locate = async () => {
    const frame = await page.locator(".present iframe:not(.hidden)").last().evaluate((el) => {
      const r = el.getBoundingClientRect();
      return { x: r.left, y: r.top, scale: r.width / el.offsetWidth };
    });
    const box = await inner.evaluate((el) => {
      const r = el.getBoundingClientRect();
      return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
    });
    return { x: frame.x + box.x * frame.scale, y: frame.y + box.y * frame.scale };
  };
  const point = await locate();
  const limit = page.viewportSize().height - 80;
  if (point.y <= limit) return point;
  await page.locator(".present-body").evaluate((el, dy) => el.scrollBy(0, dy), point.y - limit + 80);
  return locate();
}
const waitIdle = (page) => page.waitForSelector(".send-btn:not(.stop-btn)", { timeout: 30000 });
const waitFinalFrame = (page) => page.waitForFunction(() => !document.querySelector(".frame-stage iframe.hidden"), null, { timeout: 20000 });

async function screenshots(browser) {
  const { context, page } = await openApp(browser, { width: 1320, height: 860 });
  const starter = (title) => page.locator(".starter", { hasText: title }).click();
  const newChat = () => page.getByRole("button", { name: "New chat" }).click();
  const shot = async (name) => {
    await page.waitForTimeout(300);
    const raw = path.join(work, `${name}@2x.png`);
    await page.screenshot({ path: raw });
    run(["-i", raw, "-vf", "scale=1320:-1:flags=lanczos", path.join(imagesDir, `${name}.png`)]);
    console.log(`✓ docs/images/${name}.png`);
  };

  await starter("Build a wave from sines");
  const fx = await frameWith(page, "#fx-status");
  await waitFinalFrame(page);
  await waitIdle(page);
  await fx.locator("#fx-n").evaluate((el) => {
    el.value = "12";
    el.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await shot("learn-interactive");
  await page.getByRole("button", { name: "Switch to dark theme" }).click();
  await shot("dark-mode");
  await page.getByRole("button", { name: "Switch to light theme" }).click();
  await page.getByRole("button", { name: "Accessibility options" }).click();
  await page.waitForSelector(".popover");
  await shot("accessibility-menu");
  await page.keyboard.press("Escape");

  await newChat();
  await starter("Quiz me on cellular respiration");
  await page.waitForSelector("text=Check answers", { timeout: 20000 });
  await waitIdle(page);
  const questions = page.locator(".question");
  await questions.nth(0).locator(".option").nth(1).click();
  await questions.nth(1).locator(".option").nth(1).click();
  await questions.nth(2).locator("input").fill("2");
  await questions.nth(3).locator("input").fill("Oxygen");
  await questions.nth(4).locator(".option").nth(2).click();
  await page.getByRole("button", { name: "Check answers" }).click();
  await page.locator(".quiz-actions").scrollIntoViewIfNeeded();
  await shot("learn-quiz");

  await newChat();
  await starter("Flashcards for functional groups");
  await page.waitForSelector(".flashcard", { timeout: 20000 });
  await waitIdle(page);
  await page.locator(".flashcard").click();
  await page.waitForTimeout(600);
  await shot("learn-flashcards");

  await newChat();
  await starter("Solve ∫ x·eˣ dx step by step");
  await page.waitForSelector("text=Reveal step 1", { timeout: 20000 });
  await waitIdle(page);
  await page.getByRole("button", { name: "Reveal step 1" }).click();
  await page.getByRole("button", { name: "Reveal step 2" }).click();
  await shot("learn-worked-example");

  await page.getByRole("button", { name: "Teach", exact: true }).click();
  await page.waitForSelector("text=What are you teaching?");
  await starter("A 50-minute lesson on Bayesian inference");
  await page.waitForSelector(".timeline", { timeout: 20000 });
  await waitIdle(page);
  await shot("teach-lesson-plan");

  await newChat();
  await starter("A rubric for a research paper");
  await page.waitForSelector(".rubric-table", { timeout: 20000 });
  await waitIdle(page);
  await shot("teach-rubric");

  await newChat();
  await starter("Classroom demo: central limit theorem");
  await frameWith(page, "#clt-count");
  await waitIdle(page);
  await page.getByRole("button", { name: "Present full screen" }).first().click();
  await page.waitForSelector(".present");
  // Present view boots its own copy of the interactive.
  const draw = await presentPoint(page, presentFrame(page).getByRole("button", { name: "Draw 1,000 samples" }));
  await page.mouse.click(draw.x, draw.y);
  await page.waitForTimeout(1200);
  await page.locator(".present-body").evaluate((el) => el.scrollTo(0, 0));
  await shot("teach-present");
  await page.locator(".present-bar").getByRole("button", { name: "Exit" }).click();

  await page.locator(".course-button").click();
  await page.getByRole("menuitemradio", { name: /Thermodynamics/ }).click();
  await page.getByRole("button", { name: "Course materials" }).click();
  await page.waitForSelector(".material-row");
  await page.locator(".search-test input").fill("Carnot efficiency");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await shot("course-materials");

  await page.getByRole("button", { name: "Learn", exact: true }).click();
  await page.getByRole("button", { name: /^Review/ }).click();
  await page.waitForSelector(".flashcard");
  await page.getByRole("button", { name: "Show answer" }).click();
  await page.waitForTimeout(600);
  await shot("learn-review");

  await page.getByRole("button", { name: "Progress" }).click();
  await page.waitForSelector(".chart svg");
  await page.locator(".chart .hit").nth(22).hover();
  await shot("learn-progress");

  await page.getByRole("button", { name: "Settings" }).click();
  await page.waitForSelector(".settings");
  await page.locator(".provider-option", { hasText: "OpenAI" }).click();
  await page.waitForSelector('input[aria-label="OpenAI API key"]');
  await page.getByRole("heading", { name: "AI provider" }).evaluate((el) => el.scrollIntoView({ block: "start" }));
  await page.mouse.move(5, 5);
  await shot("settings-providers");
  await context.close();
}

async function tour(browser) {
  const viewport = { width: 1280, height: 800 };
  const { context, page } = await openApp(browser, viewport, { cursor: true });
  const framesDir = path.join(work, "frames");
  fs.mkdirSync(framesDir);
  const frames = [];
  const cdp = await context.newCDPSession(page);
  cdp.on("Page.screencastFrame", ({ data, metadata, sessionId }) => {
    const file = path.join(framesDir, `${String(frames.length).padStart(5, "0")}.jpg`);
    fs.writeFileSync(file, Buffer.from(data, "base64"));
    frames.push({ file, t: metadata.timestamp });
    cdp.send("Page.screencastFrameAck", { sessionId }).catch(() => {});
  });

  let pointer = { x: 760, y: 420 };
  const hold = (ms) => page.waitForTimeout(ms);
  const moveTo = async (x, y, ms = 650) => {
    const from = pointer;
    const steps = Math.max(10, Math.round(ms / 22));
    for (let i = 1; i <= steps; i++) {
      const t = i / steps;
      const e = t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2;
      const p = { x: from.x + (x - from.x) * e, y: from.y + (y - from.y) * e };
      await page.mouse.move(p.x, p.y);
      await page.evaluate(([px, py]) => window.__demoCursor?.(px, py), [p.x, p.y]);
      await page.waitForTimeout(12);
    }
    pointer = { x, y };
  };
  const center = async (locator) => {
    const box = await locator.boundingBox();
    if (!box) throw new Error("Element to click isn't visible");
    return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  };
  const clickAt = async ({ x, y }) => {
    await moveTo(x, y);
    await hold(120);
    await page.evaluate(([px, py]) => window.__demoClick?.(px, py), [x, y]);
    await page.mouse.click(x, y);
  };
  const click = async (locator) => clickAt(await center(locator));
  const scrollChat = (dy, ms = 800) =>
    page.evaluate(
      ([dy, ms]) =>
        new Promise((resolve) => {
          const el = document.querySelector(".chat-scroll");
          const start = el.scrollTop;
          const t0 = performance.now();
          const step = (now) => {
            const t = Math.min(1, (now - t0) / ms);
            el.scrollTop = start + dy * (t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2);
            if (t < 1) requestAnimationFrame(step);
            else resolve();
          };
          requestAnimationFrame(step);
        }),
      [dy, ms],
    );
  // Scrolls the chat so the element sits between the header and the composer.
  const reveal = async (locator, bottom = 600) => {
    const box = await locator.boundingBox();
    if (box && box.y + box.height > bottom) await scrollChat(box.y + box.height - bottom);
    else if (box && box.y < 130) await scrollChat(box.y - 130);
  };
  const starter = (title) => click(page.locator(".starter", { hasText: title }));
  const newChat = () => click(page.getByRole("button", { name: "New chat" }));

  await page.evaluate(([x, y]) => window.__demoCursor?.(x, y), [pointer.x, pointer.y]);
  await cdp.send("Page.startScreencast", { format: "jpeg", quality: 92, maxWidth: 1600, maxHeight: 1000, everyNthFrame: 1 });
  await hold(1200);

  // An interactive explainer streams in, then responds to the slider and the theme.
  await starter("Build a wave from sines");
  const fx = await frameWith(page, "#fx-status");
  await waitFinalFrame(page);
  await waitIdle(page);
  await hold(500);
  const slider = fx.locator("#fx-n");
  await reveal(slider, 560);
  await hold(300);
  const track = await slider.boundingBox();
  const at = (n) => ({ x: track.x + 8 + ((n - 1) / 39) * (track.width - 16), y: track.y + track.height / 2 });
  await moveTo(at(3).x, at(3).y);
  await page.mouse.down();
  await moveTo(at(28).x, at(28).y, 1800);
  await page.mouse.up();
  await hold(900);
  await click(page.getByRole("button", { name: "Switch to dark theme" }));
  await hold(1600);
  await click(page.getByRole("button", { name: "Switch to light theme" }));
  await hold(500);

  // A quiz graded on the spot.
  await newChat();
  await hold(500);
  await starter("Quiz me on cellular respiration");
  await page.waitForSelector("text=Check answers", { timeout: 20000 });
  await waitIdle(page);
  await hold(400);
  const questions = page.locator(".question");
  const choose = async (q, option) => {
    const target = questions.nth(q).locator(".option").nth(option);
    await reveal(target);
    await click(target);
    await hold(250);
  };
  const answer = async (q, text) => {
    const input = questions.nth(q).locator("input");
    await reveal(input);
    await click(input);
    await page.keyboard.type(text, { delay: 85 });
    await hold(250);
  };
  await choose(0, 1);
  await choose(1, 1);
  await answer(2, "2");
  await answer(3, "Oxygen");
  await choose(4, 2);
  const checkButton = page.getByRole("button", { name: "Check answers" });
  await reveal(checkButton);
  await click(checkButton);
  await hold(400);
  await reveal(page.locator(".quiz-actions"));
  await hold(1600);

  // Teach mode: a lesson plan, then a simulation in Present view.
  await click(page.getByRole("button", { name: "Teach", exact: true }));
  await page.waitForSelector("text=What are you teaching?");
  await hold(900);
  await starter("A 50-minute lesson on Bayesian inference");
  await page.waitForSelector(".timeline", { timeout: 20000 });
  await waitIdle(page);
  await hold(500);
  await scrollChat(520, 2200);
  await hold(1200);

  await newChat();
  await hold(400);
  await starter("Classroom demo: central limit theorem");
  const clt = await frameWith(page, "#clt-count");
  await waitFinalFrame(page);
  await waitIdle(page);
  await hold(400);
  const draw = clt.getByRole("button", { name: "Draw 1,000 samples" });
  await reveal(draw, 590);
  await click(draw);
  await hold(1400);
  const present = page.getByRole("button", { name: "Present full screen" }).first();
  await reveal(present);
  await click(present);
  await page.waitForSelector(".present");
  await hold(1000);
  await clickAt(await presentPoint(page, presentFrame(page).getByRole("button", { name: "Draw 1,000 samples" })));
  await hold(2000);
  await click(page.locator(".present-bar").getByRole("button", { name: "Exit" }));
  await page.waitForSelector(".present", { state: "detached" });
  await hold(1200);

  await cdp.send("Page.stopScreencast");
  await context.close();

  // Each frame lasts until the next one; Chrome only sends frames on change.
  const list = frames.map((f, i) => `file '${f.file}'\nduration ${((frames[i + 1]?.t ?? f.t + 1.2) - f.t).toFixed(4)}`);
  list.push(`file '${frames.at(-1).file}'`);
  const listFile = path.join(work, "frames.txt");
  fs.writeFileSync(listFile, list.join("\n"));
  const input = ["-f", "concat", "-safe", "0", "-i", listFile];
  run([...input, "-vf", "fps=30,format=yuv420p", "-c:v", "libx264", "-preset", "slow", "-crf", "22", "-movflags", "+faststart", path.join(mediaDir, "tour.mp4")]);
  console.log("✓ docs/media/tour.mp4");
  // Two passes: building the palette in the same graph buffers every frame.
  const gifScale = "fps=8,scale=880:-1:flags=lanczos";
  const palette = path.join(work, "palette.png");
  run([...input, "-vf", `${gifScale},palettegen=max_colors=192:stats_mode=diff`, palette]);
  run([...input, "-i", palette, "-lavfi", `${gifScale}[v];[v][1:v]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle`, "-loop", "0", path.join(mediaDir, "tour.gif")]);
  console.log("✓ docs/media/tour.gif");
}

const server = await createServer({ root, configFile: path.join(root, "vite.config.ts"), logLevel: "warn", server: { port: PORT, strictPort: true } });
await server.listen();
const browser = await chromium.launch({ channel: "chrome", headless: true });
let failed = false;
try {
  if (!process.argv.includes("--video-only")) await screenshots(browser);
  if (!process.argv.includes("--images-only")) await tour(browser);
} catch (err) {
  failed = true;
  console.error(err);
  const failureShot = path.join(os.tmpdir(), "edu-media-failure.png");
  if (await lastPage?.screenshot({ path: failureShot }).then(() => true, () => false)) console.error(`Screen at the failure: ${failureShot}`);
} finally {
  await browser.close();
  await server.close();
  fs.rmSync(work, { recursive: true, force: true });
  process.exit(failed ? 1 : 0);
}

//! Scripted answers for demo mode. They exercise every renderer through the
//! same streaming pipeline as real model output. Content is accurate but
//! fixed; the UI labels demo mode as scripted.

use serde_json::{json, Value};

pub struct Script {
    pub lead: Option<&'static str>,
    pub tool: Option<(&'static str, Value)>,
    pub closing: &'static str,
}

pub fn script_for(presentation: &str, text: &str, teach: bool) -> Script {
    let lower = text.to_lowercase();
    match presentation {
        "simulation" => clt(),
        "interactive_explainer" | "chart" | "diagram" | "calculator" => {
            if lower.contains("limit theorem") || lower.contains("sampling") || lower.contains("clt") {
                clt()
            } else {
                fourier()
            }
        }
        "quiz" => quiz(teach),
        "flashcards" => flashcards(),
        "worked_example" => worked_example(),
        "lesson_plan" => lesson_plan(),
        "rubric" => rubric(),
        "table" => table(),
        _ => intro(teach),
    }
}

pub fn closing_only(tool_name: &str) -> &'static str {
    match tool_name {
        "render_quiz" => "Submit when you're done to see the explanations. Your score is saved to Progress.",
        "create_flashcards" => "The deck is saved. Open Review to study it with spaced repetition.",
        _ => "Let me know what you'd like to explore next.",
    }
}

fn intro(teach: bool) -> Script {
    let text = if teach {
        r#"You're in **demo mode**, so the answers here are scripted samples, not live AI. They show what the assistant can build for teaching:

- **Plan**: "Plan a 50-minute lesson introducing Bayesian inference"
- **Assess**: "Write a quiz on cellular respiration for my intro biology class"
- **Grade**: "Create a rubric for a research paper"
- **Demonstrate**: "Build a classroom demo of the central limit theorem"

Math renders inline, for example Bayes' theorem $P(H \mid E) = \frac{P(E \mid H)\,P(H)}{P(E)}$, or in display form:

$$\bar{X} \sim \mathcal{N}\!\left(\mu, \frac{\sigma^2}{n}\right)$$

To use real tutoring, add an Anthropic API key in **Settings**."#
    } else {
        r#"You're in **demo mode**, so the answers here are scripted samples, not live AI. They show the kinds of answers you can interact with:

- **Explore**: "Visualize how a Fourier series builds a square wave"
- **Simulate**: "Show me the central limit theorem with a simulation"
- **Practice**: "Quiz me on cellular respiration" or "Make flashcards for organic functional groups"
- **Work through**: "Solve the integral of x·eˣ step by step"
- **Compare**: "Compare study strategies in a table"

Math renders inline, like $e^{i\pi} + 1 = 0$, or as a display:

$$\int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi}$$

To use real tutoring, add an Anthropic API key in **Settings**."#
    };
    Script { lead: Some(text), tool: None, closing: "" }
}

fn fourier() -> Script {
    let css = r#".fx{display:grid;gap:14px;padding:2px 2px 6px}
.fx-head h2{font:600 1.08rem/1.3 var(--font-serif);margin:0 0 4px}
.fx-head p{margin:0;color:var(--color-text-secondary);font-size:.92rem;max-width:62ch}
.fx-stage{border:1px solid var(--color-border-tertiary);border-radius:var(--border-radius-lg);background:var(--color-background-secondary)}
.fx-stage canvas{display:block;width:100%;height:260px}
.fx-legend{display:flex;gap:16px;flex-wrap:wrap;font-size:.8rem;color:var(--color-text-secondary)}
.fx-swatch{display:inline-block;width:18px;height:3px;border-radius:2px;vertical-align:middle;margin-right:6px}
.fx-swatch.t{background:var(--color-text-tertiary)}.fx-swatch.s{background:var(--color-text-info)}.fx-swatch.h{background:var(--color-text-warning)}
.fx-controls{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,13rem),1fr));gap:12px 18px;align-items:end}
.fx-field{display:grid;gap:6px;font-size:.88rem}
.fx-field output{font-variant-numeric:tabular-nums;font-weight:600}
.fx-row{display:flex;gap:8px;flex-wrap:wrap;align-items:center}
.fx-formula{font-family:var(--font-mono);font-size:.84rem;overflow-x:auto;white-space:nowrap;margin:0;padding:8px 10px;border-radius:var(--border-radius-md);background:var(--color-background-secondary)}
.fx-status{min-height:1.3em;font-size:.88rem;margin:0}
.fx-note{font-size:.86rem;color:var(--color-text-secondary);border-left:3px solid var(--color-border-info);padding:2px 0 2px 10px;margin:0}
.fx-small{font-size:.82rem;color:var(--color-text-secondary)}"#;
    let html = r#"<section class="fx" aria-labelledby="fx-title">
  <header class="fx-head">
    <h2 id="fx-title">Building a wave from sines</h2>
    <p>A Fourier series writes a periodic wave as a sum of sine (or cosine) waves at whole-number multiples of one frequency. Add harmonics and watch the partial sum approach the target.</p>
  </header>
  <div class="fx-stage"><canvas id="fx-canvas" role="img" aria-label="Plot of the target wave, the newest harmonic, and the Fourier partial sum over two periods"></canvas></div>
  <div class="fx-legend" aria-hidden="true">
    <span><i class="fx-swatch t"></i>Target wave</span>
    <span><i class="fx-swatch s"></i>Partial sum</span>
    <span><i class="fx-swatch h"></i>Newest harmonic</span>
  </div>
  <div class="fx-controls">
    <label class="fx-field">Target wave
      <select id="fx-wave">
        <option value="square">Square</option>
        <option value="sawtooth">Sawtooth</option>
        <option value="triangle">Triangle</option>
      </select>
    </label>
    <label class="fx-field">Harmonics in the sum: <output id="fx-n-out" for="fx-n">3</output>
      <input id="fx-n" type="range" min="1" max="40" step="1" value="3">
    </label>
    <div class="fx-row">
      <button type="button" id="fx-play" aria-pressed="false">Add terms automatically</button>
      <button type="button" id="fx-reset">Reset</button>
    </div>
  </div>
  <p class="fx-formula" id="fx-formula">f(x) ≈ (4/π)·[ sin x + sin 3x / 3 + sin 5x / 5 ]</p>
  <p class="fx-status" id="fx-status" role="status" aria-live="polite"></p>
  <p class="fx-note" id="fx-note">Near each jump the sum overshoots. Adding terms makes the overshoot narrower but not shorter: it stays near 9% of the jump. This is the Gibbs phenomenon.</p>
  <div class="fx-row">
    <button type="button" id="fx-ask">Ask the tutor about this sum</button>
    <span class="fx-small" id="fx-ask-status" role="status"></span>
  </div>
</section>"#;
    let js_functions = r#"var FX = { n: 3, wave: 'square', timer: null };
var FX_MAX = 40;

function fxColor(name, fallback) {
  var v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v || fallback;
}

function fxTarget(wave, x) {
  var t = x - 2 * Math.PI * Math.floor((x + Math.PI) / (2 * Math.PI));
  if (wave === 'square') return t >= 0 ? 1 : -1;
  if (wave === 'sawtooth') return t / Math.PI;
  return 1 - 2 * Math.abs(t) / Math.PI;
}

function fxTerm(wave, n, x) {
  if (wave === 'square') { var k = 2 * n - 1; return (4 / Math.PI) * Math.sin(k * x) / k; }
  if (wave === 'sawtooth') { return (2 / Math.PI) * (n % 2 ? 1 : -1) * Math.sin(n * x) / n; }
  var m = 2 * n - 1; return (8 / (Math.PI * Math.PI)) * Math.cos(m * x) / (m * m);
}

function fxSum(wave, n, x) {
  var s = 0;
  for (var i = 1; i <= n; i++) s += fxTerm(wave, i, x);
  return s;
}

function fxFormula(wave, n) {
  var parts = [];
  var shown = Math.min(n, 4);
  for (var i = 1; i <= shown; i++) {
    if (wave === 'square') { var k = 2 * i - 1; parts.push(k === 1 ? 'sin x' : 'sin ' + k + 'x / ' + k); }
    else if (wave === 'sawtooth') { parts.push((i === 1 ? '' : (i % 2 ? '+ ' : '− ')) + (i === 1 ? 'sin x' : 'sin ' + i + 'x / ' + i)); }
    else { var m = 2 * i - 1; parts.push(m === 1 ? 'cos x' : 'cos ' + m + 'x / ' + (m * m)); }
  }
  var joiner = wave === 'sawtooth' ? ' ' : ' + ';
  var body = parts.join(joiner) + (n > shown ? ' + … (' + n + ' terms)' : '');
  var lead = wave === 'square' ? '(4/π)' : wave === 'sawtooth' ? '(2/π)' : '(8/π²)';
  return 'f(x) ≈ ' + lead + '·[ ' + body + ' ]';
}

function fxDraw() {
  var c = document.getElementById('fx-canvas');
  if (!c) return;
  var ctx = c.getContext('2d');
  var dpr = window.devicePixelRatio || 1;
  var w = c.clientWidth, h = c.clientHeight;
  if (!w || !h) return;
  if (c.width !== Math.round(w * dpr) || c.height !== Math.round(h * dpr)) {
    c.width = Math.round(w * dpr); c.height = Math.round(h * dpr);
  }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);
  var pad = 16, xmin = -2 * Math.PI, xmax = 2 * Math.PI, ymax = 1.45;
  function X(x) { return pad + (x - xmin) / (xmax - xmin) * (w - 2 * pad); }
  function Y(y) { return h / 2 - y / ymax * (h / 2 - pad); }
  var axis = fxColor('--color-border-secondary', '#bbb');
  var label = fxColor('--color-text-secondary', '#666');
  ctx.strokeStyle = axis; ctx.lineWidth = 1;
  ctx.beginPath(); ctx.moveTo(pad, Y(0)); ctx.lineTo(w - pad, Y(0)); ctx.stroke();
  ctx.fillStyle = label;
  ctx.font = '12px ' + getComputedStyle(document.body).fontFamily;
  ctx.textAlign = 'center';
  [-2, -1, 1, 2].forEach(function (m) {
    var px = X(m * Math.PI);
    ctx.beginPath(); ctx.moveTo(px, Y(0) - 4); ctx.lineTo(px, Y(0) + 4); ctx.stroke();
    ctx.fillText((m < 0 ? '−' : '') + (Math.abs(m) === 1 ? '' : Math.abs(m)) + 'π', px, Y(0) + 18);
  });
  var steps = Math.max(600, Math.round(w * 1.5));
  function plot(fn) {
    ctx.beginPath();
    for (var i = 0; i <= steps; i++) {
      var x = xmin + (xmax - xmin) * i / steps;
      var y = fn(x);
      if (i === 0) ctx.moveTo(X(x), Y(y)); else ctx.lineTo(X(x), Y(y));
    }
    ctx.stroke();
  }
  ctx.lineWidth = 1.5; ctx.setLineDash([5, 4]);
  ctx.strokeStyle = fxColor('--color-text-tertiary', '#999');
  plot(function (x) { return fxTarget(FX.wave, x); });
  ctx.setLineDash([]);
  ctx.globalAlpha = 0.85; ctx.lineWidth = 1.25;
  ctx.strokeStyle = fxColor('--color-text-warning', '#b7791f');
  plot(function (x) { return fxTerm(FX.wave, FX.n, x); });
  ctx.globalAlpha = 1; ctx.lineWidth = 2.25;
  ctx.strokeStyle = fxColor('--color-text-info', '#2d4fd6');
  plot(function (x) { return fxSum(FX.wave, FX.n, x); });
}

function fxPeak() {
  var peak = -Infinity;
  for (var i = 0; i <= 4000; i++) {
    var x = -Math.PI + 2 * Math.PI * i / 4000;
    var y = fxSum(FX.wave, FX.n, x);
    if (y > peak) peak = y;
  }
  return peak;
}

function fxUpdate() {
  if (!Number.isFinite(FX.n) || FX.n < 1 || FX.n > FX_MAX) FX.n = 3;
  document.getElementById('fx-n').value = String(FX.n);
  document.getElementById('fx-n-out').textContent = String(FX.n);
  document.getElementById('fx-formula').textContent = fxFormula(FX.wave, FX.n);
  var note = document.getElementById('fx-note');
  note.textContent = FX.wave === 'triangle'
    ? 'The triangle wave has no jumps, and its terms shrink like 1/k², so a few harmonics already match it closely.'
    : 'Near each jump the sum overshoots. Adding terms makes the overshoot narrower but not shorter: it stays near 9% of the jump. This is the Gibbs phenomenon.';
  document.getElementById('fx-status').textContent =
    FX.n + (FX.n === 1 ? ' harmonic' : ' harmonics') + '. Highest point of the partial sum: ' + fxPeak().toFixed(3) + ' (the target peaks at 1.000).';
  fxDraw();
}

function fxStop() {
  if (FX.timer) { clearInterval(FX.timer); FX.timer = null; }
  var b = document.getElementById('fx-play');
  b.setAttribute('aria-pressed', 'false');
  b.textContent = 'Add terms automatically';
}

function fxToggle() {
  if (FX.timer) { fxStop(); return; }
  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) { FX.n = FX_MAX; fxUpdate(); return; }
  if (FX.n >= FX_MAX) FX.n = 1;
  var b = document.getElementById('fx-play');
  b.setAttribute('aria-pressed', 'true');
  b.textContent = 'Pause';
  FX.timer = setInterval(function () {
    if (FX.n >= FX_MAX) { fxStop(); return; }
    FX.n += 1; fxUpdate();
  }, 320);
}

function fxReset() {
  fxStop();
  FX.n = 3; FX.wave = 'square';
  document.getElementById('fx-wave').value = 'square';
  fxUpdate();
}

async function fxAsk(button) {
  var status = document.getElementById('fx-ask-status');
  var question = FX.wave === 'triangle'
    ? 'In the Fourier demo I am looking at a triangle wave with ' + FX.n + ' harmonics. Why does it converge so much faster than the square wave?'
    : 'In the Fourier demo I am looking at a ' + FX.wave + ' wave with ' + FX.n + ' harmonics. Why does the partial sum overshoot near the jumps, and why does adding more terms not remove the overshoot?';
  button.disabled = true;
  status.textContent = 'Sending…';
  try {
    await edu.sendPrompt({ text: question });
    status.textContent = 'Sent to the chat.';
  } catch (err) {
    status.textContent = 'Could not send. Type your question in the chat instead.';
  } finally {
    button.disabled = false;
  }
}

function fxInit() {
  var slider = document.getElementById('fx-n');
  var wave = document.getElementById('fx-wave');
  slider.addEventListener('input', function () { FX.n = slider.valueAsNumber; fxUpdate(); });
  wave.addEventListener('change', function () { FX.wave = wave.value; fxUpdate(); });
  document.getElementById('fx-play').addEventListener('click', fxToggle);
  document.getElementById('fx-reset').addEventListener('click', fxReset);
  document.getElementById('fx-ask').addEventListener('click', function () { fxAsk(this); });
  new ResizeObserver(function () { fxDraw(); }).observe(document.getElementById('fx-canvas'));
  window.addEventListener('themechange', fxDraw);
  fxUpdate();
}"#;
    Script {
        lead: Some("Here's a Fourier series you can build up one harmonic at a time."),
        tool: Some((
            "generate_interactive",
            json!({
                "title": "Fourier series: building a wave from sines",
                "initialHeight": 640,
                "placeholderMessages": ["Setting up the plot", "Adding harmonics", "Wiring the controls"],
                "css": css,
                "html": html,
                "jsFunctions": js_functions,
                "jsExpressions": ["fxInit();"]
            }),
        )),
        closing: "Start at 1 harmonic, then slide to 40. The sum improves everywhere except right at the jumps, where the overshoot narrows but stays about the same height. Switch to the triangle wave to see why smooth waves converge faster.",
    }
}

fn clt() -> Script {
    let css = r#".clt{display:grid;gap:14px;padding:2px 2px 6px}
.clt h2{font:600 1.08rem/1.3 var(--font-serif);margin:0 0 4px}
.clt-lede{margin:0;color:var(--color-text-secondary);font-size:.92rem;max-width:62ch}
.clt-grid{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,2fr);gap:12px}
@media (max-width:620px){.clt-grid{grid-template-columns:1fr}}
.clt-panel{border:1px solid var(--color-border-tertiary);border-radius:var(--border-radius-lg);background:var(--color-background-secondary);padding:8px 10px 6px}
.clt-panel h3{font-size:.8rem;font-weight:600;margin:0 0 4px;color:var(--color-text-secondary)}
.clt-panel canvas{display:block;width:100%;height:190px}
.clt-controls{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,13rem),1fr));gap:12px 18px;align-items:end}
.clt-field{display:grid;gap:6px;font-size:.88rem}
.clt-field output{font-weight:600;font-variant-numeric:tabular-nums}
.clt-row{display:flex;gap:8px;flex-wrap:wrap;align-items:center}
.clt-stats{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,10rem),1fr));gap:8px;margin:0}
.clt-stats div{padding:8px 10px;border-radius:var(--border-radius-md);background:var(--color-background-secondary)}
.clt-stats dt{font-size:.78rem;color:var(--color-text-secondary)}
.clt-stats dd{margin:2px 0 0;font-size:1rem;font-weight:600;font-variant-numeric:tabular-nums}
.clt-note{font-size:.84rem;color:var(--color-text-secondary);margin:0}
.clt-small{font-size:.82rem;color:var(--color-text-secondary)}"#;
    let html = r#"<section class="clt" aria-labelledby="clt-title">
  <header>
    <h2 id="clt-title">The sampling distribution of the mean</h2>
    <p class="clt-lede">Draw many samples of size n from a population and record each sample's mean. The central limit theorem predicts that the means pile up in a bell shape centred on μ with spread σ/√n, whatever the population's shape.</p>
  </header>
  <div class="clt-grid">
    <div class="clt-panel"><h3 id="clt-pop-label">Population</h3><canvas id="clt-pop" role="img" aria-labelledby="clt-pop-label"></canvas></div>
    <div class="clt-panel"><h3 id="clt-means-label">Sample means (histogram) and the CLT prediction (curve)</h3><canvas id="clt-means" role="img" aria-labelledby="clt-means-label"></canvas></div>
  </div>
  <div class="clt-controls">
    <label class="clt-field">Population shape
      <select id="clt-dist">
        <option value="exponential">Right-skewed (exponential, λ = 1)</option>
        <option value="uniform">Flat (uniform on 0 to 1)</option>
        <option value="bimodal">Two peaks (bimodal mixture)</option>
      </select>
    </label>
    <label class="clt-field">Sample size n: <output id="clt-n-out" for="clt-n">5</output>
      <input id="clt-n" type="range" min="1" max="60" step="1" value="5">
    </label>
  </div>
  <div class="clt-row">
    <button type="button" data-draw="1">Draw 1 sample</button>
    <button type="button" data-draw="100">Draw 100 samples</button>
    <button type="button" data-draw="1000">Draw 1,000 samples</button>
    <button type="button" id="clt-reset">Reset</button>
  </div>
  <dl class="clt-stats">
    <div><dt>Sample means drawn</dt><dd id="clt-count">0</dd></div>
    <div><dt>Mean of the means (μ = <span id="clt-mu">1</span>)</dt><dd id="clt-mean">–</dd></div>
    <div><dt>SD of the means (σ/√n = <span id="clt-se">0.447</span>)</dt><dd id="clt-sd">–</dd></div>
  </dl>
  <p class="clt-note" role="status" aria-live="polite" id="clt-status">Choose a population and press a Draw button.</p>
  <p class="clt-note">Results are simulated with your browser's random number generator, so each run differs slightly.</p>
  <div class="clt-row">
    <button type="button" id="clt-ask">Ask the tutor about these results</button>
    <span class="clt-small" id="clt-ask-status" role="status"></span>
  </div>
</section>"#;
    let js_functions = r#"var CLT = { dist: 'exponential', n: 5, means: [] };
var CLT_BINS = 40;
var CLT_POP = {
  exponential: { mu: 1, sigma: 1, lo: 0, hi: 4, pdf: function (x) { return x < 0 ? 0 : Math.exp(-x); }, draw: function () { return -Math.log(1 - Math.random()); } },
  uniform: { mu: 0.5, sigma: Math.sqrt(1 / 12), lo: 0, hi: 1, pdf: function (x) { return x >= 0 && x <= 1 ? 1 : 0; }, draw: function () { return Math.random(); } },
  bimodal: { mu: 0.5, sigma: Math.sqrt(0.08 * 0.08 + 0.25 * 0.25), lo: 0, hi: 1,
    pdf: function (x) { return 0.5 * cltNormal(x, 0.25, 0.08) + 0.5 * cltNormal(x, 0.75, 0.08); },
    draw: function () { return (Math.random() < 0.5 ? 0.25 : 0.75) + 0.08 * cltGauss(); } }
};

function cltGauss() {
  var u = 1 - Math.random(), v = Math.random();
  return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v);
}

function cltNormal(x, mu, sd) {
  return Math.exp(-0.5 * Math.pow((x - mu) / sd, 2)) / (sd * Math.sqrt(2 * Math.PI));
}

function cltColor(name, fallback) {
  var v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v || fallback;
}

function cltCanvas(id) {
  var c = document.getElementById(id);
  var dpr = window.devicePixelRatio || 1;
  var w = c.clientWidth, h = c.clientHeight;
  if (c.width !== Math.round(w * dpr) || c.height !== Math.round(h * dpr)) { c.width = Math.round(w * dpr); c.height = Math.round(h * dpr); }
  var ctx = c.getContext('2d');
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);
  return { ctx: ctx, w: w, h: h };
}

function cltAxis(g, lo, hi, pad) {
  var ctx = g.ctx;
  ctx.strokeStyle = cltColor('--color-border-secondary', '#bbb');
  ctx.lineWidth = 1;
  ctx.beginPath(); ctx.moveTo(pad, g.h - 20); ctx.lineTo(g.w - pad, g.h - 20); ctx.stroke();
  ctx.fillStyle = cltColor('--color-text-secondary', '#666');
  ctx.font = '11px ' + getComputedStyle(document.body).fontFamily;
  ctx.textAlign = 'center';
  for (var i = 0; i <= 4; i++) {
    var v = lo + (hi - lo) * i / 4;
    ctx.fillText(v.toFixed(hi - lo > 2 ? 0 : 2), pad + (g.w - 2 * pad) * i / 4, g.h - 6);
  }
}

function cltDrawPopulation() {
  var p = CLT_POP[CLT.dist], g = cltCanvas('clt-pop'), pad = 12;
  if (!g.w) return;
  var top = 0;
  for (var i = 0; i <= 200; i++) top = Math.max(top, p.pdf(p.lo + (p.hi - p.lo) * i / 200));
  var ctx = g.ctx;
  ctx.beginPath();
  for (var j = 0; j <= 200; j++) {
    var x = p.lo + (p.hi - p.lo) * j / 200;
    var px = pad + (g.w - 2 * pad) * j / 200;
    var py = (g.h - 20) - p.pdf(x) / top * (g.h - 36);
    if (j === 0) ctx.moveTo(px, g.h - 20);
    ctx.lineTo(px, py);
  }
  ctx.lineTo(g.w - pad, g.h - 20);
  ctx.closePath();
  ctx.globalAlpha = 0.25; ctx.fillStyle = cltColor('--color-text-success', '#3b6d11'); ctx.fill();
  ctx.globalAlpha = 1; ctx.strokeStyle = cltColor('--color-text-success', '#3b6d11'); ctx.lineWidth = 1.5; ctx.stroke();
  cltAxis(g, p.lo, p.hi, pad);
}

function cltDrawMeans() {
  var p = CLT_POP[CLT.dist], g = cltCanvas('clt-means'), pad = 12;
  if (!g.w) return;
  var se = p.sigma / Math.sqrt(CLT.n);
  var lo = p.lo, hi = p.hi, width = (hi - lo) / CLT_BINS;
  var counts = new Array(CLT_BINS).fill(0);
  CLT.means.forEach(function (m) {
    var b = Math.floor((m - lo) / width);
    counts[Math.min(CLT_BINS - 1, Math.max(0, b))] += 1;
  });
  var peakCurve = CLT.means.length * width * cltNormal(p.mu, p.mu, se);
  var top = Math.max(1, peakCurve, Math.max.apply(null, counts));
  var ctx = g.ctx, plotW = g.w - 2 * pad, plotH = g.h - 36;
  ctx.fillStyle = cltColor('--color-text-info', '#2d4fd6');
  ctx.globalAlpha = 0.45;
  counts.forEach(function (c, i) {
    var bh = c / top * plotH;
    ctx.fillRect(pad + i * plotW / CLT_BINS + 0.5, g.h - 20 - bh, plotW / CLT_BINS - 1, bh);
  });
  ctx.globalAlpha = 1;
  if (CLT.means.length > 0) {
    ctx.strokeStyle = cltColor('--color-text-primary', '#222');
    ctx.lineWidth = 2;
    ctx.beginPath();
    for (var j = 0; j <= 300; j++) {
      var x = lo + (hi - lo) * j / 300;
      var y = CLT.means.length * width * cltNormal(x, p.mu, se);
      var px = pad + plotW * j / 300, py = g.h - 20 - y / top * plotH;
      if (j === 0) ctx.moveTo(px, py); else ctx.lineTo(px, py);
    }
    ctx.stroke();
  }
  cltAxis(g, lo, hi, pad);
}

function cltStats() {
  var p = CLT_POP[CLT.dist], k = CLT.means.length;
  document.getElementById('clt-count').textContent = k.toLocaleString();
  document.getElementById('clt-mu').textContent = String(+p.mu.toFixed(3));
  document.getElementById('clt-se').textContent = (p.sigma / Math.sqrt(CLT.n)).toFixed(3);
  if (k < 2) {
    document.getElementById('clt-mean').textContent = k === 1 ? CLT.means[0].toFixed(3) : '–';
    document.getElementById('clt-sd').textContent = '–';
    return;
  }
  var mean = CLT.means.reduce(function (a, b) { return a + b; }, 0) / k;
  var v = CLT.means.reduce(function (a, b) { return a + (b - mean) * (b - mean); }, 0) / (k - 1);
  document.getElementById('clt-mean').textContent = mean.toFixed(3);
  document.getElementById('clt-sd').textContent = Math.sqrt(v).toFixed(3);
}

function cltRender() { cltDrawPopulation(); cltDrawMeans(); cltStats(); }

function cltDraw(times) {
  var p = CLT_POP[CLT.dist];
  if (!Number.isFinite(times) || times < 1) return;
  for (var t = 0; t < times; t++) {
    var s = 0;
    for (var i = 0; i < CLT.n; i++) s += p.draw();
    CLT.means.push(s / CLT.n);
  }
  if (CLT.means.length > 50000) CLT.means = CLT.means.slice(-50000);
  document.getElementById('clt-status').textContent =
    'Drew ' + times.toLocaleString() + (times === 1 ? ' sample' : ' samples') + ' of size ' + CLT.n + '. ' +
    (CLT.means.length >= 300 ? 'Compare the histogram with the curve.' : 'Draw more samples to see the shape emerge.');
  cltRender();
}

function cltReset(message) {
  CLT.means = [];
  document.getElementById('clt-status').textContent = message || 'Cleared. Press a Draw button to start again.';
  cltRender();
}

async function cltAsk(button) {
  var status = document.getElementById('clt-ask-status');
  var sd = document.getElementById('clt-sd').textContent;
  var se = document.getElementById('clt-se').textContent;
  var label = document.getElementById('clt-dist').selectedOptions[0].textContent;
  button.disabled = true;
  status.textContent = 'Sending…';
  try {
    await edu.sendPrompt({ text: 'In the CLT simulation I used the ' + label + ' population with n = ' + CLT.n + ' and drew ' + CLT.means.length + ' sample means. Their SD was ' + sd + ' versus the predicted σ/√n = ' + se + '. Why does the histogram look bell-shaped even though the population is not, and how large does n need to be?' });
    status.textContent = 'Sent to the chat.';
  } catch (err) {
    status.textContent = 'Could not send. Type your question in the chat instead.';
  } finally {
    button.disabled = false;
  }
}

function cltInit() {
  var n = document.getElementById('clt-n'), dist = document.getElementById('clt-dist');
  n.addEventListener('input', function () {
    var v = n.valueAsNumber;
    CLT.n = Number.isFinite(v) && v >= 1 && v <= 60 ? v : 5;
    document.getElementById('clt-n-out').textContent = String(CLT.n);
    cltReset('Sample size changed to ' + CLT.n + '. Previous means were cleared because they used a different n.');
  });
  dist.addEventListener('change', function () { CLT.dist = dist.value; cltReset('Population changed. Press a Draw button.'); });
  document.querySelectorAll('[data-draw]').forEach(function (b) {
    b.addEventListener('click', function () { cltDraw(Number(b.getAttribute('data-draw'))); });
  });
  document.getElementById('clt-reset').addEventListener('click', function () { cltReset(); });
  document.getElementById('clt-ask').addEventListener('click', function () { cltAsk(this); });
  var ro = new ResizeObserver(function () { cltDrawPopulation(); cltDrawMeans(); });
  ro.observe(document.getElementById('clt-means'));
  window.addEventListener('themechange', cltRender);
  cltRender();
}"#;
    Script {
        lead: Some("This simulation lets you test the central limit theorem yourself."),
        tool: Some((
            "generate_interactive",
            json!({
                "title": "Central limit theorem simulation",
                "initialHeight": 720,
                "placeholderMessages": ["Preparing the populations", "Setting up histograms", "Connecting the sampler"],
                "css": css,
                "html": html,
                "jsFunctions": js_functions,
                "jsExpressions": ["cltInit();"]
            }),
        )),
        closing: "Try the skewed population with n = 1: the means look just like the population. Then raise n to 30 and draw 1,000 samples. The histogram becomes symmetric and narrows to match σ/√n.",
    }
}

fn quiz(teach: bool) -> Script {
    Script {
        lead: Some(if teach {
            "Here's a short assessment with an answer key. Each question is tagged with its Bloom level."
        } else {
            "Let's check your understanding of cellular respiration. Take your time."
        }),
        tool: Some((
            "render_quiz",
            json!({
                "title": "Cellular respiration check-in",
                "topic": "Cellular respiration",
                "purpose": if teach { "assessment" } else { "practice" },
                "questions": [
                    {
                        "type": "multiple_choice",
                        "prompt": "Where in a eukaryotic cell does glycolysis take place?",
                        "options": ["Mitochondrial matrix", "Cytosol", "Inner mitochondrial membrane", "Nucleus"],
                        "answer_index": 1,
                        "option_feedback": [
                            "The matrix hosts pyruvate oxidation and the citric acid cycle, which come after glycolysis.",
                            "Correct. Glycolysis needs no organelle and runs in the cytosol.",
                            "The inner membrane holds the electron transport chain and ATP synthase.",
                            "The nucleus stores DNA; it does not run metabolic pathways like glycolysis."
                        ],
                        "explanation": "Glycolysis splits one glucose into two pyruvate in the cytosol, producing a net 2 ATP and 2 NADH.",
                        "bloom": "remember"
                    },
                    {
                        "type": "true_false",
                        "prompt": "Oxygen is consumed directly during the citric acid cycle.",
                        "answer_bool": false,
                        "explanation": "False. Oxygen is used only at the end of the electron transport chain as the final electron acceptor. The citric acid cycle stops without oxygen only indirectly, because NAD⁺ is not regenerated.",
                        "bloom": "understand"
                    },
                    {
                        "type": "numeric",
                        "prompt": "How many molecules of pyruvate are produced from one molecule of glucose in glycolysis?",
                        "answer_number": 2,
                        "tolerance": 0,
                        "unit": "pyruvate",
                        "explanation": "Glucose (6 carbons) is split into two 3-carbon pyruvate molecules.",
                        "bloom": "remember"
                    },
                    {
                        "type": "short_answer",
                        "prompt": "What molecule is the final electron acceptor in the electron transport chain?",
                        "accepted_answers": ["oxygen", "o2", "molecular oxygen", "dioxygen"],
                        "explanation": "Oxygen accepts electrons and protons at the end of the chain, forming water.",
                        "bloom": "remember"
                    },
                    {
                        "type": "multiple_choice",
                        "prompt": "A poison blocks ATP synthase but leaves the electron transport chain intact. What happens first in the mitochondria?",
                        "options": [
                            "The proton gradient across the inner membrane grows",
                            "Glycolysis stops immediately",
                            "Oxygen consumption increases sharply",
                            "The matrix becomes more acidic than the intermembrane space"
                        ],
                        "answer_index": 0,
                        "option_feedback": [
                            "Correct. Protons keep being pumped out but can no longer flow back through ATP synthase, so the gradient builds up.",
                            "Glycolysis is in the cytosol and continues, at least at first.",
                            "Oxygen use actually falls as the growing gradient slows electron transport.",
                            "Protons are pumped out of the matrix, so the intermembrane space becomes more acidic, not the matrix."
                        ],
                        "explanation": "With ATP synthase blocked, pumping continues briefly and the gradient builds until it opposes further pumping. This is why electron transport and ATP synthesis are called coupled.",
                        "bloom": "analyze"
                    }
                ]
            }),
        )),
        closing: "Submit when you're done to see the explanations. Your score is saved to Progress.",
    }
}

fn flashcards() -> Script {
    let cards = [
        ("Hydroxyl group", "–OH. Defines alcohols (e.g. ethanol). Polar; forms hydrogen bonds.", "Think of water's –OH."),
        ("Aldehyde", "Carbonyl at the end of a chain: –CHO (e.g. formaldehyde).", "C=O at a terminal carbon."),
        ("Ketone", "Carbonyl within a chain: R–CO–R′ (e.g. acetone).", "C=O flanked by two carbons."),
        ("Carboxyl group", "–COOH. Defines carboxylic acids; donates H⁺, so it is acidic.", "Carbonyl + hydroxyl on one carbon."),
        ("Amino group", "–NH₂. Defines amines; accepts H⁺, so it is basic.", "Nitrogen with two hydrogens."),
        ("Ester", "R–COO–R′. Formed from an acid and an alcohol; many have fruity odors.", "Condensation of –COOH and –OH."),
        ("Ether", "R–O–R′. An oxygen bridging two carbon groups (e.g. diethyl ether).", "Oxygen between two carbons, no H."),
        ("Amide", "R–CO–NR₂. The linkage in peptide bonds between amino acids.", "Carbonyl bonded to nitrogen."),
    ];
    let cards: Vec<Value> = cards
        .iter()
        .map(|(f, b, h)| json!({ "front": f, "back": b, "hint": h }))
        .collect();
    Script {
        lead: Some("Here's a deck of the core functional groups. Name the structure before you flip."),
        tool: Some((
            "create_flashcards",
            json!({
                "deck_title": "Organic functional groups",
                "description": "Core functional groups with their structures and properties.",
                "cards": cards
            }),
        )),
        closing: "The deck is saved. Open Review to study it with spaced repetition: cards you find hard come back sooner.",
    }
}

fn worked_example() -> Script {
    Script {
        lead: Some("Let's work through this with integration by parts. Try each step yourself before you reveal it."),
        tool: Some((
            "render_worked_example",
            json!({
                "title": "Integration by parts",
                "problem": "Evaluate $\\displaystyle\\int x\\,e^{x}\\,dx$.",
                "steps": [
                    { "title": "Choose u and dv", "detail": "Integration by parts uses $\\int u\\,dv = uv - \\int v\\,du$. Pick $u$ so that it gets simpler when differentiated: here $u = x$ and $dv = e^{x}\\,dx$." },
                    { "title": "Differentiate u and integrate dv", "detail": "$du = dx$ and $v = \\int e^{x}\\,dx = e^{x}$." },
                    { "title": "Apply the formula", "detail": "$\\int x\\,e^{x}\\,dx = x\\,e^{x} - \\int e^{x}\\,dx$. The remaining integral is easier than the original, which confirms the choice of $u$." },
                    { "title": "Finish the integral", "detail": "$x\\,e^{x} - e^{x} + C = e^{x}(x - 1) + C$." },
                    { "title": "Check by differentiating", "detail": "$\\frac{d}{dx}\\left[e^{x}(x-1)\\right] = e^{x}(x-1) + e^{x} = x\\,e^{x}$. ✓" }
                ],
                "answer": "$\\displaystyle\\int x\\,e^{x}\\,dx = e^{x}(x-1) + C$",
                "practice_problem": { "prompt": "Evaluate $\\displaystyle\\int x\\cos x\\,dx$.", "answer": "$x\\sin x + \\cos x + C$" }
            }),
        )),
        closing: "Try the practice problem before revealing the answer. It uses the same choice of u.",
    }
}

fn lesson_plan() -> Script {
    Script {
        lead: Some("Here's a 50-minute session built around the base-rate fallacy, which makes Bayes' theorem concrete."),
        tool: Some((
            "render_lesson_plan",
            json!({
                "title": "Introducing Bayesian inference",
                "level": "Upper-level undergraduate",
                "duration_minutes": 50,
                "overview": "Students meet Bayes' theorem through a diagnostic-test puzzle, formalize it, and practise updating beliefs with natural frequencies.",
                "objectives": [
                    { "text": "Explain how the prior, likelihood, and posterior relate in Bayes' theorem", "bloom": "understand" },
                    { "text": "Compute a posterior probability for a diagnostic-test scenario", "bloom": "apply" },
                    { "text": "Critique a conclusion that ignores the base rate", "bloom": "evaluate" }
                ],
                "prerequisites": ["Conditional probability", "The multiplication rule"],
                "materials": ["Slides or whiteboard", "Polling tool or index cards", "Scenario handout (three cases)"],
                "sections": [
                    { "title": "Warm-up poll", "minutes": 5, "kind": "warm-up", "description": "A disease affects 1% of people. A test is 99% sensitive with a 5% false-positive rate. Someone tests positive: how likely are they to have the disease? Collect estimates without discussion.", "instructor_notes": "Most estimates land near 95%. Keep the distribution on screen for later." },
                    { "title": "From conditional probability to Bayes", "minutes": 12, "kind": "lecture", "description": "Derive $P(H\\mid E) = P(E\\mid H)P(H)/P(E)$ from the definition of conditional probability, then redo the poll with natural frequencies (10,000 people: 100 sick, 99 true positives, 495 false positives).", "instructor_notes": "The answer is about 17%. Name the base-rate fallacy explicitly." },
                    { "title": "Live demonstration", "minutes": 8, "kind": "demonstration", "description": "Show an interactive frequency tree and vary the prevalence from 0.1% to 20% to see how the posterior responds.", "instructor_notes": "Ask the assistant to build the demo before class and use Present mode." },
                    { "title": "Think–pair–share scenarios", "minutes": 15, "kind": "activity", "description": "Pairs work through three scenarios (spam filtering, a courtroom DNA match, a screening programme), computing each posterior and writing one sentence on what the prior represents.", "instructor_notes": "Circulate and listen for pairs who confuse P(E|H) with P(H|E)." },
                    { "title": "Exit ticket", "minutes": 7, "kind": "assessment", "description": "Individually: compute a posterior for a new prevalence and explain in two sentences why the answer changed." },
                    { "title": "Wrap-up", "minutes": 3, "kind": "wrap-up", "description": "Return to the warm-up distribution and connect the gap to the base rate. Preview conjugate priors." }
                ],
                "assessment": "The exit ticket checks objective 2 directly and objective 3 through the written explanation. Scenario worksheets give formative evidence for objective 1.",
                "differentiation": "Offer a partially completed frequency tree for students who need scaffolding. Extension: derive the posterior odds form and interpret the likelihood ratio.",
                "follow_up": "Problem set: four applied Bayes problems, including one that asks students to choose and justify a prior."
            }),
        )),
        closing: "The timings add up to 50 minutes. Save it to the Library, or export it as Markdown for your course site.",
    }
}

fn rubric() -> Script {
    let levels = json!([
        { "label": "Exemplary", "points": 4 },
        { "label": "Proficient", "points": 3 },
        { "label": "Developing", "points": 2 },
        { "label": "Beginning", "points": 1 }
    ]);
    Script {
        lead: Some("Here's an analytic rubric with observable descriptors at each level."),
        tool: Some((
            "render_rubric",
            json!({
                "title": "Research paper rubric",
                "assignment": "8–10 page argumentative research paper",
                "levels": levels,
                "criteria": [
                    { "name": "Thesis and argument", "weight_percent": 25, "descriptors": [
                        "Precise, arguable thesis; every section advances it.",
                        "Clear thesis; most sections support it.",
                        "Thesis is broad or descriptive; support is uneven.",
                        "No identifiable thesis, or the paper drifts from it."
                    ]},
                    { "name": "Use of evidence", "weight_percent": 25, "descriptors": [
                        "Well-chosen scholarly sources, integrated and interpreted.",
                        "Relevant sources that mostly support claims.",
                        "Sources are thin or loosely connected to claims.",
                        "Little or no credible evidence."
                    ]},
                    { "name": "Analysis", "weight_percent": 20, "descriptors": [
                        "Weighs counterarguments and explains why evidence matters.",
                        "Explains evidence; addresses some counterarguments.",
                        "Mostly summarizes sources with little analysis.",
                        "Summary only, or claims go unexplained."
                    ]},
                    { "name": "Organization", "weight_percent": 15, "descriptors": [
                        "Logical structure with purposeful transitions.",
                        "Clear structure; occasional abrupt shifts.",
                        "Structure is hard to follow in places.",
                        "No discernible organization."
                    ]},
                    { "name": "Citation and academic integrity", "weight_percent": 10, "descriptors": [
                        "All sources cited accurately in the required style.",
                        "Sources cited with minor style errors.",
                        "Inconsistent or incomplete citations.",
                        "Missing citations for borrowed ideas or words."
                    ]},
                    { "name": "Mechanics and style", "weight_percent": 5, "descriptors": [
                        "Clear, concise prose with no distracting errors.",
                        "Generally clear; a few errors.",
                        "Errors sometimes obscure meaning.",
                        "Frequent errors impede reading."
                    ]}
                ],
                "notes": "Weighted score = Σ (level points ÷ 4 × weight). Share the rubric with students when you assign the paper."
            }),
        )),
        closing: "The weights total 100%. Export it as CSV or Markdown to bring it into your LMS.",
    }
}

fn table() -> Script {
    Script {
        lead: Some("Here's how common study strategies compare in a major review of the research."),
        tool: Some((
            "render_table",
            json!({
                "title": "Study strategies compared",
                "columns": ["Strategy", "What it involves", "Rated utility", "Try this week"],
                "rows": [
                    ["Practice testing", "Retrieving material from memory: self-quizzing, flashcards", "High", "Close your notes after each lecture and write down what you remember"],
                    ["Distributed practice", "Spreading study sessions over time instead of cramming", "High", "Schedule three short reviews across the week"],
                    ["Interleaved practice", "Mixing different problem types within one session", "Moderate", "Mix problems from two chapters in one set"],
                    ["Elaborative interrogation", "Asking why a stated fact is true", "Moderate", "Add a \"why?\" next to each key fact in your notes"],
                    ["Self-explanation", "Explaining how new information connects to what you know", "Moderate", "Talk through a worked example step by step"],
                    ["Rereading", "Reading the material again", "Low", "Replace one reread with a self-quiz"],
                    ["Highlighting", "Marking passages while reading", "Low", "Highlight after recalling, not during a first read"]
                ],
                "source": "Utility ratings: Dunlosky, Rawson, Marsh, Nathan & Willingham (2013), Psychological Science in the Public Interest, 14(1), 4–58. The suggestions in the last column are illustrative."
            }),
        )),
        closing: "The two high-utility strategies combine well: spaced self-quizzing is what the Review tab is built for.",
    }
}

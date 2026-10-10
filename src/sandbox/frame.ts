// Documents for sandboxed generated UIs.
//
// Frames are <iframe sandbox="allow-scripts" srcdoc=...>, which gives them an
// opaque origin: no storage, no access to the app, no Tauri IPC. Each document
// carries its own CSP limiting scripts to four CDNs. The bootstrap below is
// the only channel to the host: resize reports, error reports, and two
// validated bridge calls (sendPrompt, openLink).
import idiomorphSource from "idiomorph/dist/idiomorph.min.js?raw";
import { DESIGN_SYSTEM_CSS } from "./designSystem";
import type { Resolved } from "../lib/theme";
import type { FrameA11y } from "../lib/a11y";

const CDNS = "https://esm.sh https://cdn.jsdelivr.net https://unpkg.com https://cdnjs.cloudflare.com";

export const CSP = [
  "default-src 'none'",
  `script-src 'unsafe-inline' 'unsafe-eval' ${CDNS}`,
  "style-src 'unsafe-inline' https://cdn.jsdelivr.net https://esm.sh",
  "font-src data: https://cdn.jsdelivr.net https://esm.sh",
  "img-src data: blob: https://upload.wikimedia.org",
  `connect-src ${CDNS}`,
  "media-src data: blob:",
  "worker-src blob:",
].join("; ");

export const IMPORT_MAP = {
  imports: {
    three: "https://esm.sh/three@0.186.1",
    "three/": "https://esm.sh/three@0.186.1/",
    gsap: "https://esm.sh/gsap@3.15.0",
    "gsap/": "https://esm.sh/gsap@3.15.0/",
    d3: "https://esm.sh/d3@7.9.0",
    "chart.js": "https://esm.sh/chart.js@4.5.1",
    "chart.js/": "https://esm.sh/chart.js@4.5.1/",
    katex: "https://esm.sh/katex@0.19.0",
    mathjs: "https://esm.sh/mathjs@15.2.0",
  },
};

const KATEX_CSS = "https://cdn.jsdelivr.net/npm/katex@0.19.0/dist/katex.min.css";

/** Keeps generated text from closing the element it is embedded in. */
export const escapeStyle = (css: string) => css.replace(/<\/style/gi, "<\\/style");
export const escapeScript = (js: string) => js.replace(/<\/script/gi, "<\\/script").replace(/<!--/g, "<\\!--");

// Shared by both frame kinds. Messages from the frame carry __edu and the
// document's nonce, so a frame that navigates away loses the bridge; messages
// from the host carry __eduHost and are accepted only from the parent window.
const BOOTSTRAP_SOURCE = String.raw`(function () {
  // Remove this script so generated code can't read the nonce from the DOM.
  if (document.currentScript) document.currentScript.remove();
  var last = 0;
  // The app's "Reduce motion" setting also answers prefers-reduced-motion
  // queries, so generated code that checks it behaves accordingly.
  var nativeMatchMedia = window.matchMedia.bind(window);
  window.matchMedia = function (query) {
    if (/prefers-reduced-motion\s*:\s*reduce/.test(String(query)) && document.documentElement.getAttribute("data-reduce-motion") === "true") {
      return { matches: true, media: String(query), onchange: null,
        addListener: function () {}, removeListener: function () {},
        addEventListener: function () {}, removeEventListener: function () {},
        dispatchEvent: function () { return false; } };
    }
    return nativeMatchMedia(query);
  };
  var A11Y = { "data-contrast": ["more", "normal"], "data-reduce-motion": ["true", "false"],
               "data-spacing": ["relaxed", "normal"], "data-font": ["hyperlegible", "serif", "system"] };
  var NONCE = "__EDU_NONCE__";
  function post(msg) { msg.__edu = true; msg.nonce = NONCE; try { parent.postMessage(msg, "*"); } catch (e) {} }
  function measure() {
    var b = document.body;
    if (!b) return;
    var cs = getComputedStyle(b);
    var h = Math.ceil(b.scrollHeight + (parseFloat(cs.marginTop) || 0) + (parseFloat(cs.marginBottom) || 0));
    if (h !== last) { last = h; post({ type: "resize", height: h }); }
  }
  window.__eduMeasure = measure;
  window.addEventListener("error", function (e) {
    post({ type: "error", message: String((e && e.message) || "Script error").slice(0, 500) });
  });
  window.addEventListener("unhandledrejection", function (e) {
    var r = e && e.reason;
    post({ type: "error", message: String((r && r.message) || r || "Unhandled promise rejection").slice(0, 500) });
  });
  // Key presses inside the frame never reach the app, so pass Escape on (it
  // closes Present view and menus) unless the interactive handled it itself.
  // Checked after dispatch so handlers registered later still count.
  window.addEventListener("keydown", function (e) {
    if (e.key !== "Escape" || !e.isTrusted) return;
    setTimeout(function () { if (!e.defaultPrevented) post({ type: "escape" }); }, 0);
  });
  var pending = {}, seq = 0;
  function call(method, args) {
    return new Promise(function (resolve, reject) {
      var id = ++seq;
      pending[id] = { resolve: resolve, reject: reject };
      post({ type: "bridge", id: id, method: method, args: args });
      setTimeout(function () {
        if (pending[id]) { delete pending[id]; reject(new Error("The app did not respond. Try again.")); }
      }, 20000);
    });
  }
  window.edu = Object.freeze({
    sendPrompt: function (args) {
      var ua = navigator.userActivation;
      if (ua && !ua.isActive) return Promise.reject(new Error("sendPrompt can only be called from a click or key press."));
      var text = args && typeof args.text === "string" ? args.text.trim() : "";
      if (!text) return Promise.reject(new Error("sendPrompt needs { text }."));
      return call("sendPrompt", { text: text.slice(0, 4000) });
    },
    openLink: function (args) {
      return call("openLink", { url: String((args && args.url) || "") });
    }
  });
  window.addEventListener("message", function (e) {
    if (e.source !== parent) return;
    var d = e.data;
    if (!d || d.__eduHost !== true) return;
    if (d.type === "bridge-result") {
      var p = pending[d.id];
      if (!p) return;
      delete pending[d.id];
      if (d.ok) p.resolve({ ok: true }); else p.reject(new Error(d.error || "The request failed."));
    } else if (d.type === "exec") {
      var s = document.createElement("script");
      s.textContent = d.code;
      (document.body || document.documentElement).appendChild(s);
      s.remove();
      measure();
    } else if (d.type === "theme") {
      document.documentElement.setAttribute("data-theme", d.theme === "dark" ? "dark" : "light");
      window.dispatchEvent(new Event("themechange"));
    } else if (d.type === "a11y" && d.attrs) {
      Object.keys(A11Y).forEach(function (name) {
        var v = d.attrs[name];
        if (A11Y[name].indexOf(v) !== -1) document.documentElement.setAttribute(name, v);
      });
      // Colors may have changed; canvases redraw on themechange.
      window.dispatchEvent(new Event("themechange"));
      measure();
    } else if (d.type === "preview" && window.Idiomorph) {
      document.getElementById("edu-css").textContent = d.css || "";
      document.getElementById("edu-extra").textContent = d.styles || "";
      try {
        Idiomorph.morph(document.body, d.body || "", {
          morphStyle: "innerHTML",
          callbacks: { beforeNodeAdded: function (n) { if (n.nodeType === 1) n.classList.add("morph-enter"); return true; } }
        });
      } catch (err) { document.body.innerHTML = d.body || ""; }
      measure();
    }
  });
  function start() {
    new ResizeObserver(measure).observe(document.body);
    window.addEventListener("resize", measure);
    var n = 0, t = setInterval(function () { measure(); if (++n > 20) clearInterval(t); }, 150);
    measure();
    post({ type: "ready" });
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start); else start();
})();`;

/** Attributes for the frame's <html> element. */
function htmlAttrs(theme: Resolved, a11y: FrameA11y): string {
  return `lang="en" data-theme="${theme}" data-contrast="${a11y.contrast}" data-reduce-motion="${a11y.reduceMotion}" data-spacing="${a11y.spacing}" data-font="${a11y.font}"`;
}

/** The a11y message body for a live frame. */
export function a11yMessage(a11y: FrameA11y) {
  return {
    type: "a11y",
    attrs: { "data-contrast": a11y.contrast, "data-reduce-motion": a11y.reduceMotion, "data-spacing": a11y.spacing, "data-font": a11y.font },
  };
}

const bootstrap = (nonce: string) => BOOTSTRAP_SOURCE.replace("__EDU_NONCE__", nonce.replace(/[^a-zA-Z0-9-]/g, ""));

export function newNonce(): string {
  return crypto.randomUUID();
}

const BASE_HEAD =
  `<meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${CSP}">` +
  `<meta name="viewport" content="width=device-width, initial-scale=1">` +
  `<style>${DESIGN_SYSTEM_CSS}</style>` +
  `<style>html,body{overflow:hidden!important;height:auto!important;min-height:0!important}</style>`;

/** The preview document: no generated scripts, just morphed markup. */
export function buildPreviewDocument(theme: Resolved, nonce: string, a11y: FrameA11y): string {
  return (
    `<!doctype html><html ${htmlAttrs(theme, a11y)}><head>${BASE_HEAD}` +
    `<style id="edu-css"></style><style id="edu-extra"></style>` +
    `<script>${idiomorphSource}</script><script>${bootstrap(nonce)}</script>` +
    `</head><body></body></html>`
  );
}

/** Text of complete <style> blocks found in partial HTML. */
export function styleText(styleTags: string): string {
  return styleTags.replace(/<\/?style[^>]*>/gi, "");
}

/** The final document. Scripts arrive afterwards through "exec" messages. */
export function buildFinalDocument(theme: Resolved, css: string, html: string, nonce: string, a11y: FrameA11y): string {
  return (
    `<!doctype html><html ${htmlAttrs(theme, a11y)}><head>${BASE_HEAD}` +
    `<script type="importmap">${JSON.stringify(IMPORT_MAP)}</script>` +
    `<link rel="stylesheet" href="${KATEX_CSS}">` +
    `<style>${escapeStyle(css)}</style>` +
    `<script>${bootstrap(nonce)}</script>` +
    `</head><body>${html}</body></html>`
  );
}

/**
 * A self-contained HTML file for export. Scripts are inlined in order and the
 * bridge explains that follow-up questions need the app.
 */
export function buildStandaloneDocument(
  theme: Resolved,
  title: string,
  input: { css?: string; html?: string; jsFunctions?: string; jsExpressions?: string[] },
): string {
  const stub = String.raw`window.edu = {
  sendPrompt: function () { alert("Open this activity in Explorable to send follow-up questions."); return Promise.resolve({ ok: false }); },
  openLink: function (a) { window.open(a && a.url, "_blank", "noopener"); return Promise.resolve({ ok: true }); }
};
window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", function (e) {
  document.documentElement.setAttribute("data-theme", e.matches ? "dark" : "light");
  window.dispatchEvent(new Event("themechange"));
});`;
  const scripts = [input.jsFunctions ?? "", ...(input.jsExpressions ?? [])]
    .filter((s) => s.trim())
    .map((s) => `<script>${escapeScript(s)}</script>`)
    .join("\n");
  const safeTitle = title.replace(/[<>&"]/g, (c) => ({ "<": "&lt;", ">": "&gt;", "&": "&amp;", '"': "&quot;" })[c] ?? c);
  return `<!doctype html>
<html lang="en" data-theme="${theme}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${safeTitle}</title>
<script type="importmap">${JSON.stringify(IMPORT_MAP)}</script>
<link rel="stylesheet" href="${KATEX_CSS}">
<style>${DESIGN_SYSTEM_CSS}</style>
<style>body{max-width:960px;margin:24px auto;padding:0 16px;background:var(--color-background-primary)}</style>
<style>${escapeStyle(input.css ?? "")}</style>
<script>${stub}</script>
</head>
<body>
${input.html ?? ""}
${scripts}
</body>
</html>`;
}

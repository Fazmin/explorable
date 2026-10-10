import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AlertTriangle, RotateCcw, Wand2 } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import { currentTheme, type Resolved } from "../lib/theme";
import { a11yMessage, buildFinalDocument, buildPreviewDocument, newNonce, styleText } from "../sandbox/frame";
import { frameA11y, type FrameA11y } from "../lib/a11y";
import { extractCompleteStyles, processPartialHtml } from "../sandbox/partialHtml";
import { Figure, type FigureContext } from "../components/Figure";
import { useStore } from "../lib/store";

export interface InteractiveInput {
  title?: string;
  initialHeight?: number;
  placeholderMessages?: string[];
  css?: string;
  html?: string;
  jsFunctions?: string;
  jsExpressions?: string[];
}

interface Props {
  input: InteractiveInput;
  /** The whole tool input has arrived. */
  complete: boolean;
  completeKeys: string[];
  ctx: FigureContext;
}

const MIN_HEIGHT = 60;
const MAX_HEIGHT = 4000;
const PREVIEW_THROTTLE_MS = 140;
const PROMPT_COOLDOWN_MS = 2500;
const LINK_COOLDOWN_MS = 1500;

/** True while the user is interacting (a click or key press inside a child frame activates this window too). */
function hasUserActivation(): boolean {
  const ua = (navigator as Navigator & { userActivation?: { isActive: boolean } }).userActivation;
  return ua ? ua.isActive : true;
}

const clampHeight = (h: unknown): number | null =>
  typeof h === "number" && Number.isFinite(h) ? Math.max(MIN_HEIGHT, Math.min(Math.ceil(h), MAX_HEIGHT)) : null;

function isHostMessage(d: unknown): d is { __edu: true; type: string; [k: string]: unknown } {
  return !!d && typeof d === "object" && (d as { __edu?: unknown }).__edu === true;
}

export function InteractiveFrame({ input, complete, completeKeys, ctx }: Props) {
  const title = input.title?.trim() || "Interactive";
  const has = (k: string) => complete || completeKeys.includes(k);
  const htmlDone = has("html");
  const cssDone = has("css") || htmlDone;
  const fnDone = has("jsFunctions");
  const expressions = input.jsExpressions ?? [];
  const streaming = !complete;

  const initialHeight = Math.max(160, Math.min(clampHeight(input.initialHeight) ?? 360, 1400));
  const [height, setHeight] = useState<number | null>(null);
  const [finalReady, setFinalReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [phraseIndex, setPhraseIndex] = useState(0);

  const previewRef = useRef<HTMLIFrameElement>(null);
  const finalRef = useRef<HTMLIFrameElement>(null);
  const previewReady = useRef(false);
  const exec = useRef({ fn: false, count: 0 });
  const latest = useRef({ input, fnDone, expressions });
  latest.current = { input, fnDone, expressions };
  const lastPrompt = useRef(0);
  const lastLink = useRef(0);
  const [nonce] = useState(newNonce);
  const finalReadyRef = useRef(false);
  const ctxRef = useRef(ctx);
  ctxRef.current = ctx;
  // A preview frame is only worth creating if we watched this stream arrive.
  const sawStreaming = useRef(!complete);
  if (!complete) sawStreaming.current = true;
  const throttle = useRef<{ timer: number | null; last: number }>({ timer: null, last: 0 });

  // The final document is built once, when the HTML is complete; script
  // channels arrive afterwards as exec messages.
  const [themeAtBuild] = useState<Resolved>(() => currentTheme());
  const [a11yAtBuild] = useState<FrameA11y>(frameA11y);
  const finalDoc = useMemo(
    () => (htmlDone && input.html ? buildFinalDocument(themeAtBuild, input.css ?? "", input.html, nonce, a11yAtBuild) : null),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [htmlDone, reloadKey],
  );
  const previewDoc = useMemo(() => buildPreviewDocument(themeAtBuild, nonce, a11yAtBuild), [themeAtBuild, nonce, a11yAtBuild]);
  const showPreview = !finalReady && cssDone && sawStreaming.current;

  const post = (frame: HTMLIFrameElement | null, msg: Record<string, unknown>) => {
    frame?.contentWindow?.postMessage({ ...msg, __eduHost: true }, "*");
  };

  const flushExec = useCallback(() => {
    const frame = finalRef.current;
    if (!frame) return;
    const { input: inp, fnDone: fd, expressions: exprs } = latest.current;
    if (!exec.current.fn) {
      if (!fd) return;
      exec.current.fn = true;
      if (inp.jsFunctions?.trim()) post(frame, { type: "exec", code: inp.jsFunctions });
    }
    while (exec.current.count < exprs.length) {
      const code = exprs[exec.current.count++];
      if (typeof code === "string" && code.trim()) post(frame, { type: "exec", code });
    }
  }, []);

  const sendPreview = useCallback(() => {
    const frame = previewRef.current;
    if (!frame || !previewReady.current) return;
    const { input: inp } = latest.current;
    const raw = inp.html ?? "";
    post(frame, {
      type: "preview",
      css: inp.css ?? "",
      styles: styleText(extractCompleteStyles(raw)),
      body: processPartialHtml(raw),
    });
    throttle.current.last = Date.now();
  }, []);

  // Stream partial HTML into the preview at a bounded rate.
  useEffect(() => {
    if (!showPreview) return;
    const t = throttle.current;
    if (t.timer !== null) return;
    const wait = Math.max(0, PREVIEW_THROTTLE_MS - (Date.now() - t.last));
    t.timer = window.setTimeout(() => {
      t.timer = null;
      sendPreview();
    }, wait);
  }, [showPreview, input.html, input.css, sendPreview]);

  useEffect(() => () => {
    if (throttle.current.timer !== null) clearTimeout(throttle.current.timer);
  }, []);

  // Run newly arrived script channels once the final frame is ready.
  useEffect(() => {
    if (finalReady) flushExec();
  }, [finalReady, fnDone, expressions.length, flushExec]);

  // Rotate placeholder messages until something is visible.
  const placeholders = input.placeholderMessages?.filter((m) => typeof m === "string" && m.trim()) ?? [];
  useEffect(() => {
    if (cssDone || placeholders.length < 2) return;
    const id = window.setInterval(() => setPhraseIndex((i) => i + 1), 1700);
    return () => clearInterval(id);
  }, [cssDone, placeholders.length]);

  // Messages from both frames: size, readiness, errors, and bridge calls.
  useEffect(() => {
    const onMessage = async (e: MessageEvent) => {
      const ctx = ctxRef.current;
      const fromFinal = e.source === finalRef.current?.contentWindow && !!finalRef.current;
      const fromPreview = e.source === previewRef.current?.contentWindow && !!previewRef.current;
      if ((!fromFinal && !fromPreview) || !isHostMessage(e.data)) return;
      const d = e.data;
      // Only the documents this component built carry its nonce.
      if (d.nonce !== nonce) return;
      switch (d.type) {
        case "resize": {
          const h = clampHeight(d.height);
          if (h !== null && (fromFinal || !finalReadyRef.current)) setHeight(h);
          break;
        }
        case "ready":
          if (fromPreview) {
            previewReady.current = true;
            post(previewRef.current, { type: "theme", theme: currentTheme() });
            post(previewRef.current, a11yMessage(frameA11y()));
            sendPreview();
          } else {
            finalReadyRef.current = true;
            setFinalReady(true);
            post(finalRef.current, { type: "theme", theme: currentTheme() });
            post(finalRef.current, a11yMessage(frameA11y()));
          }
          break;
        case "error":
          if (fromFinal) setError((prev) => prev ?? String(d.message ?? "Script error").slice(0, 400));
          break;
        case "escape":
          // Replay it in the app, so Present view, menus, and dialogs close as
          // if focus were outside the frame. The nonce shows it came from a real
          // key press: generated code can't read it. Escape doesn't count as
          // user activation, so there's no gesture check here.
          document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
          break;
        case "bridge": {
          const reply = (ok: boolean, err?: string) => post(finalRef.current, { type: "bridge-result", id: d.id, ok, error: err });
          if (!fromFinal) return;
          const args = (d.args ?? {}) as { text?: unknown; url?: unknown };
          if (d.method === "sendPrompt") {
            const text = typeof args.text === "string" ? args.text.trim() : "";
            if (!text || text.length > 4000) return reply(false, "The message must be 1–4000 characters.");
            if (!ctx.onPrompt) return reply(false, "Follow-up questions aren't available here. Ask in the chat instead.");
            // Checked here too: the in-frame check runs alongside generated code.
            if (!hasUserActivation()) return reply(false, "Follow-up questions can only be sent from a click or key press.");
            if (Date.now() - lastPrompt.current < PROMPT_COOLDOWN_MS) return reply(false, "Please wait a moment before sending again.");
            lastPrompt.current = Date.now();
            try {
              await ctx.onPrompt(text);
              reply(true);
            } catch (err) {
              reply(false, errorMessage(err));
            }
          } else if (d.method === "openLink") {
            const url = typeof args.url === "string" ? args.url : "";
            let ok = false;
            try {
              ok = new URL(url).protocol === "https:";
            } catch {
              ok = false;
            }
            if (!ok) return reply(false, "Only https links can be opened.");
            if (!hasUserActivation()) return reply(false, "Links can only be opened from a click or key press.");
            if (Date.now() - lastLink.current < LINK_COOLDOWN_MS) return reply(false, "Please wait a moment before opening another link.");
            lastLink.current = Date.now();
            try {
              await api.openExternal(url);
              reply(true);
            } catch (err) {
              reply(false, errorMessage(err));
            }
          } else {
            reply(false, "Unknown request.");
          }
          break;
        }
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [sendPreview, nonce]);

  // Follow the app theme.
  useEffect(() => {
    const onTheme = (e: Event) => {
      const theme = (e as CustomEvent<Resolved>).detail;
      post(previewRef.current, { type: "theme", theme });
      post(finalRef.current, { type: "theme", theme });
    };
    const onA11y = (e: Event) => {
      const msg = a11yMessage((e as CustomEvent<FrameA11y>).detail);
      post(previewRef.current, msg);
      post(finalRef.current, msg);
    };
    window.addEventListener("edu-theme", onTheme);
    window.addEventListener("edu-a11y", onA11y);
    return () => {
      window.removeEventListener("edu-theme", onTheme);
      window.removeEventListener("edu-a11y", onA11y);
    };
  }, []);

  const reload = () => {
    exec.current = { fn: false, count: 0 };
    finalReadyRef.current = false;
    setFinalReady(false);
    setError(null);
    setReloadKey((k) => k + 1);
  };

  const askToFix = async () => {
    if (!ctx.onPrompt || !error) return;
    try {
      await ctx.onPrompt(`The interactive "${title}" showed this error: ${error}\nPlease fix the problem and render a corrected version.`);
    } catch (err) {
      // Keep the original error visible; report the send failure separately.
      useStore.getState().toast(errorMessage(err), "error");
    }
  };

  // Present view enlarges the frame from outside, so the frame's own layout
  // and height measurement stay in its native coordinates.
  const zoom = ctx.surface === "present" ? 1.2 : 1;
  const stageHeight = (height ?? initialHeight) * zoom;
  const frameStyle = zoom === 1 ? undefined : { width: `${100 / zoom}%`, height: `${100 / zoom}%`, transform: `scale(${zoom})`, transformOrigin: "0 0" };
  const nothingVisible = !cssDone && !finalDoc;
  const phrase = placeholders.length ? placeholders[phraseIndex % placeholders.length] : "Building the interactive";

  const status = streaming ? (
    <span className="figure-status" role="status">
      <span className="spinner" />
      {!cssDone ? "Styling" : !htmlDone ? "Laying out" : !fnDone ? "Adding behavior" : "Finishing"}
    </span>
  ) : null;

  const footer = error ? (
    <div className="frame-error" role="alert">
      <AlertTriangle aria-hidden />
      <span className="grow">
        This interactive hit an error: <code>{error}</code>
      </span>
      <button type="button" className="btn btn-sm" onClick={reload}>
        <RotateCcw aria-hidden /> Reload
      </button>
      {ctx.onPrompt && (
        <button type="button" className="btn btn-sm" onClick={askToFix}>
          <Wand2 aria-hidden /> Ask to fix
        </button>
      )}
    </div>
  ) : null;

  if (complete && !input.html?.trim()) {
    return (
      <p className="notice warning">
        <AlertTriangle aria-hidden /> This interactive arrived without any content.
      </p>
    );
  }

  return (
    <Figure
      kind="interactive"
      title={title}
      payload={input as Record<string, unknown>}
      ctx={ctx}
      streaming={streaming}
      status={status}
      footer={footer}
      wide
      bodyClass="figure-body-flush"
    >
      <div className="frame-stage" style={{ height: stageHeight }}>
        {nothingVisible && (
          <div className="frame-placeholder" aria-hidden="true">
            <span className="placeholder-line">
              <span className="spinner" /> {phrase}
            </span>
          </div>
        )}
        {showPreview && (
          <iframe
            ref={previewRef}
            title={`${title} (preview)`}
            sandbox="allow-scripts"
            srcDoc={previewDoc}
            style={frameStyle}
            aria-hidden="true"
            tabIndex={-1}
          />
        )}
        {finalDoc && (
          <iframe
            key={reloadKey}
            ref={finalRef}
            title={title}
            sandbox="allow-scripts"
            srcDoc={finalDoc}
            style={frameStyle}
            className={finalReady ? undefined : "hidden"}
          />
        )}
      </div>
    </Figure>
  );
}

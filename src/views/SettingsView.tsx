import { useEffect, useState, type ReactNode } from "react";
import { CheckCircle2, ExternalLink, KeyRound, Monitor, Moon, Sun } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import { useStore } from "../lib/store";
import type { CatalogModel, KeyProvider, ProviderId, ProviderInfo, Settings as SettingsData, SettingsView as SettingsViewData, Style, Theme } from "../lib/types";
import { scaleLabel, TEXT_SCALES } from "../lib/a11y";

const EFFORTS = [
  { id: "low", label: "Low", note: "Quickest answers" },
  { id: "medium", label: "Medium", note: "Balanced (default)" },
  { id: "high", label: "High", note: "More careful reasoning; slower" },
];

/** Settings fields that hold each provider's models and endpoint. */
const FIELDS: Record<KeyProvider, { model: keyof SettingsViewData; router: keyof SettingsViewData; baseUrl?: keyof SettingsViewData }> = {
  anthropic: { model: "model", router: "routerModel" },
  openai: { model: "openaiModel", router: "openaiRouterModel", baseUrl: "openaiBaseUrl" },
  gemini: { model: "geminiModel", router: "geminiRouterModel", baseUrl: "geminiBaseUrl" },
};

export function SettingsView() {
  const settings = useStore((s) => s.settings);
  const { setTheme, saveSettings } = useStore.getState();
  const [catalog, setCatalog] = useState<ProviderInfo[]>([]);

  useEffect(() => {
    api.modelCatalog().then(setCatalog).catch(() => setCatalog([]));
  }, []);

  if (!settings) return null;
  const active = catalog.find((p) => p.id === settings.provider) ?? null;

  const themes: { value: Theme; label: string; icon: typeof Sun }[] = [
    { value: "light", label: "Light", icon: Sun },
    { value: "dark", label: "Dark", icon: Moon },
    { value: "system", label: "Match system", icon: Monitor },
  ];
  const choices: { id: ProviderId; title: string; note: string }[] = [
    ...catalog.map((p) => ({ id: p.id as ProviderId, title: p.label, note: p.tagline })),
    { id: "demo", title: "Demo mode", note: "Scripted sample answers, no network" },
  ];

  return (
    <>
      <header className="topbar">
        <h1>Settings</h1>
      </header>
      <div className="view-scroll">
        <div className="settings">
          <h2>Settings</h2>

          <section className="settings-section">
            <header>
              <h3>Appearance</h3>
              <p>Interactives follow the theme too.</p>
            </header>
            <div className="settings-fields">
              <div className="segmented" role="group" aria-label="Theme">
                {themes.map(({ value, label, icon: Icon }) => (
                  <button key={value} type="button" aria-pressed={settings.theme === value} onClick={() => setTheme(value)}>
                    <Icon aria-hidden /> {label}
                  </button>
                ))}
              </div>
            </div>
          </section>

          <AccessibilitySection settings={settings} />

          <section className="settings-section">
            <header>
              <h3>AI provider</h3>
              <p>Use Claude, OpenAI, or Gemini with your own API key. Demo mode replays scripted answers so you can try every feature without one.</p>
            </header>
            <div className="settings-fields">
              <div className="provider-choice" role="group" aria-label="Provider">
                {choices.map((c) => {
                  const hasKey = c.id !== "demo" && !!settings.apiKeys[c.id as KeyProvider];
                  return (
                    <button key={c.id} type="button" className="provider-option" aria-pressed={settings.provider === c.id} onClick={() => saveSettings({ provider: c.id })}>
                      <strong>{c.title}</strong>
                      <span>{c.note}</span>
                      {hasKey && (
                        <span className="key-status ok">
                          <CheckCircle2 aria-hidden /> Key saved
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>
              {active && <ProviderSettings key={active.id} info={active} settings={settings} />}
            </div>
          </section>

          <section className="settings-section">
            <header>
              <h3>Tutoring</h3>
              <p>The default for new study chats. You can switch in any chat.</p>
            </header>
            <div className="settings-fields">
              <div className="segmented" role="group" aria-label="Default tutoring style">
                {(
                  [
                    ["direct", "Explain"],
                    ["socratic", "Guide me"],
                  ] as [Style, string][]
                ).map(([value, label]) => (
                  <button key={value} type="button" aria-pressed={settings.defaultStyle === value} onClick={() => saveSettings({ defaultStyle: value })}>
                    {label}
                  </button>
                ))}
              </div>
              <span className="field-hint">
                Explain gives complete answers. Guide me asks questions and offers hints first, which helps with homework you need to learn to do yourself.
              </span>
            </div>
          </section>

          <section className="settings-section">
            <header>
              <h3>Your data</h3>
              <p>Everything stays on this computer.</p>
            </header>
            <div className="settings-fields">
              <div className="field">
                <span className="field-label">Database folder</span>
                <span className="mono">{settings.dataDir}</span>
              </div>
              <span className="field-hint">
                Chats, courses, materials, decks, and progress are stored in a single SQLite file in this folder. Messages go to the AI provider you choose only when you use live tutoring.
              </span>
              <span className="field-hint">Explorable {settings.version}</span>
            </div>
          </section>
        </div>
      </div>
    </>
  );
}

function ModelSelect({ label, value, models, onChange, disabled, hint }: { label: string; value: string; models: CatalogModel[]; onChange: (id: string) => void; disabled?: boolean; hint?: string }) {
  const known = models.some((m) => m.id === value);
  return (
    <label className="field">
      <span>{label}</span>
      <select className="select" value={value} onChange={(e) => onChange(e.target.value)} disabled={disabled}>
        {models.map((m) => (
          <option key={m.id} value={m.id}>
            {m.label}: {m.note}
          </option>
        ))}
        {!known && <option value={value}>{value} (custom)</option>}
      </select>
      {hint && <span className="field-hint">{hint}</span>}
    </label>
  );
}

function ProviderSettings({ info, settings }: { info: ProviderInfo; settings: SettingsViewData }) {
  const { saveSettings, setSettings, toast } = useStore.getState();
  const provider = info.id;
  const fields = FIELDS[provider];
  const source = settings.apiKeys[provider];
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const [custom, setCustom] = useState({ model: "", router: "", baseUrl: (fields.baseUrl ? String(settings[fields.baseUrl] ?? "") : "") });

  const saveKey = async () => {
    setBusy(true);
    setMessage(null);
    try {
      setSettings(await api.setApiKey(provider, key));
      setKey("");
      try {
        await api.testApiKey(provider);
        setMessage({ ok: true, text: "Key saved to the system keychain and verified." });
      } catch (err) {
        setMessage({ ok: false, text: `Key saved, but the check failed: ${errorMessage(err)}` });
      }
    } catch (err) {
      setMessage({ ok: false, text: errorMessage(err) });
    } finally {
      setBusy(false);
    }
  };

  const testKey = async () => {
    setBusy(true);
    setMessage(null);
    try {
      await api.testApiKey(provider);
      setMessage({ ok: true, text: "Connection works." });
    } catch (err) {
      setMessage({ ok: false, text: errorMessage(err) });
    } finally {
      setBusy(false);
    }
  };

  const removeKey = async () => {
    try {
      setSettings(await api.clearApiKey(provider));
      setMessage(null);
      toast(`${info.company} API key removed from the keychain`, "success");
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  };

  const saveCustom = async () => {
    const patch: Record<string, string> = {};
    if (custom.model.trim()) patch[fields.model] = custom.model.trim();
    if (custom.router.trim()) patch[fields.router] = custom.router.trim();
    if (fields.baseUrl) patch[fields.baseUrl] = custom.baseUrl.trim();
    try {
      await saveSettings(patch as never);
      setCustom((c) => ({ ...c, model: "", router: "" }));
      toast("Advanced settings saved", "success");
    } catch {
      // saveSettings already reported the error.
    }
  };

  return (
    <>
      <div className="field">
        <span className="field-label">{info.company} API key</span>
        {source && (
          <div className="key-row" style={{ alignItems: "center" }}>
            <span className="key-status ok">
              <CheckCircle2 aria-hidden />
              {source === "keychain" ? "Saved in the system keychain" : `Read from the ${info.envVar} environment variable`}
            </span>
            <span style={{ flex: 1 }} />
            <button type="button" className="btn btn-sm" onClick={testKey} disabled={busy}>
              Test connection
            </button>
            {source === "keychain" && (
              <button type="button" className="btn btn-sm btn-ghost btn-danger" onClick={removeKey}>
                Remove
              </button>
            )}
          </div>
        )}
        <div className="key-row">
          <input
            className="input"
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={key}
            onChange={(e) => setKey(e.target.value)}
            placeholder={source ? "Paste a new key to replace it" : info.keyPlaceholder}
            aria-label={`${info.company} API key`}
          />
          <button type="button" className="btn btn-primary" onClick={saveKey} disabled={!key.trim() || busy}>
            <KeyRound aria-hidden /> Save key
          </button>
        </div>
        <span className="field-hint">
          Stored in your operating system's keychain, never in the app database. Usage is billed to your {info.company} account.{" "}
          <button type="button" className="link-btn" onClick={() => void api.openExternal(info.keyUrl)}>
            Get a key <ExternalLink aria-hidden />
          </button>
        </span>
        {message && (
          <span role="status" style={{ fontSize: "var(--text-sm)", color: message.ok ? "var(--success)" : "var(--danger)" }}>
            {message.text}
          </span>
        )}
      </div>

      <ModelSelect label="Answer model" value={String(settings[fields.model])} models={info.models} onChange={(id) => saveSettings({ [fields.model]: id } as never)} />

      <label className="field">
        <span>Effort</span>
        <select className="select" value={settings.effort} onChange={(e) => saveSettings({ effort: e.target.value })}>
          {EFFORTS.map((e) => (
            <option key={e.id} value={e.id}>
              {e.label}: {e.note}
            </option>
          ))}
        </select>
        <span className="field-hint">How much the model reasons before answering. Each provider maps this to its own reasoning setting; models without one ignore it.</span>
      </label>

      <label className="check-row">
        <input type="checkbox" checked={settings.routerEnabled} onChange={(e) => saveSettings({ routerEnabled: e.target.checked })} />
        <span>
          Choose the answer format with a fast router model
          <small>A small model picks text, a quiz, an interactive, and so on for each question, and names new chats. Turn off to let the answer model decide.</small>
        </span>
      </label>

      <ModelSelect
        label="Router model"
        value={String(settings[fields.router])}
        models={info.routerModels}
        onChange={(id) => saveSettings({ [fields.router]: id } as never)}
        disabled={!settings.routerEnabled}
      />

      {provider === "anthropic" && (
        <label className="check-row">
          <input type="checkbox" checked={settings.refusalFallback} onChange={(e) => saveSettings({ refusalFallback: e.target.checked })} />
          <span>
            Retry declined requests on a fallback model
            <small>If a safety classifier declines a request, Anthropic re-runs it on its recommended fallback model instead of stopping.</small>
          </span>
        </label>
      )}

      <details className="advanced">
        <summary>Advanced</summary>
        <div className="settings-fields" style={{ marginTop: 12 }}>
          <label className="field">
            <span>Custom answer model ID</span>
            <input className="input" value={custom.model} onChange={(e) => setCustom({ ...custom, model: e.target.value })} placeholder={String(settings[fields.model])} spellCheck={false} />
          </label>
          <label className="field">
            <span>Custom router model ID</span>
            <input className="input" value={custom.router} onChange={(e) => setCustom({ ...custom, router: e.target.value })} placeholder={String(settings[fields.router])} spellCheck={false} />
          </label>
          {fields.baseUrl && (
            <label className="field">
              <span>API base URL</span>
              <input className="input" value={custom.baseUrl} onChange={(e) => setCustom({ ...custom, baseUrl: e.target.value })} placeholder="Leave empty for the official endpoint" spellCheck={false} />
              <span className="field-hint">For a compatible gateway or proxy. Your API key is sent to this address.</span>
            </label>
          )}
          <div>
            <button type="button" className="btn" onClick={saveCustom}>
              Save advanced settings
            </button>
          </div>
        </div>
      </details>
    </>
  );
}

function Check({ checked, onChange, title, children }: { checked: boolean; onChange: (v: boolean) => void; title: string; children: ReactNode }) {
  return (
    <label className="check-row">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span>
        {title}
        <small>{children}</small>
      </span>
    </label>
  );
}

function AccessibilitySection({ settings }: { settings: SettingsViewData }) {
  const updatePrefs = useStore((s) => s.updatePrefs);
  const set = (patch: Partial<SettingsData>) => updatePrefs(patch);
  const fonts: { value: SettingsData["readingFont"]; label: string }[] = [
    { value: "hyperlegible", label: "Hyperlegible" },
    { value: "serif", label: "Serif" },
    { value: "system", label: "System" },
  ];
  return (
    <section className="settings-section" id="accessibility-settings">
      <header>
        <h3>Accessibility</h3>
        <p>These apply across the app and inside interactive answers. The accessibility button next to the theme button has the most common ones.</p>
      </header>
      <div className="settings-fields">
        <div className="field">
          <span className="field-label" id="text-size">Text size</span>
          <div className="segmented" role="group" aria-labelledby="text-size">
            {TEXT_SCALES.map((scale) => (
              <button key={scale} type="button" aria-pressed={Math.abs(settings.textScale - scale) < 0.01} onClick={() => set({ textScale: scale })}>
                {scaleLabel(scale)}
              </button>
            ))}
          </div>
          <span className="field-hint">Enlarges everything, including interactive answers.</span>
        </div>
        <Check checked={settings.highContrast} onChange={(v) => set({ highContrast: v })} title="Increase contrast">
          Darker text, firmer borders, and a thicker focus outline, in both light and dark themes.
        </Check>
        <Check checked={settings.reduceMotion} onChange={(v) => set({ reduceMotion: v })} title="Reduce motion">
          Turns off animations in the app and asks interactives to stay still, even if your system setting is off.
        </Check>
        <Check checked={settings.relaxedSpacing} onChange={(v) => set({ relaxedSpacing: v })} title="Relaxed spacing">
          Wider line, letter, and word spacing, which many readers with dyslexia find easier.
        </Check>
        <div className="field">
          <span className="field-label" id="reading-font">Reading font</span>
          <div className="segmented" role="group" aria-labelledby="reading-font">
            {fonts.map((f) => (
              <button key={f.value} type="button" aria-pressed={settings.readingFont === f.value} onClick={() => set({ readingFont: f.value })}>
                {f.label}
              </button>
            ))}
          </div>
          <span className="field-hint">Hyperlegible (Atkinson Hyperlegible Next) is designed for low-vision readers. Serif suits long reading; System uses your computer's font.</span>
        </div>
        <Check checked={settings.readAloud} onChange={(v) => set({ readAloud: v })} title="Show Read aloud buttons">
          Adds a button under each answer that reads it with your system's voices, including spoken math.
        </Check>
      </div>
    </section>
  );
}

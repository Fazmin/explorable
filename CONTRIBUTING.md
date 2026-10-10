# Contributing to Explorable

Thanks for helping. This guide covers setup, the rules the code depends on, step-by-step instructions for the most common kinds of change, and what a pull request needs before review.

Read [How it works](README.md#how-it-works) in the README first. The rest of this guide assumes you know the turn flow (route, stream, validate, persist) and the sandbox model.

For anything bigger than a small fix, such as a new provider, a new answer type, or a schema change, open an issue to discuss it before you write the code.

## Development setup

You need:

- [Rust](https://rustup.rs) 1.80 or later, with clippy (`rustup component add clippy`)
- Node.js 22 or later
- The [Tauri system prerequisites](https://tauri.app/start/prerequisites/) for your OS
- Google Chrome, but only for the browser test harness (`npm run test:e2e`)

```bash
npm install
npm run tauri dev
```

The app starts in demo mode, which replays scripted answers, so most UI and agent work needs no API key.

To test against a real provider, set its key in the environment when you launch:

```bash
ANTHROPIC_API_KEY=sk-ant-... npm run tauri dev
```

A key saved in the OS keychain takes precedence over the environment variable. To make sure the app uses the environment variable, remove any saved key in Settings → AI provider.

All app data is in one SQLite file, `explorable.sqlite3`, in the app data folder for `com.explorable.desktop`. On macOS that is `~/Library/Application Support/com.explorable.desktop/`. Settings shows the exact path. To start fresh, quit the app and delete the file.

## Where things live

The [project layout](README.md#project-layout) in the README gives the full tree. This table shows where to start for common changes:

| To change | Start in |
| --- | --- |
| What the assistant is told | `src-tauri/src/agent/prompt.rs` |
| How a question is routed | `src-tauri/src/agent/router.rs` |
| Tool definitions and input validation | `src-tauri/src/agent/tools.rs` |
| The turn loop, retries, cancellation | `src-tauri/src/agent/mod.rs`, `src-tauri/src/llm/stream.rs` |
| What is sent back to each provider | `src-tauri/src/agent/history.rs` and the adapter in `src-tauri/src/llm/` |
| Models offered in Settings | `src-tauri/src/llm/catalog.rs` |
| Scripted demo answers | `src-tauri/src/llm/demo_content.rs` |
| How an answer type looks | `src/renderers/` |
| Generated interactives and the sandbox | `src/sandbox/frame.ts`, `src/renderers/InteractiveFrame.tsx` |
| Colors, type, spacing | `src/styles/tokens.css` |

## Rules the code depends on

These rules prevent whole classes of bugs. A pull request that breaks one needs a strong reason, stated in its description.

1. **Logic lives in Rust. The webview only renders.** Anything that touches data, providers, files, or the network is a Tauri command in `src-tauri/src/commands/`. The frontend calls it through the typed wrappers in `src/lib/api.ts`, never through `invoke()` directly.
2. **Stored history is append-only and verbatim.** Content blocks are saved exactly as they were received, including thinking signatures and provider data. Don't edit, reorder, or rewrite stored blocks. All conversion for a particular provider happens when a request is built, in `history::to_api_messages`.
3. **Tool input is untrusted until validated.** Before anything uses a tool input, it is deserialized into a typed struct and checked: indexes in range, rows matching columns, and so on. Invalid input goes back to the model as an error `tool_result` so it can correct itself. The partial-JSON snapshots that stream to the UI are for display only and must never drive an action.
4. **API keys stay in the OS keychain or the environment.** Don't write keys to SQLite, logs, error messages, test fixtures, or anything sent to the webview. `SettingsView` reports only where a key came from.
5. **Provider-specific data goes in `_`-prefixed fields.** Data that must round-trip to one provider is stored in a private field on the block: encrypted reasoning, thought signatures, item IDs. `strip_private` removes these fields before a request goes to any other provider. A new field without the underscore would be sent to every provider and rejected by some.
6. **Model output is never HTML in the main webview.** Answers render through the Markdown component and the typed renderers. Generated HTML runs only inside the sandboxed frame (see below).

## Sandbox security

Generated interactives run model-written HTML and JavaScript, so the sandbox is the most security-sensitive part of the app. Keep all of these:

- Frames use `sandbox="allow-scripts"` and nothing else. Never add `allow-same-origin`: with `allow-scripts`, it lets the frame remove its own sandbox. `allow-top-navigation`, `allow-popups`, and `allow-forms` are also off limits.
- Documents load through `srcdoc` with the per-frame Content Security Policy built in `src/sandbox/frame.ts`. Adding a script origin or a library to the import map is a security change; say so in the pull request.
- The bridge (`edu.sendPrompt`, `edu.openLink`) depends on each of these:
  - a nonce for each document, which the bootstrap script removes from the DOM
  - a check of the message source in the host
  - a real user gesture, checked in the frame and again in the host
  - rate limits in the host (2.5 s for prompts, 1.5 s for links)
  - validation of the text
- `openLink` accepts only https, and the check is repeated in Rust (`open_external` in `src-tauri/src/commands/system.rs`).
- Frames pass Escape on to the app, so Present view and menus still close while focus is inside an interactive. The frame sends it only for real key presses (`isTrusted`) that the interactive didn't handle, with the document's nonce.

After any change to `src/sandbox/` or `InteractiveFrame.tsx`, run `npm run test:e2e`. It checks isolation (no storage, no parent access, no IPC), the gesture requirement, and the bridge.

Report security problems privately to the maintainers, not in a public issue. That includes any way for generated content to escape the frame.

## Coding conventions

### Rust

- `cargo clippy --all-targets -- -D warnings` must pass.
- Return `AppResult<T>`. Use `AppError::Invalid` for bad input, `AppError::NotFound` for missing records, and `AppError::Provider` for provider failures. Error text is shown to users, so write it as a plain sentence that says what happened and what to do next.
- Avoid `unwrap()` and `expect()` outside tests.
- Short SQLite queries run directly in commands. Slow blocking work, such as keychain access and PDF text extraction, goes in `tokio::task::spawn_blocking`, as in `secrets.rs` and `commands/materials.rs`.
- The code isn't run through `rustfmt`. Match the formatting of the surrounding code, and don't reformat code you aren't changing.

### TypeScript and React

- `npm run typecheck` must pass (strict mode).
- Types that mirror Rust structs are in `src/lib/types.ts`. Serde uses camelCase on the wire, so the TypeScript fields are camelCase.
- Shared state lives in the zustand store, `src/lib/store.ts`. Keep component state local unless another view needs it.
- Renderers receive tool input that grows while it streams. Every field can be missing until it arrives, and a partial string inside an array is left out until it is complete. Render whatever has arrived without crashing.

### CSS

- Use the tokens in `src/styles/tokens.css` and don't hard-code colors in component styles. A new color token needs four values: light, dark, and the increased-contrast version of each (`[data-theme="dark"]`, `[data-contrast="more"]`).
- Use the existing scales (`--text-*`, `--radius-*`) rather than one-off sizes.
- Animations and transitions must stop under both `prefers-reduced-motion` and `:root[data-reduce-motion="true"]`. `base.css` already handles most cases; check anything that animates through JavaScript.

### Interface text

- Use sentence case and plain verbs. Name things the way a student or instructor would.
- A button says what it does, and the confirmation uses the same word: "Save to library", then "Saved".
- Error messages say what went wrong and how to fix it, without apologizing.

## Accessibility

Accessibility is a feature of the app, not a polish step. Every change to the interface must:

- Work with the keyboard alone and show a visible focus outline.
- Give icon-only buttons an `aria-label`.
- Never use color as the only signal. For example, quiz answers show a letter badge and text along with the green or red highlight.
- Still work in both themes with these settings on: text size at 150%, increased contrast, relaxed spacing, and reduce motion.

A new accessibility setting also has to reach generated interactives: add it to `frameA11y` in `src/lib/a11y.ts` and to the frame message in `src/sandbox/frame.ts`.

## Common changes

### Add a model

1. Add it to the provider's `models` list (or `router_models`) in `src-tauri/src/llm/catalog.rs`. Settings shows the models in the order of that list.
2. If the model's capabilities differ from its provider's other models, update the capability check:
   - Claude: `anthropic::caps` (adaptive thinking, effort, server-side fallbacks)
   - OpenAI: `openai::streams` (models that don't stream)
   - Gemini: `gemini::thinking_level`
3. To make it a default, change `Settings::default` in `src-tauri/src/models.rs`. The `defaults_are_in_the_catalog` test fails if a default is missing from the catalog.
4. If the browser harness depends on it, update the mock catalog in `tests/e2e/mock-ipc.js`.
5. Update the model table in the README.

### Add an AI provider

1. Create `src-tauri/src/llm/<provider>.rs` with a client and a `StreamAdapter` implementation:
   - `request` builds the HTTP request.
   - `on_http_error` returns `ResendNow`, `Retry`, or `Fail`.
   - `translate` turns each server-sent event into canonical events, which have the shape of the Claude Messages stream.
   - `finish` closes any open blocks.
   - `translate_body` handles models that don't stream.
2. Store anything that must round-trip in `_`-prefixed fields, emitted as `meta_delta` events, and drop thinking blocks from other providers when you build the request.
3. Add a variant to `Provider` in `src-tauri/src/llm/mod.rs`, including `complete_json`, which the router uses for structured output.
4. Register the key in `src-tauri/src/secrets.rs`: `PROVIDERS`, the keychain account, the environment variables, and `validate_key`.
5. Add a `ProviderInfo` entry to `catalog.rs`, and add the provider's model fields to `Settings` (Rust) and `src/lib/types.ts`. The Settings view builds its provider list from the catalog.
6. Add mock-server tests in `src-tauri/src/agent/e2e_tests.rs`, following the OpenAI and Gemini tests. Cover a full tool loop, what the follow-up request sends back, router output, and an HTTP error with retry.

### Add an answer type

Backend:

1. Define the tool in `definitions()` in `src-tauri/src/agent/tools.rs`, add it to `RENDER_TOOLS`, and add a typed struct with checks to `validate_render`.
2. Add the presentation to `LEARN_OPTIONS` or `TEACH_EXTRA` in `router.rs`, describe when to choose it in `ROUTER_SYSTEM`, and map it in `renderer_for`.
3. Tell the model when and how to use it in `src-tauri/src/agent/prompt.rs`.
4. Add export formats to `src-tauri/src/export.rs`.
5. Write a demo script in `src-tauri/src/llm/demo_content.rs`, so demo mode and the recorded fixtures include the new type.

Frontend:

1. Add the input type to `src/lib/types.ts` and write `src/renderers/<Name>.tsx`.
2. Add a case to `src/renderers/ToolBlock.tsx`.
3. Add entries to `TOOL_KIND`, `KIND_LABEL`, `KIND_ICON`, `EXPORTS`, and `PRESENTATION_LABEL` in `src/lib/kinds.ts`.
4. Optionally, add a starter prompt in `src/chat/starters.ts`.

Then run `npm run fixtures`, add checks for the new renderer to `tests/e2e/screenshots.mjs`, and run `npm run test:e2e`.

### Add a Tauri command

1. Write the handler in `src-tauri/src/commands/<area>.rs` and register it in `generate_handler!` in `src-tauri/src/lib.rs`.
2. Add a typed wrapper to `src/lib/api.ts`.
3. Add a handler to `tests/e2e/mock-ipc.js` so the browser harness can still load every view.
4. If the command uses a plugin that needs a permission, add the permission to `src-tauri/capabilities/default.json`, and grant only what the command needs.

### Change the database schema

Append a new migration to `MIGRATIONS` in `src-tauri/src/db/schema.rs`. Never edit a migration that has shipped. Installed copies track their place with `PRAGMA user_version` and will not rerun it. Each migration runs in its own transaction. Put the queries in the matching file under `src-tauri/src/repo/`, with tests in `src-tauri/src/repo/tests.rs`.

## Testing

| Command | What it runs | When to run it |
| --- | --- | --- |
| `npm run test:rust` | Rust unit tests and end-to-end agent runs (demo provider and mock provider servers) | Every change |
| `npm test` | Frontend unit tests (Vitest) | Every change |
| `npm run typecheck` | TypeScript in strict mode | Every change |
| `cargo clippy --all-targets -- -D warnings` (in `src-tauri/`) | Rust lints | Every change |
| `npm run test:e2e` | Real frontend in headless Chrome against recorded agent output, with checks inside sandboxed frames and screenshots of every view in both themes | Changes to the UI, renderers, or sandbox |
| `npm run fixtures` | Re-records `tests/fixtures/` from the real agent loop | After changing starters, demo content, or the event stream |

The browser harness serves the app on port 1430, so that port must be free. It saves screenshots to `tests/e2e/screenshots/`, which git ignores. Set `SHOTS_DIR` to save them somewhere else. If you re-record fixtures, commit the new files in the same pull request as the change that required them.

### Testing against live providers

The adapters are tested against each provider's documented formats through a local mock server. They have not yet been run against the live APIs, so live checks from contributors who have keys are especially useful. A useful smoke test covers:

- a short text answer
- an interactive
- a quiz
- a turn that asks you to choose between options
- a follow-up question in the same chat
- cancelling a turn while it streams
- switching provider partway through a chat

These behaviors are the most likely to differ from the documentation:

- Claude: the `fallbacks: "default"` setting
- OpenAI: reasoning summaries (some organizations need verification first)
- Gemini: `responseJsonSchema` in the router
- GPT-5.5 Pro: the non-streaming path

Say in your pull request which providers and models you tested live.

## Pull requests

- Keep each pull request to one change. Refactors belong in a pull request of their own.
- Describe the change from the user's side first, then the implementation.
- For interface changes, include before-and-after screenshots in light and dark themes.
- If a change alters what the README screenshots or tour show, regenerate them with `npm run media`. It replays the demo answers in Chrome, writes `docs/images/` and `docs/media/`, and needs ffmpeg with libx264 (on your `PATH`, or set `FFMPEG` to its path).
- Write commit subjects in the imperative mood, at 72 characters or fewer ("Add Gemini router fallback"). Use the body to explain why.
- The project uses npm. Commit `package-lock.json` together with any dependency change.
- Never commit API keys, `.env` files, your local database, `dist/`, or `src-tauri/target/`.

Before you request review, check that:

- [ ] `npm run test:rust`, `npm test`, `npm run typecheck`, and clippy pass
- [ ] `npm run test:e2e` passes, if you changed the UI, renderers, or sandbox
- [ ] Fixtures are re-recorded, if demo content or the event stream changed
- [ ] New UI works by keyboard and in both themes, with increased contrast and 150% text
- [ ] The README is updated, if setup, models, or user-visible features changed, and its media is regenerated if the interface changed

## License

Explorable is licensed under the [Apache License 2.0](LICENSE). Under section 5 of that license, anything you intentionally submit for inclusion is licensed under the same terms, unless you state otherwise in writing. Code you didn't write must have a compatible license, and its original copyright and license notices must be kept. Say in the pull request where any copied code comes from.

// Browser-side stand-in for the Tauri IPC layer, used only by the screenshot
// harness. State lives in memory. Chat runs replay event streams recorded
// from the real Rust agent (tests/fixtures/demo-events.json), so the frontend
// renders exactly what the backend emits.
(() => {
  const FIXTURES = window.__EDU_FIXTURES__ || {};
  const callbacks = new Map();
  let seq = 0;
  const now = () => new Date().toISOString();
  const id = () => (crypto.randomUUID ? crypto.randomUUID() : String(Math.random()).slice(2));
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  const model = (id, label, note) => ({ id, label, note });
  const CATALOG = [
    { id: "anthropic", label: "Claude", company: "Anthropic", tagline: "Claude models by Anthropic", keyPlaceholder: "sk-ant-…", keyUrl: "https://console.anthropic.com/settings/keys", envVar: "ANTHROPIC_API_KEY",
      models: [model("claude-opus-5-5", "Claude Opus 5.5", "Most capable; best for rich interactives"), model("claude-sonnet-5-5", "Claude Sonnet 5.5", "Faster and lower cost"), model("claude-haiku-5-5", "Claude Haiku 5.5", "Fastest and cheapest; simpler answers")],
      routerModels: [model("claude-haiku-5-5", "Claude Haiku 5.5", "Fast and inexpensive (default)")] },
    { id: "openai", label: "OpenAI", company: "OpenAI", tagline: "GPT models by OpenAI", keyPlaceholder: "sk-…", keyUrl: "https://platform.openai.com/api-keys", envVar: "OPENAI_API_KEY",
      models: [model("gpt-6-astra", "GPT-6 Astra", "Most capable; highest cost"), model("gpt-6.1-sol", "GPT-6.1 Sol", "Balanced capability and cost (default)"), model("gpt-6-luna", "GPT-6 Luna", "Fastest and cheapest"), model("gpt-5.6-sol", "GPT-5.6 Sol", "GPT-5.6 flagship"), model("gpt-5.6-terra", "GPT-5.6 Terra", "GPT-5.6, balanced capability and cost"), model("gpt-5.6-luna", "GPT-5.6 Luna", "GPT-5.6, fast and low cost"), model("gpt-5.5", "GPT-5.5", "GPT-5.5 flagship"), model("gpt-5.5-pro", "GPT-5.5 Pro", "Extra compute; no live streaming; answers can take minutes; very high cost")],
      routerModels: [model("gpt-6-luna", "GPT-6 Luna", "Fast and inexpensive (default)")] },
    { id: "gemini", label: "Gemini", company: "Google", tagline: "Gemini models by Google", keyPlaceholder: "AIza…", keyUrl: "https://aistudio.google.com/apikey", envVar: "GEMINI_API_KEY",
      models: [model("gemini-3.1-pro-preview", "Gemini 3.1 Pro (preview)", "Most capable; no free tier"), model("gemini-3.8-flash", "Gemini 3.8 Flash", "Fast and capable (default)"), model("gemini-3.5-flash-lite", "Gemini 3.5 Flash-Lite", "Fastest and cheapest")],
      routerModels: [model("gemini-3.1-flash-lite", "Gemini 3.1 Flash-Lite", "Fast and inexpensive (default)")] },
  ];

  const state = {
    settings: {
      theme: "light",
      provider: "demo",
      model: "claude-opus-5-5",
      routerModel: "claude-haiku-5-5",
      openaiModel: "gpt-6.1-sol",
      openaiRouterModel: "gpt-6-luna",
      openaiBaseUrl: "",
      geminiModel: "gemini-3.8-flash",
      geminiRouterModel: "gemini-3.1-flash-lite",
      geminiBaseUrl: "",
      effort: "medium",
      routerEnabled: true,
      refusalFallback: true,
      defaultStyle: "direct",
      onboarded: false,
      textScale: 1,
      highContrast: false,
      reduceMotion: false,
      relaxedSpacing: false,
      readingFont: "hyperlegible",
      readAloud: true,
      apiKeys: { anthropic: null, openai: null, gemini: null },
      dataDir: "/Users/student/Library/Application Support/com.explorable.desktop",
      version: "0.1.0",
    },
    courses: [],
    conversations: [],
    messages: {},
    artifacts: [],
    decks: [],
    cards: [],
    materials: [],
  };

  // Seed data for the Review, Progress, Materials, and Library views.
  const seed = () => {
    const course = {
      id: "course-thermo",
      name: "Thermodynamics",
      code: "ME 310",
      level: "advanced",
      discipline: "Mechanical engineering",
      description: "Laws of thermodynamics, cycles, and entropy.",
      color: "teal",
      createdAt: now(),
      updatedAt: now(),
    };
    state.courses.push(course);
    const deck = { id: "deck-1", courseId: course.id, title: "Entropy and the second law", description: "", createdAt: now(), updatedAt: now() };
    state.decks.push(deck);
    const pairs = [
      ["State the second law in terms of entropy", "The entropy of an isolated system never decreases; it stays constant only for reversible processes."],
      ["Carnot efficiency formula", "$\\eta = 1 - \\dfrac{T_C}{T_H}$, with temperatures in kelvin."],
      ["Clausius inequality", "$\\oint \\dfrac{\\delta Q}{T} \\le 0$ for any cycle."],
      ["Why can't a heat engine be 100% efficient?", "Some heat must be rejected to a cold reservoir to keep total entropy from decreasing."],
    ];
    pairs.forEach(([front, back], i) =>
      state.cards.push({ id: `card-${i}`, deckId: deck.id, position: i, front, back, hint: "", ease: 2.5, intervalDays: 0, reps: 0, lapses: 0, dueAt: now(), lastReviewedAt: null, createdAt: now() }),
    );
    state.materials.push(
      { id: "mat-1", courseId: course.id, title: "Week 5: Entropy and the Clausius inequality", sourceName: "week5-entropy.pdf", wordCount: 4120, chunkCount: 21, createdAt: now() },
      { id: "mat-2", courseId: course.id, title: "ME 310 syllabus", sourceName: "syllabus.md", wordCount: 1680, chunkCount: 8, createdAt: now() },
    );
  };
  seed();

  const progress = () => {
    const activity = [];
    for (let i = 29; i >= 0; i--) {
      const d = new Date(Date.now() - i * 86400000);
      const date = d.toISOString().slice(0, 10);
      const wave = Math.max(0, Math.round(6 + 5 * Math.sin(i / 3) + (i % 4 === 0 ? 4 : 0)));
      activity.push({ date, reviews: i > 24 ? 0 : wave, questions: i % 5 === 1 ? 5 : i % 7 === 2 ? 4 : 0 });
    }
    return {
      quizzesTaken: 7,
      averageScore: 0.78,
      cardsTotal: 36,
      cardsDue: 9,
      cardsLearned: 27,
      reviewsLast7: 48,
      streakDays: 5,
      topics: [
        { topic: "Entropy", attempts: 3, accuracy: 0.86, lastAttemptAt: now() },
        { topic: "Carnot cycle", attempts: 2, accuracy: 0.7, lastAttemptAt: now() },
        { topic: "First law", attempts: 2, accuracy: 0.55, lastAttemptAt: now() },
      ],
      activity,
      recentAttempts: [
        { id: "a1", courseId: "course-thermo", quizKey: "k1", title: "Entropy check-in", topic: "Entropy", score: 5, maxScore: 5, createdAt: now() },
        { id: "a2", courseId: "course-thermo", quizKey: "k2", title: "Carnot cycle practice", topic: "Carnot cycle", score: 3, maxScore: 5, createdAt: now() },
      ],
    };
  };

  const deckView = (d) => {
    const cards = state.cards.filter((c) => c.deckId === d.id);
    return { ...d, cardCount: cards.length, dueCount: 0, newCount: cards.filter((c) => c.reps === 0).length };
  };

  async function replay(conversationId, mode, text, channel) {
    const key = `${mode}:${text}`;
    const fallback = `${mode}:What can you do?` in FIXTURES ? `${mode}:What can you do?` : "learn:What can you do?";
    const events = FIXTURES[key] || FIXTURES[Object.keys(FIXTURES).find((k) => k.startsWith(`${mode}:`) && text.startsWith(k.slice(mode.length + 1, mode.length + 25))) || fallback];
    const send = callbacks.get(channel.id);
    const runSuffix = id().slice(0, 8);
    let index = 0;
    for (const raw of events) {
      const ev = JSON.parse(JSON.stringify(raw));
      // Re-home the recorded run in this conversation with fresh message ids.
      const remap = (m) => {
        if (!m) return;
        m.conversationId = conversationId;
        m.id = `${m.id}-${runSuffix}`;
      };
      if (ev.conversationId) ev.conversationId = conversationId;
      if (ev.messageId) ev.messageId = `${ev.messageId}-${runSuffix}`;
      if (ev.message) remap(ev.message);
      if (ev.type === "userMessage") {
        ev.message.content = ev.message.content.map((b) => (b.type === "text" ? { ...b, text } : b));
      }
      if (ev.type === "userMessage" || ev.type === "assistantDone" || ev.type === "toolResults") {
        (state.messages[conversationId] ||= []).push(ev.message);
      }
      if (ev.type === "route") {
        const m = (state.messages[conversationId] || []).find((x) => x.id === ev.messageId);
        if (m) m.meta = { ...m.meta, route: ev.decision };
      }
      if (ev.type === "titleUpdated") {
        const c = state.conversations.find((x) => x.id === conversationId);
        if (c) c.title = ev.title;
      }
      const delay = ev.type === "textDelta" ? 14 : ev.type === "toolInput" ? 260 : ev.type === "started" ? 0 : 40;
      await sleep(window.__EDU_SPEED__ === "fast" ? 1 : delay);
      send && send({ index: index++, message: ev });
    }
  }

  const handlers = {
    get_settings: () => state.settings,
    update_settings: ({ patch }) => Object.assign(state.settings, patch),
    set_api_key: ({ provider }) => {
      state.settings.apiKeys = { ...state.settings.apiKeys, [provider]: "keychain" };
      state.settings.provider = provider;
      return state.settings;
    },
    clear_api_key: ({ provider }) => {
      state.settings.apiKeys = { ...state.settings.apiKeys, [provider]: null };
      return state.settings;
    },
    model_catalog: () => CATALOG,
    test_api_key: () => null,
    list_courses: () => state.courses,
    create_course: ({ input }) => {
      const c = { ...input, id: id(), createdAt: now(), updatedAt: now() };
      state.courses.push(c);
      return c;
    },
    update_course: ({ id: cid, input }) => Object.assign(state.courses.find((c) => c.id === cid), input),
    delete_course: ({ id: cid }) => {
      state.courses = state.courses.filter((c) => c.id !== cid);
    },
    list_conversations: ({ courseId, mode }) =>
      state.conversations.filter((c) => (c.courseId ?? null) === (courseId ?? null) && c.mode === mode).sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)),
    create_conversation: ({ courseId, mode, style }) => {
      const c = { id: id(), courseId, mode, style, title: "New chat", pending: null, createdAt: now(), updatedAt: now() };
      state.conversations.push(c);
      state.messages[c.id] = [];
      return c;
    },
    get_conversation: ({ id: cid }) => ({ conversation: state.conversations.find((c) => c.id === cid), messages: state.messages[cid] || [], running: false }),
    rename_conversation: ({ id: cid, title }) => {
      state.conversations.find((c) => c.id === cid).title = title;
    },
    set_conversation_style: ({ id: cid, style }) => {
      state.conversations.find((c) => c.id === cid).style = style;
    },
    delete_conversation: ({ id: cid }) => {
      state.conversations = state.conversations.filter((c) => c.id !== cid);
    },
    send_message: ({ conversationId, text, onEvent }) => {
      const conv = state.conversations.find((c) => c.id === conversationId);
      conv.updatedAt = now();
      void replay(conversationId, conv.mode, text, onEvent);
      return "run";
    },
    answer_choice: () => "run",
    retry_run: () => "run",
    cancel_run: () => true,
    list_artifacts: ({ courseId, kind }) => state.artifacts.filter((a) => (!courseId || a.courseId === courseId) && (!kind || a.kind === kind)),
    get_artifact: ({ id: aid }) => state.artifacts.find((a) => a.id === aid),
    save_artifact: ({ input }) => {
      const a = { ...input, id: id(), createdAt: now(), updatedAt: now() };
      state.artifacts.unshift(a);
      return a;
    },
    rename_artifact: () => null,
    delete_artifact: ({ id: aid }) => {
      state.artifacts = state.artifacts.filter((a) => a.id !== aid);
    },
    list_decks: () => state.decks.map(deckView),
    get_deck: ({ id: did }) => ({ deck: deckView(state.decks.find((d) => d.id === did)), cards: state.cards.filter((c) => c.deckId === did) }),
    create_deck: ({ courseId, title, cards }) => {
      const d = { id: id(), courseId, title, description: "", createdAt: now(), updatedAt: now() };
      state.decks.push(d);
      cards.forEach((c, i) => state.cards.push({ ...c, id: id(), deckId: d.id, position: i, ease: 2.5, intervalDays: 0, reps: 0, lapses: 0, dueAt: now(), lastReviewedAt: null, createdAt: now() }));
      return deckView(d);
    },
    due_cards: ({ deckId }) =>
      state.cards
        .filter((c) => c.reps === 0 && (!deckId || c.deckId === deckId))
        .map((card) => ({ card, deckTitle: state.decks.find((d) => d.id === card.deckId)?.title ?? "", previews: ["10m", "1d", "1d", "4d"] })),
    review_card: ({ cardId }) => {
      const c = state.cards.find((x) => x.id === cardId);
      c.reps = 1;
      return c;
    },
    rename_deck: () => null,
    delete_deck: () => null,
    add_card: () => null,
    update_card: () => null,
    delete_card: () => null,
    get_progress: progress,
    record_quiz_attempt: ({ input }) => ({ ...input, id: id(), createdAt: now() }),
    quiz_attempts: () => [],
    list_materials: ({ courseId }) => state.materials.filter((m) => m.courseId === courseId),
    get_material: () => ({ title: "Week 5", content: "Entropy is a state function…" }),
    add_material_text: () => null,
    import_material: () => null,
    delete_material: () => null,
    search_materials: () => [
      { materialId: "mat-1", title: "Week 5: Entropy and the Clausius inequality", ordinal: 3, content: "", snippet: "…the «Carnot» «efficiency» depends only on the reservoir temperatures, which is why…" },
    ],
    export_item: () => ({ contents: "", extension: "md" }),
    write_text_file: () => null,
    open_external: () => null,
    "plugin:dialog|ask": () => true,
    "plugin:dialog|confirm": () => true,
    "plugin:dialog|message": () => null,
    "plugin:dialog|save": () => null,
    "plugin:dialog|open": () => null,
  };

  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    transformCallback(cb) {
      const n = ++seq;
      callbacks.set(n, cb);
      return n;
    },
    unregisterCallback(n) {
      callbacks.delete(n);
    },
    convertFileSrc: (p) => p,
    async invoke(cmd, args) {
      const h = handlers[cmd];
      if (!h) throw `Mock has no handler for ${cmd}`;
      await sleep(5);
      const result = await h(args || {});
      // Real IPC hands back freshly deserialized values, never shared references.
      return result === undefined ? null : JSON.parse(JSON.stringify(result));
    },
  };
  window.__EDU_STATE__ = state;
})();

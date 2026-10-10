import { create } from "zustand";
import { api, errorMessage } from "./api";
import { applyTheme, storedTheme } from "./theme";
import type {
  ArtifactKind,
  Block,
  ChatEvent,
  Conversation,
  Course,
  Mode,
  PendingChoice,
  SettingsView,
  StoredMessage,
  Style,
  Theme,
  View,
} from "./types";

export interface ConvState {
  messages: StoredMessage[];
  running: boolean;
  error: { message: string; retryable: boolean } | null;
  notices: { level: "info" | "warning"; text: string }[];
  pending: PendingChoice | null;
}

export interface Toast {
  id: number;
  text: string;
  kind: "info" | "success" | "error";
}

export interface PresentItem {
  kind: ArtifactKind;
  title: string;
  payload: Record<string, unknown>;
  key: string;
}

interface Store {
  ready: boolean;
  bootError: string | null;
  settings: SettingsView | null;
  courses: Course[];
  courseId: string | null;
  mode: Mode;
  view: View;
  conversations: Conversation[];
  activeId: string | null;
  convs: Record<string, ConvState>;
  draftStyle: Style;
  toasts: Toast[];
  present: PresentItem | null;
  reviewDeckId: string | null;
  dueCount: number;
  courseDialog: { mode: "create" } | { mode: "edit"; course: Course } | null;

  boot: () => Promise<void>;
  setTheme: (t: Theme) => Promise<void>;
  saveSettings: (patch: Partial<SettingsView>) => Promise<void>;
  /** Applies a preference immediately, then saves it. */
  updatePrefs: (patch: Partial<SettingsView>) => Promise<void>;
  setSettings: (s: SettingsView) => void;
  setMode: (m: Mode) => void;
  setCourse: (id: string | null) => void;
  setView: (v: View) => void;
  openCourseDialog: (d: Store["courseDialog"]) => void;
  refreshCourses: () => Promise<void>;
  refreshConversations: () => Promise<void>;
  refreshDue: () => Promise<void>;
  openConversation: (id: string) => Promise<void>;
  newChat: () => void;
  setStyle: (style: Style) => Promise<void>;
  /** Resolves false if the message couldn't be sent (the error is shown in the chat). */
  send: (text: string) => Promise<boolean>;
  answerChoice: (toolUseId: string, answer: string) => Promise<boolean>;
  retry: () => Promise<void>;
  stop: () => Promise<void>;
  deleteConversation: (id: string) => Promise<void>;
  applyEvent: (conversationId: string, e: ChatEvent) => void;
  toast: (text: string, kind?: Toast["kind"]) => void;
  dismissToast: (id: number) => void;
  setPresent: (p: PresentItem | null) => void;
  studyDeck: (deckId: string | null) => void;
}

const PREFS = "edu.prefs";

function loadPrefs(): { mode?: Mode; courseId?: string | null } {
  try {
    return JSON.parse(localStorage.getItem(PREFS) ?? "{}");
  } catch {
    return {};
  }
}

function savePrefs(mode: Mode, courseId: string | null) {
  try {
    localStorage.setItem(PREFS, JSON.stringify({ mode, courseId }));
  } catch {
    // Preferences are a convenience only.
  }
}

const emptyConv = (): ConvState => ({ messages: [], running: false, error: null, notices: [], pending: null });

let toastSeq = 0;
/** Polls chats whose run continues in the backend without a channel to this window (e.g. after a reload). */
const watchers = new Map<string, number>();

/** Replaces one message by id, cloning only what changes. */
function patchMessage(messages: StoredMessage[], id: string, fn: (m: StoredMessage) => StoredMessage) {
  const i = messages.findIndex((m) => m.id === id);
  if (i < 0) return messages;
  const next = messages.slice();
  next[i] = fn(messages[i]);
  return next;
}

function patchBlock(m: StoredMessage, index: number, fn: (b: Block) => Block): StoredMessage {
  const content = m.content.slice();
  content[index] = fn(content[index] ?? { type: "text", text: "" });
  return { ...m, content };
}

export const useStore = create<Store>((set, get) => {
  const updateConv = (id: string, fn: (c: ConvState) => ConvState) =>
    set((s) => ({ convs: { ...s.convs, [id]: fn(s.convs[id] ?? emptyConv()) } }));

  const handlerFor = (id: string) => (e: ChatEvent) => get().applyEvent(id, e);

  return {
    ready: false,
    bootError: null,
    settings: null,
    courses: [],
    courseId: null,
    mode: "learn",
    view: "chat",
    conversations: [],
    activeId: null,
    convs: {},
    draftStyle: "direct",
    toasts: [],
    present: null,
    reviewDeckId: null,
    dueCount: 0,
    courseDialog: null,

    boot: async () => {
      try {
        applyTheme(storedTheme());
        const [settings, courses] = await Promise.all([api.getSettings(), api.listCourses()]);
        applyTheme(settings.theme);
        const prefs = loadPrefs();
        const courseId = prefs.courseId && courses.some((c) => c.id === prefs.courseId) ? prefs.courseId : null;
        set({
          settings,
          courses,
          courseId,
          mode: prefs.mode === "teach" ? "teach" : "learn",
          draftStyle: settings.defaultStyle,
          ready: true,
        });
        await Promise.all([get().refreshConversations(), get().refreshDue()]);
      } catch (err) {
        set({ bootError: errorMessage(err), ready: true });
      }
    },

    setTheme: async (theme) => {
      applyTheme(theme);
      const s = get().settings;
      if (s) set({ settings: { ...s, theme } });
      try {
        set({ settings: await api.updateSettings({ theme }) });
      } catch (err) {
        get().toast(errorMessage(err), "error");
      }
    },

    saveSettings: async (patch) => {
      try {
        const next = await api.updateSettings(patch);
        set({ settings: next });
        if (patch.defaultStyle) set({ draftStyle: patch.defaultStyle });
      } catch (err) {
        get().toast(errorMessage(err), "error");
        throw err;
      }
    },

    updatePrefs: async (patch) => {
      const before = get().settings;
      if (before) set({ settings: { ...before, ...patch } });
      try {
        set({ settings: await api.updateSettings(patch) });
      } catch (err) {
        if (before) set({ settings: before });
        get().toast(errorMessage(err), "error");
      }
    },

    setSettings: (settings) => set({ settings }),

    setMode: (mode) => {
      if (mode === get().mode) return;
      const view = get().view;
      const keepView = view === "library" || view === "materials" || view === "settings";
      set({ mode, activeId: null, view: keepView ? view : "chat" });
      savePrefs(mode, get().courseId);
      void get().refreshConversations();
    },

    setCourse: (courseId) => {
      const view = get().view;
      set({
        courseId,
        activeId: null,
        view: view === "materials" && !courseId ? "chat" : view,
        reviewDeckId: null,
      });
      savePrefs(get().mode, courseId);
      void get().refreshConversations();
      void get().refreshDue();
    },

    setView: (view) => set({ view }),
    openCourseDialog: (courseDialog) => set({ courseDialog }),

    refreshCourses: async () => {
      const courses = await api.listCourses();
      const { courseId } = get();
      set({ courses, courseId: courseId && courses.some((c) => c.id === courseId) ? courseId : null });
    },

    refreshConversations: async () => {
      const { courseId, mode } = get();
      try {
        const conversations = await api.listConversations(courseId, mode);
        if (get().courseId === courseId && get().mode === mode) set({ conversations });
      } catch (err) {
        get().toast(errorMessage(err), "error");
      }
    },

    refreshDue: async () => {
      try {
        const due = await api.dueCards(get().courseId, null, 500);
        set({ dueCount: due.length });
      } catch {
        // The badge is informational.
      }
    },

    openConversation: async (id) => {
      set({ activeId: id, view: "chat" });
      const existing = get().convs[id];
      if (existing?.running) return;
      try {
        const detail = await api.getConversation(id);
        // A send that started while we were fetching owns this chat's state now.
        if (get().convs[id]?.running) return;
        const conv = detail.conversation;
        if (!get().conversations.some((c) => c.id === id)) {
          // Opened from elsewhere (e.g. the Library): move to its course and mode.
          set({ courseId: conv.courseId, mode: conv.mode });
          savePrefs(conv.mode, conv.courseId);
          await get().refreshConversations();
          set({ activeId: id, view: "chat" });
        }
        updateConv(id, (c) => ({
          ...c,
          messages: detail.messages,
          pending: detail.conversation.pending,
          running: detail.running,
        }));
        if (detail.running && !watchers.has(id)) {
          const timer = window.setInterval(async () => {
            const next = await api.getConversation(id).catch(() => null);
            if (!next || next.running) return;
            clearInterval(timer);
            watchers.delete(id);
            updateConv(id, (c) => ({ ...c, messages: next.messages, pending: next.conversation.pending, running: false }));
            void get().refreshConversations();
          }, 1500);
          watchers.set(id, timer);
        }
      } catch (err) {
        get().toast(errorMessage(err), "error");
      }
    },

    newChat: () => set({ activeId: null, view: "chat", draftStyle: get().settings?.defaultStyle ?? "direct" }),

    setStyle: async (style) => {
      const id = get().activeId;
      if (!id) {
        set({ draftStyle: style });
        return;
      }
      set((s) => ({ conversations: s.conversations.map((c) => (c.id === id ? { ...c, style } : c)) }));
      try {
        await api.setConversationStyle(id, style);
      } catch (err) {
        get().toast(errorMessage(err), "error");
      }
    },

    send: async (text) => {
      const trimmed = text.trim();
      if (!trimmed) return false;
      let id = get().activeId;
      try {
        if (!id) {
          const { courseId, mode, draftStyle } = get();
          const conv = await api.createConversation(courseId, mode, draftStyle);
          id = conv.id;
          set((s) => ({ conversations: [conv, ...s.conversations], activeId: conv.id }));
          updateConv(conv.id, () => emptyConv());
        }
        const convId = id;
        updateConv(convId, (c) => ({ ...c, running: true, error: null, notices: [] }));
        await api.sendMessage(convId, trimmed, handlerFor(convId));
        return true;
      } catch (err) {
        if (id) updateConv(id, (c) => ({ ...c, running: false, error: { message: errorMessage(err), retryable: false } }));
        else get().toast(errorMessage(err), "error");
        return false;
      }
    },

    answerChoice: async (toolUseId, answer) => {
      const id = get().activeId;
      if (!id) return false;
      updateConv(id, (c) => ({ ...c, running: true, error: null }));
      try {
        await api.answerChoice(id, toolUseId, answer, handlerFor(id));
        return true;
      } catch (err) {
        updateConv(id, (c) => ({ ...c, running: false, error: { message: errorMessage(err), retryable: false } }));
        return false;
      }
    },

    retry: async () => {
      const id = get().activeId;
      if (!id) return;
      updateConv(id, (c) => ({ ...c, running: true, error: null, notices: [] }));
      try {
        await api.retryRun(id, handlerFor(id));
      } catch (err) {
        updateConv(id, (c) => ({ ...c, running: false, error: { message: errorMessage(err), retryable: false } }));
      }
    },

    stop: async () => {
      const id = get().activeId;
      if (id) await api.cancelRun(id).catch(() => false);
    },

    deleteConversation: async (id) => {
      try {
        await api.deleteConversation(id);
        set((s) => {
          const convs = { ...s.convs };
          delete convs[id];
          return {
            conversations: s.conversations.filter((c) => c.id !== id),
            convs,
            activeId: s.activeId === id ? null : s.activeId,
          };
        });
      } catch (err) {
        get().toast(errorMessage(err), "error");
      }
    },

    applyEvent: (id, e) => {
      switch (e.type) {
        case "started":
          updateConv(id, (c) => ({ ...c, running: true, error: null, notices: [] }));
          break;
        case "userMessage":
          updateConv(id, (c) => {
            if (c.messages.some((m) => m.id === e.message.id)) return c;
            const answersPending =
              c.pending && e.message.content.some((b) => b.type === "tool_result" && b.tool_use_id === c.pending?.toolUseId);
            return { ...c, messages: [...c.messages, e.message], pending: answersPending ? null : c.pending };
          });
          break;
        case "route":
          updateConv(id, (c) => ({
            ...c,
            messages: patchMessage(c.messages, e.messageId, (m) => ({ ...m, meta: { ...m.meta, route: e.decision } })),
          }));
          break;
        case "assistantStart":
          updateConv(id, (c) => ({
            ...c,
            messages: [
              ...c.messages,
              {
                id: e.messageId,
                conversationId: id,
                seq: Number.MAX_SAFE_INTEGER,
                role: "assistant",
                content: [],
                meta: { _live: true },
                createdAt: new Date().toISOString(),
              },
            ],
          }));
          break;
        case "blockStart":
          updateConv(id, (c) => ({
            ...c,
            messages: patchMessage(c.messages, e.messageId, (m) =>
              patchBlock(m, e.index, () => ({ ...e.block, _done: false })),
            ),
          }));
          break;
        case "textDelta":
          updateConv(id, (c) => ({
            ...c,
            messages: patchMessage(c.messages, e.messageId, (m) =>
              patchBlock(m, e.index, (b) => ({ ...b, text: (b.text ?? "") + e.text })),
            ),
          }));
          break;
        case "thinkingDelta":
          updateConv(id, (c) => ({
            ...c,
            messages: patchMessage(c.messages, e.messageId, (m) =>
              patchBlock(m, e.index, (b) => ({ ...b, thinking: (b.thinking ?? "") + e.text })),
            ),
          }));
          break;
        case "toolInput":
          updateConv(id, (c) => ({
            ...c,
            messages: patchMessage(c.messages, e.messageId, (m) =>
              patchBlock(m, e.index, (b) => ({ ...b, _partial: e.partial, _completeKeys: e.completeKeys })),
            ),
          }));
          break;
        case "blockStop":
          updateConv(id, (c) => ({
            ...c,
            messages: patchMessage(c.messages, e.messageId, (m) =>
              patchBlock(m, e.index, () => ({ ...e.block, _done: true })),
            ),
          }));
          break;
        case "assistantDone":
          updateConv(id, (c) => {
            const exists = c.messages.some((m) => m.id === e.message.id);
            return {
              ...c,
              messages: exists ? patchMessage(c.messages, e.message.id, () => e.message) : [...c.messages, e.message],
            };
          });
          break;
        case "toolResults":
          updateConv(id, (c) => ({ ...c, messages: [...c.messages, e.message] }));
          if (e.message.content.some((b) => typeof b.content === "string" && b.content.includes('"deck_id"'))) {
            void get().refreshDue();
          }
          break;
        case "awaiting":
          updateConv(id, (c) => ({ ...c, pending: { toolUseId: e.toolUseId, otherResults: e.otherResults } }));
          break;
        case "notice":
          updateConv(id, (c) => ({ ...c, notices: [...c.notices, { level: e.level, text: e.text }] }));
          break;
        case "titleUpdated":
          set((s) => ({
            conversations: s.conversations.map((cv) => (cv.id === e.conversationId ? { ...cv, title: e.title } : cv)),
          }));
          break;
        case "error":
          updateConv(id, (c) => ({ ...c, error: { message: e.message, retryable: e.retryable } }));
          break;
        case "finished":
          updateConv(id, (c) => ({
            ...c,
            running: false,
            // Drop placeholders that were never persisted (errors, empty cancels).
            messages: c.messages.filter((m) => !m.meta._live),
          }));
          void get().refreshConversations();
          break;
      }
    },

    toast: (text, kind = "info") => {
      const id = ++toastSeq;
      set((s) => ({ toasts: [...s.toasts.slice(-3), { id, text, kind }] }));
      setTimeout(() => get().dismissToast(id), kind === "error" ? 7000 : 3500);
    },
    dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
    setPresent: (present) => set({ present }),
    studyDeck: (reviewDeckId) => set({ reviewDeckId, view: "review" }),
  };
});

export const selectActive = (s: ReturnType<typeof useStore.getState>) =>
  s.activeId ? s.conversations.find((c) => c.id === s.activeId) ?? null : null;

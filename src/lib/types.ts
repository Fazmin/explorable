// Mirrors the Rust models (serialized camelCase) and agent events.

export type Mode = "learn" | "teach";
export type Style = "direct" | "socratic";
export type Theme = "light" | "dark" | "system";
export type View = "chat" | "library" | "review" | "progress" | "materials" | "settings";

export interface Course {
  id: string;
  name: string;
  code: string;
  level: string;
  discipline: string;
  description: string;
  color: string;
  createdAt: string;
  updatedAt: string;
}

export interface CourseInput {
  name: string;
  code: string;
  level: string;
  discipline: string;
  description: string;
  color: string;
}

export interface PendingChoice {
  toolUseId: string;
  otherResults: unknown[];
}

export interface Conversation {
  id: string;
  courseId: string | null;
  mode: Mode;
  style: Style;
  title: string;
  pending: PendingChoice | null;
  createdAt: string;
  updatedAt: string;
}

/** An Anthropic content block, plus UI-only fields while streaming. */
export interface Block {
  type: string;
  text?: string;
  thinking?: string;
  signature?: string;
  id?: string;
  name?: string;
  input?: Record<string, unknown>;
  tool_use_id?: string;
  content?: unknown;
  is_error?: boolean;
  /** Arguments parsed so far while a tool call streams. */
  _partial?: Record<string, unknown>;
  _completeKeys?: string[];
  /** True once the block has fully arrived. */
  _done?: boolean;
  [key: string]: unknown;
}

export interface RouteDecision {
  presentation: string;
  renderer: string;
  reason: string;
  source: "router" | "demo" | "model";
}

export interface StoredMessage {
  id: string;
  conversationId: string;
  seq: number;
  role: "user" | "assistant";
  content: Block[];
  meta: {
    route?: RouteDecision;
    model?: string;
    stopReason?: string;
    choiceAnswer?: string;
    toolResults?: boolean;
    [key: string]: unknown;
  };
  createdAt: string;
}

export interface ConversationDetail {
  conversation: Conversation;
  messages: StoredMessage[];
  running: boolean;
}

export interface Artifact {
  id: string;
  courseId: string | null;
  conversationId: string | null;
  kind: ArtifactKind;
  title: string;
  payload: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
}

export type ArtifactKind =
  | "interactive"
  | "quiz"
  | "flashcards"
  | "worked_example"
  | "lesson_plan"
  | "rubric"
  | "table"
  | "note";

export interface Material {
  id: string;
  courseId: string;
  title: string;
  sourceName: string;
  wordCount: number;
  chunkCount: number;
  createdAt: string;
}

export interface MaterialHit {
  materialId: string;
  title: string;
  ordinal: number;
  content: string;
  snippet: string;
}

export interface Deck {
  id: string;
  courseId: string | null;
  title: string;
  description: string;
  cardCount: number;
  dueCount: number;
  newCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface Card {
  id: string;
  deckId: string;
  position: number;
  front: string;
  back: string;
  hint: string;
  ease: number;
  intervalDays: number;
  reps: number;
  lapses: number;
  dueAt: string;
  lastReviewedAt: string | null;
  createdAt: string;
}

export interface DueCard {
  card: Card;
  deckTitle: string;
  previews: [string, string, string, string];
}

export interface QuizAttempt {
  id: string;
  courseId: string | null;
  quizKey: string;
  title: string;
  topic: string;
  score: number;
  maxScore: number;
  createdAt: string;
}

export interface Progress {
  quizzesTaken: number;
  averageScore: number | null;
  cardsTotal: number;
  cardsDue: number;
  cardsLearned: number;
  reviewsLast7: number;
  streakDays: number;
  topics: { topic: string; attempts: number; accuracy: number; lastAttemptAt: string }[];
  activity: { date: string; reviews: number; questions: number }[];
  recentAttempts: QuizAttempt[];
}

export type ProviderId = "anthropic" | "openai" | "gemini" | "demo";
export type KeyProvider = Exclude<ProviderId, "demo">;

export interface Settings {
  theme: Theme;
  provider: ProviderId;
  /** Claude answer and router models. */
  model: string;
  routerModel: string;
  openaiModel: string;
  openaiRouterModel: string;
  openaiBaseUrl: string;
  geminiModel: string;
  geminiRouterModel: string;
  geminiBaseUrl: string;
  effort: string;
  routerEnabled: boolean;
  /** Whole-app zoom, 1.0 to 1.5. */
  textScale: number;
  highContrast: boolean;
  reduceMotion: boolean;
  relaxedSpacing: boolean;
  readingFont: "hyperlegible" | "serif" | "system";
  readAloud: boolean;
  refusalFallback: boolean;
  defaultStyle: Style;
  onboarded: boolean;
}

export interface SettingsView extends Settings {
  apiKeys: Record<KeyProvider, "keychain" | "env" | null>;
  dataDir: string;
  version: string;
}

export type ChatEvent =
  | { type: "started"; runId: string; conversationId: string }
  | { type: "userMessage"; message: StoredMessage }
  | { type: "route"; messageId: string; decision: RouteDecision }
  | { type: "assistantStart"; messageId: string }
  | { type: "blockStart"; messageId: string; index: number; block: Block }
  | { type: "textDelta"; messageId: string; index: number; text: string }
  | { type: "thinkingDelta"; messageId: string; index: number; text: string }
  | { type: "toolInput"; messageId: string; index: number; partial: Record<string, unknown>; completeKeys: string[] }
  | { type: "blockStop"; messageId: string; index: number; block: Block }
  | { type: "assistantDone"; message: StoredMessage }
  | { type: "toolResults"; message: StoredMessage }
  | { type: "awaiting"; toolUseId: string; otherResults: unknown[] }
  | { type: "notice"; level: "info" | "warning"; text: string }
  | { type: "titleUpdated"; conversationId: string; title: string }
  | { type: "error"; message: string; retryable: boolean }
  | { type: "finished"; cancelled: boolean };

export interface CatalogModel {
  id: string;
  label: string;
  note: string;
}

export interface ProviderInfo {
  id: KeyProvider;
  label: string;
  company: string;
  tagline: string;
  keyPlaceholder: string;
  keyUrl: string;
  envVar: string;
  models: CatalogModel[];
  routerModels: CatalogModel[];
}

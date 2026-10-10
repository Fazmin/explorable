// Typed wrappers around the Rust commands.
import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  Artifact,
  ArtifactKind,
  Card,
  ChatEvent,
  Conversation,
  ConversationDetail,
  Course,
  CourseInput,
  Deck,
  DueCard,
  KeyProvider,
  Material,
  MaterialHit,
  Mode,
  Progress,
  ProviderInfo,
  QuizAttempt,
  Settings,
  SettingsView,
  Style,
} from "./types";

/** Command errors arrive as plain strings; normalize anything else. */
export function errorMessage(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return "Something went wrong.";
}

function channel(onEvent: (e: ChatEvent) => void): Channel<ChatEvent> {
  const ch = new Channel<ChatEvent>();
  ch.onmessage = onEvent;
  return ch;
}

export const api = {
  // Settings
  getSettings: () => invoke<SettingsView>("get_settings"),
  updateSettings: (patch: Partial<Settings>) => invoke<SettingsView>("update_settings", { patch }),
  setApiKey: (provider: KeyProvider, key: string) => invoke<SettingsView>("set_api_key", { provider, key }),
  clearApiKey: (provider: KeyProvider) => invoke<SettingsView>("clear_api_key", { provider }),
  testApiKey: (provider: KeyProvider) => invoke<void>("test_api_key", { provider }),
  modelCatalog: () => invoke<ProviderInfo[]>("model_catalog"),

  // Courses
  listCourses: () => invoke<Course[]>("list_courses"),
  createCourse: (input: CourseInput) => invoke<Course>("create_course", { input }),
  updateCourse: (id: string, input: CourseInput) => invoke<Course>("update_course", { id, input }),
  deleteCourse: (id: string) => invoke<void>("delete_course", { id }),

  // Conversations
  listConversations: (courseId: string | null, mode: Mode) =>
    invoke<Conversation[]>("list_conversations", { courseId, mode }),
  createConversation: (courseId: string | null, mode: Mode, style: Style) =>
    invoke<Conversation>("create_conversation", { courseId, mode, style }),
  getConversation: (id: string) => invoke<ConversationDetail>("get_conversation", { id }),
  renameConversation: (id: string, title: string) => invoke<void>("rename_conversation", { id, title }),
  setConversationStyle: (id: string, style: Style) => invoke<void>("set_conversation_style", { id, style }),
  deleteConversation: (id: string) => invoke<void>("delete_conversation", { id }),
  sendMessage: (conversationId: string, text: string, onEvent: (e: ChatEvent) => void) =>
    invoke<string>("send_message", { conversationId, text, onEvent: channel(onEvent) }),
  answerChoice: (conversationId: string, toolUseId: string, answer: string, onEvent: (e: ChatEvent) => void) =>
    invoke<string>("answer_choice", { conversationId, toolUseId, answer, onEvent: channel(onEvent) }),
  retryRun: (conversationId: string, onEvent: (e: ChatEvent) => void) =>
    invoke<string>("retry_run", { conversationId, onEvent: channel(onEvent) }),
  cancelRun: (conversationId: string) => invoke<boolean>("cancel_run", { conversationId }),

  // Library
  listArtifacts: (courseId: string | null, kind: ArtifactKind | null, query: string | null) =>
    invoke<Artifact[]>("list_artifacts", { courseId, kind, query }),
  getArtifact: (id: string) => invoke<Artifact>("get_artifact", { id }),
  saveArtifact: (input: {
    courseId: string | null;
    conversationId: string | null;
    kind: ArtifactKind;
    title: string;
    payload: unknown;
  }) => invoke<Artifact>("save_artifact", { input }),
  renameArtifact: (id: string, title: string) => invoke<void>("rename_artifact", { id, title }),
  deleteArtifact: (id: string) => invoke<void>("delete_artifact", { id }),

  // Decks
  listDecks: (courseId: string | null) => invoke<Deck[]>("list_decks", { courseId }),
  getDeck: (id: string) => invoke<{ deck: Deck; cards: Card[] }>("get_deck", { id }),
  createDeck: (courseId: string | null, title: string, description: string, cards: { front: string; back: string; hint: string }[]) =>
    invoke<Deck>("create_deck", { courseId, title, description, cards }),
  renameDeck: (id: string, title: string) => invoke<void>("rename_deck", { id, title }),
  deleteDeck: (id: string) => invoke<void>("delete_deck", { id }),
  addCard: (deckId: string, input: { front: string; back: string; hint: string }) =>
    invoke<Card>("add_card", { deckId, input }),
  updateCard: (id: string, input: { front: string; back: string; hint: string }) =>
    invoke<Card>("update_card", { id, input }),
  deleteCard: (id: string) => invoke<void>("delete_card", { id }),
  dueCards: (courseId: string | null, deckId: string | null, limit?: number) =>
    invoke<DueCard[]>("due_cards", { courseId, deckId, limit }),
  reviewCard: (cardId: string, rating: number) => invoke<Card>("review_card", { cardId, rating }),

  // Progress
  getProgress: (courseId: string | null) => invoke<Progress>("get_progress", { courseId }),
  recordQuizAttempt: (input: {
    courseId: string | null;
    conversationId: string | null;
    quizKey: string;
    title: string;
    topic: string;
    score: number;
    maxScore: number;
    details: unknown[];
  }) => invoke<QuizAttempt>("record_quiz_attempt", { input }),
  quizAttempts: (quizKey: string) => invoke<QuizAttempt[]>("quiz_attempts", { quizKey }),

  // Materials
  listMaterials: (courseId: string) => invoke<Material[]>("list_materials", { courseId }),
  getMaterial: (id: string) => invoke<{ title: string; content: string }>("get_material", { id }),
  addMaterialText: (courseId: string, title: string, content: string) =>
    invoke<Material>("add_material_text", { courseId, title, content }),
  importMaterial: (courseId: string, path: string) => invoke<Material>("import_material", { courseId, path }),
  deleteMaterial: (id: string) => invoke<void>("delete_material", { id }),
  searchMaterials: (courseId: string, query: string) =>
    invoke<MaterialHit[]>("search_materials", { courseId, query }),

  // System
  exportItem: (kind: string, format: string, payload: unknown) =>
    invoke<{ contents: string; extension: string }>("export_item", { kind, format, payload }),
  writeTextFile: (path: string, contents: string) => invoke<void>("write_text_file", { path, contents }),
  openExternal: (url: string) => invoke<void>("open_external", { url }),
};

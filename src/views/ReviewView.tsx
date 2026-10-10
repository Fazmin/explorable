import { useCallback, useEffect, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { CheckCircle2, Download, Layers, Pencil, Plus, Trash2, X } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import { useStore } from "../lib/store";
import { exportArtifact } from "../lib/exporting";
import type { Card, Deck, DueCard } from "../lib/types";
import { FlashcardFace } from "../renderers/Flashcards";

const RATINGS = [
  { value: 1, label: "Again", cls: "again" },
  { value: 2, label: "Hard", cls: "" },
  { value: 3, label: "Good", cls: "" },
  { value: 4, label: "Easy", cls: "" },
];

export function ReviewView() {
  const courseId = useStore((s) => s.courseId);
  const deckId = useStore((s) => s.reviewDeckId);
  const mode = useStore((s) => s.mode);
  const { toast, refreshDue, studyDeck } = useStore.getState();
  const [decks, setDecks] = useState<Deck[]>([]);
  const [queue, setQueue] = useState<DueCard[]>([]);
  const [pos, setPos] = useState(0);
  const [flipped, setFlipped] = useState(false);
  const [reviewed, setReviewed] = useState(0);
  const [busy, setBusy] = useState(false);
  const [managing, setManaging] = useState(false);

  const loadDecks = useCallback(async () => {
    try {
      setDecks(await api.listDecks(courseId));
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  }, [courseId, toast]);

  const loadQueue = useCallback(async () => {
    try {
      setQueue(await api.dueCards(courseId, deckId, 100));
      setPos(0);
      setFlipped(false);
      setReviewed(0);
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  }, [courseId, deckId, toast]);

  useEffect(() => {
    void loadDecks();
    void loadQueue();
    setManaging(false);
  }, [loadDecks, loadQueue]);

  const current = queue[pos];

  const rate = useCallback(
    async (rating: number) => {
      if (!current || busy) return;
      setBusy(true);
      try {
        await api.reviewCard(current.card.id, rating);
        setReviewed((n) => n + 1);
        setPos((p) => p + 1);
        setFlipped(false);
        if (pos + 1 >= queue.length) {
          void loadDecks();
          void refreshDue();
        }
      } catch (err) {
        toast(errorMessage(err), "error");
      } finally {
        setBusy(false);
      }
    },
    [current, busy, pos, queue.length, loadDecks, refreshDue, toast],
  );

  useEffect(() => {
    if (managing) return;
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      if (target.closest("input, textarea, select")) return;
      if (!current) return;
      if (e.key === " " || e.key === "Enter") {
        e.preventDefault();
        setFlipped((f) => !f);
      } else if (flipped && ["1", "2", "3", "4"].includes(e.key)) {
        void rate(Number(e.key));
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [current, flipped, rate, managing]);

  const selectedDeck = decks.find((d) => d.id === deckId) ?? null;
  const totalDue = decks.reduce((n, d) => n + d.dueCount, 0);
  const totalNew = decks.reduce((n, d) => n + d.newCount, 0);

  return (
    <>
      <header className="topbar">
        <h1>{mode === "learn" ? "Review" : "Decks"}</h1>
        <span className="crumb">{selectedDeck ? selectedDeck.title : "All decks"}</span>
      </header>
      <div className="review view-scroll" style={{ overflow: "hidden" }}>
        <div className="review-stage">
          {managing && selectedDeck ? (
            <DeckManager deck={selectedDeck} onClose={() => setManaging(false)} onChanged={() => { void loadDecks(); void loadQueue(); void refreshDue(); }} />
          ) : decks.length === 0 ? (
            <div className="empty">
              <Layers aria-hidden />
              <h3>No flashcard decks yet</h3>
              <p>Ask for flashcards in a chat, for example “Make flashcards for the key terms in chapter 3”. Decks are saved here and scheduled for review.</p>
            </div>
          ) : current ? (
            <>
              <div className="review-progress" aria-label={`Card ${pos + 1} of ${queue.length}`}>
                <span>
                  {pos + 1} of {queue.length}
                </span>
                <span className="track">
                  <span style={{ width: `${(pos / queue.length) * 100}%` }} />
                </span>
                <span>{current.card.reps === 0 && !current.card.lastReviewedAt ? "New card" : "Review"}</span>
              </div>
              <div className="review-card">
                <div className="review-deck-name">{current.deckTitle}</div>
                <FlashcardFace front={current.card.front} back={current.card.back} hint={current.card.hint} flipped={flipped} onFlip={() => setFlipped(!flipped)} />
              </div>
              {flipped ? (
                <div className="ratings" role="group" aria-label="How well did you remember it?">
                  {RATINGS.map((r, i) => (
                    <button key={r.value} type="button" className={`rating ${r.cls}`} onClick={() => rate(r.value)} disabled={busy}>
                      <strong>{r.label}</strong>
                      <span>{current.previews[i]}</span>
                      <kbd>{r.value}</kbd>
                    </button>
                  ))}
                </div>
              ) : (
                <button type="button" className="btn btn-primary" onClick={() => setFlipped(true)}>
                  Show answer
                </button>
              )}
              <p className="review-hint">
                <kbd>Space</kbd> flips the card. <kbd>1</kbd>–<kbd>4</kbd> rate it once the answer shows.
              </p>
            </>
          ) : (
            <div className="done-card">
              <CheckCircle2 aria-hidden style={{ width: 32, height: 32, color: "var(--success)" }} />
              <h3>{reviewed > 0 ? "Session complete" : "Nothing due right now"}</h3>
              <p>
                {reviewed > 0
                  ? `You reviewed ${reviewed} ${reviewed === 1 ? "card" : "cards"}. Cards you marked Again come back in about ten minutes; the rest are scheduled further out.`
                  : "Every card is scheduled for later. Come back when cards are due, or add a new deck from a chat."}
              </p>
              <button type="button" className="btn" onClick={loadQueue}>
                Check again
              </button>
            </div>
          )}
        </div>

        <aside className="decks-panel" aria-label="Decks">
          <h3>Decks</h3>
          <button type="button" className="deck-row" aria-current={!deckId} onClick={() => studyDeck(null)}>
            <span className="dr-title">All decks</span>
            {totalDue > 0 && <span className="badge">{totalDue}</span>}
            <span className="dr-meta">
              {totalDue} due, {totalNew} new
            </span>
          </button>
          {decks.map((d) => (
            <button key={d.id} type="button" className="deck-row" aria-current={d.id === deckId} onClick={() => studyDeck(d.id)}>
              <span className="dr-title">{d.title}</span>
              {d.dueCount > 0 && <span className="badge">{d.dueCount}</span>}
              <span className="dr-meta">
                {d.cardCount} cards, {d.dueCount} due, {d.newCount} new
              </span>
            </button>
          ))}
          {selectedDeck && (
            <div className="deck-tools">
              <button type="button" className="btn btn-sm" onClick={() => setManaging(!managing)} aria-pressed={managing}>
                <Pencil aria-hidden /> {managing ? "Done editing" : "Edit cards"}
              </button>
              <DeckExport deck={selectedDeck} />
              <button
                type="button"
                className="icon-btn"
                aria-label={`Delete ${selectedDeck.title}`}
                title="Delete deck"
                onClick={async () => {
                  const ok = await ask(`Delete the deck “${selectedDeck.title}” and its review history?`, { title: "Delete deck", kind: "warning", okLabel: "Delete", cancelLabel: "Cancel" });
                  if (!ok) return;
                  await api.deleteDeck(selectedDeck.id).catch((e) => toast(errorMessage(e), "error"));
                  studyDeck(null);
                  void loadDecks();
                  void refreshDue();
                }}
              >
                <Trash2 aria-hidden />
              </button>
            </div>
          )}
        </aside>
      </div>
    </>
  );
}

function DeckExport({ deck }: { deck: Deck }) {
  const toast = useStore((s) => s.toast);
  const run = async () => {
    try {
      const { cards } = await api.getDeck(deck.id);
      if (await exportArtifact("flashcards", deck.title, { deck_title: deck.title, cards }, "tsv")) toast("Exported for Anki", "success");
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  };
  return (
    <button type="button" className="icon-btn" aria-label="Export for Anki" title="Export for Anki" onClick={run}>
      <Download aria-hidden />
    </button>
  );
}

function DeckManager({ deck, onClose, onChanged }: { deck: Deck; onClose: () => void; onChanged: () => void }) {
  const toast = useStore((s) => s.toast);
  const [cards, setCards] = useState<Card[]>([]);
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState({ front: "", back: "", hint: "" });

  const load = useCallback(async () => {
    try {
      setCards((await api.getDeck(deck.id)).cards);
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  }, [deck.id, toast]);

  useEffect(() => {
    void load();
  }, [load]);

  const saveDraft = async () => {
    try {
      if (editing === "new") await api.addCard(deck.id, draft);
      else if (editing) await api.updateCard(editing, draft);
      setEditing(null);
      setDraft({ front: "", back: "", hint: "" });
      await load();
      onChanged();
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  };

  return (
    <div style={{ width: "min(640px, 100%)", alignSelf: "stretch", margin: "0 auto" }}>
      <div className="view-header" style={{ marginBottom: 8 }}>
        <div>
          <h2>{deck.title}</h2>
          <p>{cards.length} cards. Edits keep each card's review schedule.</p>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button type="button" className="btn" onClick={() => { setEditing("new"); setDraft({ front: "", back: "", hint: "" }); }}>
            <Plus aria-hidden /> Add card
          </button>
          <button type="button" className="icon-btn" aria-label="Close editor" onClick={onClose}>
            <X aria-hidden />
          </button>
        </div>
      </div>
      {editing && (
        <div className="card-editor">
          <label className="field">
            <span>Front</span>
            <textarea className="textarea" style={{ minHeight: 60 }} value={draft.front} onChange={(e) => setDraft({ ...draft, front: e.target.value })} />
          </label>
          <label className="field">
            <span>Back</span>
            <textarea className="textarea" style={{ minHeight: 60 }} value={draft.back} onChange={(e) => setDraft({ ...draft, back: e.target.value })} />
          </label>
          <label className="field">
            <span>Hint (optional)</span>
            <input className="input" value={draft.hint} onChange={(e) => setDraft({ ...draft, hint: e.target.value })} />
          </label>
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <button type="button" className="btn" onClick={() => setEditing(null)}>
              Cancel
            </button>
            <button type="button" className="btn btn-primary" onClick={saveDraft} disabled={!draft.front.trim() || !draft.back.trim()}>
              {editing === "new" ? "Add card" : "Save card"}
            </button>
          </div>
        </div>
      )}
      <div className="card-list">
        {cards.map((c) => (
          <div key={c.id} className="card-list-item">
            <strong>{c.front}</strong>
            <span style={{ display: "flex", gap: 2 }}>
              <button type="button" className="icon-btn" aria-label="Edit card" onClick={() => { setEditing(c.id); setDraft({ front: c.front, back: c.back, hint: c.hint }); }}>
                <Pencil aria-hidden />
              </button>
              <button
                type="button"
                className="icon-btn"
                aria-label="Delete card"
                onClick={async () => {
                  await api.deleteCard(c.id).catch((e) => toast(errorMessage(e), "error"));
                  await load();
                  onChanged();
                }}
              >
                <Trash2 aria-hidden />
              </button>
            </span>
            <span>{c.back}</span>
            <span className="clm">{c.reps > 0 ? `Next review ${new Date(c.dueAt).toLocaleDateString()}` : "New"}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

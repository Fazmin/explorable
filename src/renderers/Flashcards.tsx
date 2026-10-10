import { useEffect, useState } from "react";
import { ChevronLeft, ChevronRight, Layers, Shuffle } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import { useStore } from "../lib/store";
import { Figure, type FigureContext } from "../components/Figure";
import { Markdown } from "../components/Markdown";

export interface FlashcardsInput {
  deck_title?: string;
  description?: string;
  cards?: { front?: string; back?: string; hint?: string }[];
}

export function FlashcardFace({ front, back, hint, flipped, onFlip }: { front: string; back: string; hint?: string; flipped: boolean; onFlip: () => void }) {
  return (
    <button type="button" className="flashcard" data-flipped={flipped} onClick={onFlip} aria-label={flipped ? "Show the front" : "Show the answer"}>
      <div className="flashcard-inner">
        <div className="flashcard-face front" aria-hidden={flipped}>
          <span className="face-label">Front</span>
          <div className="face-text">
            <Markdown text={front} className="prose inline" />
          </div>
          {hint && <span className="face-hint">Hint: {hint}</span>}
        </div>
        <div className="flashcard-face back" aria-hidden={!flipped}>
          <span className="face-label">Back</span>
          <div className="face-text">
            <Markdown text={back} className="prose inline" />
          </div>
        </div>
      </div>
    </button>
  );
}

/** The deck as rendered in a chat answer or the Library. */
export function Flashcards({ input, streaming, ctx, deckId }: { input: FlashcardsInput; streaming: boolean; ctx: FigureContext; deckId?: string | null }) {
  const cards = (input.cards ?? []).filter((c) => c && (c.front || c.back));
  const [order, setOrder] = useState<number[]>([]);
  const [pos, setPos] = useState(0);
  const [flipped, setFlipped] = useState(false);
  const [addedDeck, setAddedDeck] = useState<string | null>(null);
  const { studyDeck, toast } = useStore.getState();
  const title = input.deck_title?.trim() || "Flashcards";

  useEffect(() => {
    setOrder(cards.map((_, i) => i));
    // Reset the order only when the number of cards changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cards.length]);

  const index = order[pos] ?? pos;
  const card = cards[index];
  const go = (delta: number) => {
    setFlipped(false);
    setPos((p) => (p + delta + cards.length) % cards.length);
  };
  const shuffle = () => {
    const next = cards.map((_, i) => i);
    for (let i = next.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [next[i], next[j]] = [next[j], next[i]];
    }
    setOrder(next);
    setPos(0);
    setFlipped(false);
  };

  const addToReview = async () => {
    try {
      const deck = await api.createDeck(
        ctx.courseId,
        title,
        input.description ?? "",
        cards.map((c) => ({ front: c.front ?? "", back: c.back ?? "", hint: c.hint ?? "" })),
      );
      setAddedDeck(deck.id);
      void useStore.getState().refreshDue();
      toast(`Added “${deck.title}” to Review`, "success");
    } catch (err) {
      toast(errorMessage(err), "error");
    }
  };

  const reviewDeck = deckId ?? addedDeck;
  const footer = !streaming ? (
    <div className="figure-foot" style={{ display: "flex", alignItems: "center", gap: 10 }}>
      <span style={{ flex: 1 }}>
        {reviewDeck ? "Saved as a deck and scheduled for spaced repetition." : "Not in your review queue yet."}
      </span>
      {reviewDeck ? (
        <button type="button" className="btn btn-sm" onClick={() => studyDeck(reviewDeck)}>
          <Layers aria-hidden /> Study in Review
        </button>
      ) : (
        <button type="button" className="btn btn-sm" onClick={addToReview}>
          <Layers aria-hidden /> Add to Review
        </button>
      )}
    </div>
  ) : null;

  const status = streaming ? (
    <span className="figure-status" role="status">
      <span className="spinner" /> {cards.length} {cards.length === 1 ? "card" : "cards"}
    </span>
  ) : null;

  return (
    <Figure kind="flashcards" title={title} payload={input as Record<string, unknown>} ctx={ctx} streaming={streaming} status={status} footer={footer}>
      {card ? (
        <div
          className="deck"
          onKeyDown={(e) => {
            if (e.key === "ArrowRight") go(1);
            if (e.key === "ArrowLeft") go(-1);
          }}
        >
          <FlashcardFace front={card.front ?? ""} back={card.back ?? ""} hint={card.hint} flipped={flipped} onFlip={() => setFlipped(!flipped)} />
          <div className="deck-nav">
            <button type="button" className="icon-btn" aria-label="Previous card" onClick={() => go(-1)} disabled={cards.length < 2}>
              <ChevronLeft aria-hidden />
            </button>
            <span aria-live="polite">
              {pos + 1} / {cards.length}
            </span>
            <button type="button" className="icon-btn" aria-label="Next card" onClick={() => go(1)} disabled={cards.length < 2}>
              <ChevronRight aria-hidden />
            </button>
            <button type="button" className="icon-btn" aria-label="Shuffle" title="Shuffle" onClick={shuffle} disabled={cards.length < 2 || streaming}>
              <Shuffle aria-hidden />
            </button>
          </div>
        </div>
      ) : (
        <div className="skeleton-lines" aria-hidden>
          <span />
          <span />
          <span />
        </div>
      )}
    </Figure>
  );
}

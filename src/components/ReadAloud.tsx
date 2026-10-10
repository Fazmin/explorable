import { useEffect, useRef, useState } from "react";
import { Square, Volume2 } from "lucide-react";
import { useStore } from "../lib/store";
import { markdownToSpeech } from "../lib/speech";

/** Reads an answer aloud with the system's speech voices. */
export function ReadAloud({ text }: { text: string }) {
  const enabled = useStore((s) => s.settings?.readAloud ?? true);
  const [speaking, setSpeaking] = useState(false);
  const speakingRef = useRef(false);
  speakingRef.current = speaking;

  useEffect(
    () => () => {
      if (speakingRef.current) window.speechSynthesis?.cancel();
    },
    [],
  );

  if (!enabled || typeof window.speechSynthesis === "undefined" || !text.trim()) return null;

  const toggle = () => {
    const synth = window.speechSynthesis;
    if (speaking) {
      synth.cancel();
      setSpeaking(false);
      return;
    }
    synth.cancel();
    const utterance = new SpeechSynthesisUtterance(markdownToSpeech(text));
    utterance.lang = document.documentElement.lang || "en";
    utterance.onend = () => setSpeaking(false);
    utterance.onerror = () => setSpeaking(false);
    setSpeaking(true);
    synth.speak(utterance);
  };

  return (
    <button type="button" className="btn btn-ghost btn-sm read-aloud" aria-pressed={speaking} onClick={toggle}>
      {speaking ? <Square aria-hidden /> : <Volume2 aria-hidden />}
      {speaking ? "Stop reading" : "Read aloud"}
    </button>
  );
}

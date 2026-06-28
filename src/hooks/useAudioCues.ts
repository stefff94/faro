import { useEffect, useRef } from "react";
import type { SessionState } from "../types";
import type { Settings } from "../settings";
import { isMuted } from "../settings";
import { detectCues } from "../attention";

const SOUND: Record<string, string> = {
  "needs-input": "/sounds/needs-input.wav",
  "done": "/sounds/done.wav",
};

export function useAudioCues(sessions: SessionState[], settings: Settings) {
  const prev = useRef<SessionState[]>([]);
  useEffect(() => {
    if (settings.soundEnabled) {
      const audible = sessions.filter((s) => !isMuted(settings, s.id));
      const prevAudible = prev.current.filter((s) => !isMuted(settings, s.id));
      for (const cue of detectCues(prevAudible, audible)) {
        new Audio(SOUND[cue]).play().catch(() => {});
      }
    }
    prev.current = sessions;
  }, [sessions, settings]);
}

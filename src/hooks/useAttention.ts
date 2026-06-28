import { useEffect, useRef, useState } from "react";
import type { SessionState, AttentionPhase } from "../types";
import { aggregate } from "../snapshot";
import { attentionPhase } from "../attention";

export function useAttention(sessions: SessionState[], decayMs: number): AttentionPhase {
  const blockedSince = useRef<number | null>(null);
  const [, tick] = useState(0);
  const agg = aggregate(sessions);

  if (agg.input > 0 && blockedSince.current === null) blockedSince.current = Date.now();
  if (agg.input === 0) blockedSince.current = null;

  useEffect(() => {
    const t = setInterval(() => tick((n) => n + 1), 1000);
    return () => clearInterval(t);
  }, []);

  return attentionPhase(agg, blockedSince.current, Date.now(), decayMs);
}

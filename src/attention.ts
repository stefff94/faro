import type { Aggregate, SessionState, AttentionPhase, Cue } from "./types";

export function attentionPhase(
  agg: Aggregate, blockedSince: number | null, now: number, decayMs: number,
): AttentionPhase {
  if (agg.input > 0 && blockedSince !== null) {
    return now - blockedSince < decayMs ? "needs-input-evident" : "needs-input-compact";
  }
  if (agg.working > 0) return "working";
  return "idle";
}

const isBlocked = (s: SessionState) => s.status === "blocked" || s.status === "error";

export function detectCues(prev: SessionState[], next: SessionState[]): Cue[] {
  const before = new Map(prev.map((s) => [s.id, s.status]));
  const cues: Cue[] = [];
  for (const s of next) {
    const was = before.get(s.id);
    if (was === s.status) continue;
    if (isBlocked(s) && !(was === "blocked" || was === "error")) cues.push("needs-input");
    else if (s.status === "done" && was !== "done") cues.push("done");
  }
  return cues;
}

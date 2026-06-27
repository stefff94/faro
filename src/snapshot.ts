import type { SessionState } from "./types";

export function sortSessions(list: SessionState[]): SessionState[] {
  return [...list].sort((a, b) => a.id.localeCompare(b.id));
}

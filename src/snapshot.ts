import type { SessionState, Status, Aggregate } from "./types";

const PRIORITY: Record<Status, number> = {
  blocked: 0, error: 0, working: 1, done: 2, idle: 3, stale: 3,
};

export function sortSessions(list: SessionState[]): SessionState[] {
  return [...list].sort(
    (a, b) =>
      PRIORITY[a.status] - PRIORITY[b.status] ||
      b.lastUpdate - a.lastUpdate ||
      a.id.localeCompare(b.id),
  );
}

export function formatDuration(ms: number): string {
  const s = Math.floor(ms / 1000);
  if (s < 5) return "ora";
  if (s < 60) return `${s}s`;
  return `${Math.floor(s / 60)}m ${s % 60}s`;
}

/** Last path segment of cwd, regardless of separator style (Unix / Windows). */
export function sessionName(s: SessionState): string {
  const segs = s.cwd.split(/[/\\]/).filter(Boolean);
  return segs[segs.length - 1] || s.label;
}

export function aggregate(list: SessionState[]): Aggregate {
  const a: Aggregate = { input: 0, working: 0, done: 0, idle: 0, total: list.length };
  for (const s of list) {
    if (s.status === "blocked" || s.status === "error") a.input++;
    else if (s.status === "working") a.working++;
    else if (s.status === "done") a.done++;
    else a.idle++;
  }
  return a;
}

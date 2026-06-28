import { describe, it, expect } from "vitest";
import { sortSessions, formatDuration, aggregate } from "./snapshot";
import type { SessionState, Status } from "./types";

const mk = (id: string, status: Status, lastUpdate = 0): SessionState => ({
  id, source: "claude-code", sessionId: id, label: "p", cwd: "/x/p",
  status, lastEventName: "x", lastUpdate, statusSince: lastUpdate,
});

describe("sortSessions", () => {
  it("orders blocked before working before done", () => {
    const out = sortSessions([mk("a", "done", 9), mk("b", "working", 5), mk("c", "blocked", 1)]);
    expect(out.map((s) => s.status)).toEqual(["blocked", "working", "done"]);
  });
  it("within a status, newer lastUpdate first", () => {
    const out = sortSessions([mk("a", "working", 1), mk("b", "working", 9)]);
    expect(out.map((s) => s.id)).toEqual(["b", "a"]);
  });
});

describe("formatDuration", () => {
  it("formats buckets", () => {
    expect(formatDuration(2_000)).toBe("ora");
    expect(formatDuration(22_000)).toBe("22s");
    expect(formatDuration(72_000)).toBe("1m 12s");
  });
});

describe("aggregate", () => {
  it("counts by bucket", () => {
    const a = aggregate([mk("a", "blocked"), mk("b", "working"), mk("c", "done"), mk("d", "idle")]);
    expect(a).toEqual({ input: 1, working: 1, done: 1, idle: 1, total: 4 });
  });
});

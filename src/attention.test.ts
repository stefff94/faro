import { describe, it, expect } from "vitest";
import { attentionPhase, detectCues } from "./attention";
import type { SessionState, Status, Aggregate } from "./types";

const agg = (input: number, working: number, done = 0): Aggregate => ({
  input, working, done, idle: 0, total: input + working + done,
});
const mk = (id: string, status: Status): SessionState => ({
  id, source: "c", sessionId: id, label: "p", cwd: "/x", status,
  lastEventName: "x", lastUpdate: 0, statusSince: 0,
});

describe("attentionPhase", () => {
  it("idle when nothing active", () => {
    expect(attentionPhase(agg(0, 0), null, 0, 8000)).toBe("idle");
  });
  it("working when something runs and none blocked", () => {
    expect(attentionPhase(agg(0, 2), null, 0, 8000)).toBe("working");
  });
  it("needs-input is evident before decay, compact after", () => {
    expect(attentionPhase(agg(1, 0), 1000, 5000, 8000)).toBe("needs-input-evident");
    expect(attentionPhase(agg(1, 0), 1000, 10000, 8000)).toBe("needs-input-compact");
  });
});

describe("detectCues", () => {
  it("fires needs-input when a session newly blocks", () => {
    expect(detectCues([mk("a", "working")], [mk("a", "blocked")])).toEqual(["needs-input"]);
  });
  it("fires done when a session newly completes", () => {
    expect(detectCues([mk("a", "working")], [mk("a", "done")])).toEqual(["done"]);
  });
  it("no cue when status unchanged", () => {
    expect(detectCues([mk("a", "blocked")], [mk("a", "blocked")])).toEqual([]);
  });
});

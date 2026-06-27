import { describe, it, expect } from "vitest";
import { sortSessions } from "./snapshot";
import type { SessionState } from "./types";

const mk = (id: string): SessionState => ({
  id, source: "claude-code", sessionId: id.split(":")[1] ?? id,
  label: "p", cwd: "/x/p", status: "working",
  lastEventName: "PreToolUse", lastUpdate: 0,
});

describe("sortSessions", () => {
  it("orders by id for stable rows", () => {
    const out = sortSessions([mk("claude-code:b"), mk("claude-code:a")]);
    expect(out.map((s) => s.id)).toEqual(["claude-code:a", "claude-code:b"]);
  });
});

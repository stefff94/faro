import { describe, it, expect, beforeEach } from "vitest";
import { loadSettings, saveSettings, isMuted, DEFAULT_SETTINGS } from "./settings";

beforeEach(() => {
  (globalThis as any).localStorage = {
    _v: {} as Record<string, string>,
    getItem(k: string) { return this._v[k] ?? null; },
    setItem(k: string, v: string) { this._v[k] = v; },
  };
});

describe("settings", () => {
  it("returns defaults when nothing stored", () => {
    expect(loadSettings()).toEqual(DEFAULT_SETTINGS);
  });
  it("round-trips saved settings", () => {
    saveSettings({ decayMs: 5000, soundEnabled: false, mutedSessionIds: ["x"] });
    expect(loadSettings().decayMs).toBe(5000);
    expect(isMuted(loadSettings(), "x")).toBe(true);
  });
  it("tolerates malformed storage", () => {
    localStorage.setItem("faro.settings", "{not json");
    expect(loadSettings()).toEqual(DEFAULT_SETTINGS);
  });
});

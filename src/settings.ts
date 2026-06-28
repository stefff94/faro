export interface Settings {
  decayMs: number;
  soundEnabled: boolean;
  mutedSessionIds: string[];
}

export const DEFAULT_SETTINGS: Settings = {
  decayMs: 3000,
  soundEnabled: true,
  mutedSessionIds: [],
};

const STORAGE_KEY = "faro.settings";

export function loadSettings(): Settings {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (!stored) return DEFAULT_SETTINGS;
    return JSON.parse(stored) as Settings;
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export function saveSettings(s: Settings): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
}

export function isMuted(s: Settings, sessionId: string): boolean {
  return s.mutedSessionIds.includes(sessionId);
}

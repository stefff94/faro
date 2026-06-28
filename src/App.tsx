import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import type { SessionState } from "./types";
import { sortSessions, aggregate } from "./snapshot";
import { loadSettings, saveSettings, isMuted, type Settings } from "./settings";
import { useAttention } from "./hooks/useAttention";
import { useAudioCues } from "./hooks/useAudioCues";
import { CollapsedPill } from "./components/CollapsedPill";
import { DrawerPanel } from "./components/DrawerPanel";
import { SessionCard } from "./components/SessionCard";
import { SessionDetail } from "./components/SessionDetail";

export default function App() {
  const [sessions, setSessions] = useState<SessionState[]>([]);
  const [settings, setSettings] = useState<Settings>(() => loadSettings());
  const [hovering, setHovering] = useState(false);
  const [pinned, setPinned] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [pinnedTop, setPinnedTop] = useState<string[]>([]);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    const un = listen<SessionState[]>("sessions-updated", (e) => setSessions(e.payload));
    return () => { un.then((f) => f()); };
  }, []);
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);

  // Poll cursor position every 80ms; toggle window click-through accordingly.
  // The window starts with ignore_cursor_events=true (click-through by default).
  // When the cursor enters the window bounds, Rust disables click-through so the
  // UI becomes interactive; when the cursor leaves, click-through is re-enabled.
  useEffect(() => {
    let active = true;
    let lastIn = false;
    async function poll() {
      if (!active) return;
      try {
        const inWin = await invoke<boolean>("cursor_in_window");
        if (inWin !== lastIn) {
          lastIn = inWin;
          await invoke("set_cursor_passthrough", { passthrough: !inWin });
        }
      } catch { /* ignore — not in Tauri context (e.g. browser dev) */ }
      if (active) setTimeout(poll, 80);
    }
    poll();
    return () => { active = false; };
  }, []);

  const ordered = useMemo(() => {
    const sorted = sortSessions(sessions);
    return [...sorted].sort(
      (a, b) => Number(pinnedTop.includes(b.id)) - Number(pinnedTop.includes(a.id)),
    );
  }, [sessions, pinnedTop]);

  const agg = aggregate(sessions);
  const phase = useAttention(sessions, settings.decayMs);
  useAudioCues(sessions, settings);

  const update = (s: Settings) => { setSettings(s); saveSettings(s); };
  const open = hovering || pinned;
  const selected = ordered.find((s) => s.id === selectedId) ?? null;
  const topSession = ordered.find((s) => s.status === "blocked" || s.status === "error") ?? null;

  return (
    <div className="faro-root">
      <DrawerPanel
        open={open}
        onEnter={() => setHovering(true)}
        onLeave={() => setHovering(false)}
        onToggle={() => setPinned((p) => !p)}
        pill={<CollapsedPill agg={agg} phase={phase} topSession={topSession} />}
        panel={
          selected ? (
            <SessionDetail
              session={selected} now={now} muted={isMuted(settings, selected.id)}
              onClose={() => setSelectedId(null)}
              onMute={() => update({
                ...settings,
                mutedSessionIds: isMuted(settings, selected.id)
                  ? settings.mutedSessionIds.filter((x) => x !== selected.id)
                  : [...settings.mutedSessionIds, selected.id],
              })}
              onPinTop={() => setPinnedTop((p) =>
                p.includes(selected.id) ? p.filter((x) => x !== selected.id) : [...p, selected.id])}
              onArchive={() => {
                setSessions((list) => list.filter((s) => s.id !== selected.id));
                setSelectedId(null);
              }}
            />
          ) : (
            <>
              <div className="phdr"><span>Faro</span><span>{agg.total} sessioni</span></div>
              {ordered.length === 0
                ? <div className="empty">nessuna sessione</div>
                : ordered.map((s) => (
                    <SessionCard key={s.id} session={s} now={now} onClick={() => setSelectedId(s.id)} />
                  ))}
            </>
          )
        }
      />
    </div>
  );
}

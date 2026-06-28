import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import type { SessionState } from "./types";
import { sortSessions } from "./snapshot";
import { SessionCard } from "./components/SessionCard";

export default function App() {
  const [sessions, setSessions] = useState<SessionState[]>([]);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    const unlisten = listen<SessionState[]>("sessions-updated", (event) => {
      setSessions(sortSessions(event.payload));
      setNow(Date.now());
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return (
    <div className="faro-root">
      {sessions.length === 0 ? (
        <div className="empty-pill">idle</div>
      ) : (
        sessions.map((s) => <SessionCard key={s.id} session={s} now={now} />)
      )}
    </div>
  );
}

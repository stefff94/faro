import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import type { SessionState } from "./types";
import { sortSessions } from "./snapshot";
import { SessionRow } from "./components/SessionRow";

export default function App() {
  const [sessions, setSessions] = useState<SessionState[]>([]);

  useEffect(() => {
    const unlisten = listen<SessionState[]>("sessions-updated", (event) => {
      setSessions(sortSessions(event.payload));
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return (
    <div className="faro-root">
      {sessions.map((session) => (
        <SessionRow key={session.id} session={session} />
      ))}
    </div>
  );
}

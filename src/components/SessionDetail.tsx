import type { SessionState } from "../types";
import { formatDuration, sessionName } from "../snapshot";

export function SessionDetail({
  session, now, muted, onMute, onPinTop, onArchive, onClose,
}: {
  session: SessionState; now: number; muted: boolean;
  onMute: () => void; onPinTop: () => void; onArchive: () => void; onClose: () => void;
}) {
  return (
    <div className="detail">
      <div className="detail-head">
        <span className="proj">{sessionName(session)}{session.branch && <span className="branch"> {session.branch}</span>}</span>
        <button className="x" onClick={onClose}>✕</button>
      </div>
      {session.taskSummary && <div className="task">{session.taskSummary}</div>}
      <dl className="kv">
        <dt>stato</dt><dd>{session.status} · da {formatDuration(now - session.statusSince)}</dd>
        <dt>evento</dt><dd>{session.lastEventName}</dd>
        <dt>path</dt><dd className="mono">{session.cwd}</dd>
      </dl>
      <div className="actions">
        <button onClick={onMute}>{muted ? "🔔 riattiva" : "🔕 silenzia"}</button>
        <button onClick={onPinTop}>📌 in cima</button>
        {(session.status === "done" || session.status === "stale") && (
          <button onClick={onArchive}>🗙 archivia</button>
        )}
      </div>
    </div>
  );
}

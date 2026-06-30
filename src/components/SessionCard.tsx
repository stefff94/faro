import type { SessionState } from "../types";
import { formatDuration, sessionName } from "../snapshot";
import { StatusChip } from "./StatusChip";

const cardClass: Record<string, string> = {
  working: "card cW", blocked: "card cB", error: "card cB",
  done: "card cD", idle: "card cI", stale: "card cI",
};

export function SessionCard(
  { session, now, onClick, compact }: { session: SessionState; now: number; onClick?: () => void; compact?: boolean },
) {
  return (
    <div className={`${cardClass[session.status]}${compact ? " compact" : ""}`} onClick={onClick} title={session.cwd}>
      <div className="card-r1">
        <div className="proj" title={sessionName(session)}>{sessionName(session)}</div>
        <div className="meta">{formatDuration(now - session.statusSince)}</div>
      </div>
      <div className="card-r2">
        <StatusChip status={session.status} />
        {session.branch && <div className="branch">{session.branch}</div>}
      </div>
      {session.taskSummary && <div className="task">{session.taskSummary}</div>}
    </div>
  );
}

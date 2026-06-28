import type { SessionState } from "../types";
import { formatDuration } from "../snapshot";
import { StatusChip } from "./StatusChip";

const cardClass: Record<string, string> = {
  working: "card cW", blocked: "card cB", error: "card cB",
  done: "card cD", idle: "card cI", stale: "card cI",
};

export function SessionCard(
  { session, now, onClick }: { session: SessionState; now: number; onClick?: () => void },
) {
  return (
    <div className={cardClass[session.status]} onClick={onClick} title={session.cwd}>
      <StatusChip status={session.status} />
      <div className="body">
        <div className="proj">
          {session.label}
          {session.branch && <span className="branch"> {session.branch}</span>}
        </div>
        {session.taskSummary && <div className="task">{session.taskSummary}</div>}
      </div>
      <div className="meta">{formatDuration(now - session.statusSince)}</div>
    </div>
  );
}

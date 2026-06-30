import type { Aggregate, AttentionPhase, SessionState } from "../types";
import { sessionName } from "../snapshot";

export function CollapsedPill(
  { agg, phase, topSession }: { agg: Aggregate; phase: AttentionPhase; topSession: SessionState | null },
) {
  if (phase === "idle") {
    return <div className="pill nub"><span className="cnt g">✓ {agg.done || agg.total}</span></div>;
  }
  if (phase === "needs-input-evident" && topSession) {
    return (
      <div className="pill bhi">
        <span className="chip chB">◆ serve input</span>
        <div className="proj">{sessionName(topSession)}</div>
        {topSession.taskSummary && <div className="task">{topSession.taskSummary}</div>}
        <div className="rest">+{agg.working} lavora · +{agg.done} fatto ▸</div>
      </div>
    );
  }
  const blocked = phase === "needs-input-compact";
  return (
    <div className={"pill " + (blocked ? "blo" : "")}>
      {agg.input > 0 && <span className="cnt r">◆ {agg.input}</span>}
      {agg.working > 0 && <span className={"cnt y" + (blocked ? "" : " breathe")}>● {agg.working}</span>}
      {agg.done > 0 && <span className="cnt g">✓ {agg.done}</span>}
    </div>
  );
}

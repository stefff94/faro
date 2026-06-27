import type { SessionState } from "../types";
import { TrafficLight } from "./TrafficLight";

export function SessionRow({ session }: { session: SessionState }) {
  return (
    <div className="session-row" title={session.cwd}>
      <TrafficLight status={session.status} />
      <span className="label">{session.label}</span>
    </div>
  );
}

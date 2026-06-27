export type Status = "idle" | "working" | "blocked" | "done" | "stale" | "error";

export interface SessionState {
  id: string;
  source: string;
  sessionId: string;
  label: string;
  cwd: string;
  status: Status;
  lastEventName: string;
  lastUpdate: number;
  transcriptPath?: string;
}

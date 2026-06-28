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
  statusSince: number;
  branch?: string;
  taskSummary?: string;
  transcriptPath?: string;
}

export type Aggregate = {
  input: number; working: number; done: number; idle: number; total: number;
};

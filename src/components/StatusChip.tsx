import type { Status } from "../types";

const CHIP: Record<Status, { cls: string; label: string }> = {
  working: { cls: "chip chW", label: "● working" },
  blocked: { cls: "chip chB", label: "◆ input" },
  error:   { cls: "chip chB", label: "◆ error" },
  done:    { cls: "chip chD", label: "✓ done" },
  idle:    { cls: "chip chI", label: "· idle" },
  stale:   { cls: "chip chI", label: "· idle" },
};

export function StatusChip({ status }: { status: Status }) {
  const c = CHIP[status];
  return <span className={c.cls}>{c.label}</span>;
}

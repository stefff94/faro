import type { Status } from "../types";

const COLOR: Record<Status, string> = {
  idle: "#6b7280",
  working: "#f5c518",
  blocked: "#ef4444",
  done: "#22c55e",
  stale: "#6b7280",
  error: "#ef4444",
};

const LIT: Record<Status, "red" | "yellow" | "green" | "none"> = {
  idle: "none", working: "yellow", blocked: "red",
  done: "green", stale: "none", error: "red",
};

export function TrafficLight({ status }: { status: Status }) {
  const lit = LIT[status];
  const dot = (which: "red" | "yellow" | "green", color: string) => (
    <span
      className={"dot" + (status === "error" && which === "red" ? " blink" : "")}
      style={{
        background: lit === which ? color : "#2a2a2a",
        opacity: lit === which ? 1 : 0.35,
      }}
    />
  );
  return (
    <span className="traffic-light">
      {dot("red", COLOR.blocked)}
      {dot("yellow", COLOR.working)}
      {dot("green", COLOR.done)}
    </span>
  );
}

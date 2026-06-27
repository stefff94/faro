type Status = "working" | "blocked" | "idle";

interface TrafficLightProps {
  status: Status;
  label: string;
}

const STATUS_COLORS: Record<Status, string> = {
  working: "#22c55e",
  blocked: "#ef4444",
  idle: "#6b7280",
};

const STATUS_LABELS: Record<Status, string> = {
  working: "working",
  blocked: "blocked",
  idle: "idle",
};

export function TrafficLight({ status, label }: TrafficLightProps) {
  const color = STATUS_COLORS[status];

  return (
    <div className="traffic-light-row">
      <div
        className="traffic-light-dot"
        style={{ backgroundColor: color }}
        aria-label={STATUS_LABELS[status]}
      />
      <span className="traffic-light-label">{label}</span>
    </div>
  );
}

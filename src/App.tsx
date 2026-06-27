import "./App.css";
import { TrafficLight } from "./components/TrafficLight";

export default function App() {
  return (
    <div className="faro-root">
      <div className="session-row">
        <TrafficLight status="working" />
        <span className="label">dummy-session</span>
      </div>
    </div>
  );
}

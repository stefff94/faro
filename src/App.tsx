import "./App.css";
import { TrafficLight } from "./TrafficLight";

function App() {
  return (
    <div className="faro-widget">
      <TrafficLight status="working" label="my-project" />
    </div>
  );
}

export default App;

import { createRoot } from "react-dom/client";
import App from "./App";
import WebGate from "./WebGate";
import { host } from "@host";

createRoot(document.getElementById("root")!).render(host.kind === "web" ? <WebGate host={host} /> : <App host={host} />);

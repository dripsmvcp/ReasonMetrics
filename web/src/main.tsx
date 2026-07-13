import "./style.css";
import { createRoot } from "react-dom/client";
import App from "./App";
import { initWasm } from "./lib/wasm";

async function main() {
  const app = document.querySelector<HTMLDivElement>("#app")!;
  app.textContent = "Loading wasm…";

  await initWasm();
  app.textContent = "";

  createRoot(app).render(<App />);
}

void main();

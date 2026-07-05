import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./playwright-gate";
import "./index.css";
import App from "./App";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

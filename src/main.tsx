import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./playwright-gate";
import "./styles/fonts.css";
import "./styles/tokens.css";
import "./index.css";
import App from "./App";
import { initTheme } from "./lib/theme";

initTheme();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

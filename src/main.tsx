import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./design-system/styles.css";

// R6: subscribe, don't sample once — macOS commonly switches appearance while
// the app runs (auto light/dark), and the tray icon already re-reads dark mode
// on every repaint. Dark is the :root default; "light" is the only override
// the tokens define (see design-system/tokens/colors.css).
const appearanceQuery = window.matchMedia("(prefers-color-scheme: light)");
const applyAppearance = () => {
  if (appearanceQuery.matches) {
    document.documentElement.dataset.appearance = "light";
  } else {
    delete document.documentElement.dataset.appearance;
  }
};
applyAppearance();
appearanceQuery.addEventListener("change", applyAppearance);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

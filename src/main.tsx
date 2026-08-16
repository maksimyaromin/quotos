import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./design-system/styles.css";

// macOS can switch between light and dark appearance while the app runs, so
// this subscribes to the change event instead of reading the media query
// once. Dark is the :root default, and light is the only override the design
// tokens define in design-system/tokens/colors.css.
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

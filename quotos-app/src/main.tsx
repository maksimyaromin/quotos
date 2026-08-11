import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./design-system/styles.css";

const prefersLight = window.matchMedia("(prefers-color-scheme: light)").matches;
if (prefersLight) {
  document.documentElement.dataset.appearance = "light";
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

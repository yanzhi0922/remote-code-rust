import React from "react";
import ReactDOM from "react-dom/client";
import "./index.css";
import App from "./App";
import { hasTauriRuntime } from "./lib/runtime";

if ('serviceWorker' in navigator && !hasTauriRuntime()) {
  window.addEventListener('load', () => {
    void navigator.serviceWorker.register('/sw.js').catch(() => {
      // Keep the browser shell functional even when the PWA cache is unavailable.
    });
  });
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

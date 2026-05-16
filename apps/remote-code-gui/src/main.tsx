import React from "react";
import ReactDOM from "react-dom/client";
import "katex/dist/katex.min.css";
import "highlight.js/styles/github.css";
import "./index.css";
import App from "./App";
import { AppErrorBoundary } from "./components/layout/AppErrorBoundary";
import { hasTauriRuntime } from "./lib/runtime";

const SERVICE_WORKER_VERSION = __APP_BUILD_ID__;
const SERVICE_WORKER_URL = `/sw.js?v=${SERVICE_WORKER_VERSION}`;

if ('serviceWorker' in navigator && !hasTauriRuntime()) {
  window.addEventListener('load', () => {
    let reloadedForFreshController = false;
    navigator.serviceWorker.addEventListener('controllerchange', () => {
      if (reloadedForFreshController) {
        return;
      }
      reloadedForFreshController = true;
      window.location.reload();
    });

    void navigator.serviceWorker
      .register(SERVICE_WORKER_URL, { updateViaCache: 'none' })
      .then((registration) => registration.update())
      .catch(() => {
        // Keep the browser shell functional even when the PWA cache is unavailable.
      });
  });
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppErrorBoundary>
      <App />
    </AppErrorBoundary>
  </React.StrictMode>,
);

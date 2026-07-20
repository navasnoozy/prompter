import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./shared/ErrorBoundary";
import { bootstrapStores } from "./shared/bootstrap";

const root = ReactDOM.createRoot(
  document.getElementById("root") as HTMLElement,
);

function renderApplication() {
  root.render(
    <React.StrictMode>
      <ErrorBoundary>
        <App />
      </ErrorBoundary>
    </React.StrictMode>,
  );
}

async function start(): Promise<void> {
  root.render(
    <div aria-live="polite" className="boot-screen" role="status">
      <span aria-hidden="true" className="boot-mark">
        ✦
      </span>
      <span>Opening Prompter…</span>
    </div>,
  );

  await bootstrapStores();
  renderApplication();
}

void start().catch(() => {
  root.render(
    <div className="error-boundary" role="alert">
      <h1>Prompter could not start</h1>
      <p>Quit and reopen the app. If this continues, reinstall Prompter.</p>
    </div>,
  );
});

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./shared/ErrorBoundary";
import { bootstrapStores } from "./shared/bootstrap";

// The window starts hidden until the lifecycle presents it, so loading the
// durable settings before the first render costs nothing visible and
// guarantees the UI never flashes default state.
await bootstrapStores();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);

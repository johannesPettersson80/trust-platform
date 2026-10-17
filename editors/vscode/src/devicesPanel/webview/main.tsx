// The webview entry point. The extension host owns the transport: it posts one view per poll and
// receives the `op` of whatever the user chose (design §1.4.1 and §4.5 D1). Until D7 wires the
// panel command, nothing posts, so the shell stays empty. The message envelope is the host's
// wiring, not part of the view model.

import React from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import type { DevicesView } from "./types";

/** The slice of VS Code's webview API that the shell uses. */
interface VsCodeApi {
  postMessage(message: unknown): void;
}

declare function acquireVsCodeApi(): VsCodeApi;

const vscode: VsCodeApi =
  typeof acquireVsCodeApi === "function" ? acquireVsCodeApi() : { postMessage() {} };

function post(message: unknown): void {
  vscode.postMessage(message);
}

const container = document.getElementById("root");
if (!container) {
  throw new Error("Devices & Connections: the root element is missing.");
}

/** Waits for the first view from the host, then draws the shell. */
function DevicesShell() {
  const [view, setView] = React.useState<DevicesView | undefined>(undefined);
  React.useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      const message = event.data as { view?: DevicesView } | undefined;
      if (message?.view) {
        setView(message.view);
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, []);
  return view ? <App view={view} post={post} /> : null;
}

createRoot(container).render(
  <React.StrictMode>
    <DevicesShell />
  </React.StrictMode>,
);

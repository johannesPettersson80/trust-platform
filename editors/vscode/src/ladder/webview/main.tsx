import React from "react";
import { createRoot } from "react-dom/client";
import { LadderEditor } from "./LadderEditor";
import { getVsCodeApi } from "../../visual/runtime/webview/vscodeApi";
import "./styles.css";

const vscodeApi = getVsCodeApi();

function reportFatal(message: string, stack?: string): void {
  const body = document.body;
  if (body) {
    body.innerHTML = `
      <div style="padding:16px;color:var(--vscode-editor-foreground, var(--vscode-foreground, #cccccc));font-family:var(--vscode-font-family);">
        <h2 style="margin:0 0 8px 0;font-size:16px;">Ladder webview fatal error</h2>
        <pre style="white-space:pre-wrap;font-size:12px;">${message}${stack ? `\n\n${stack}` : ""}</pre>
      </div>
    `;
  }
  vscodeApi.postMessage({
    type: "webviewBootError",
    message,
    stack,
  });
}

window.addEventListener("error", (event) => {
  const err = event.error as Error | undefined;
  reportFatal(event.message || "Unknown webview error", err?.stack);
});

window.addEventListener("unhandledrejection", (event) => {
  const reason = event.reason as Error | string | undefined;
  const message =
    reason instanceof Error ? reason.message : String(reason ?? "Unknown rejection");
  const stack = reason instanceof Error ? reason.stack : undefined;
  reportFatal(message, stack);
});

/**
 * Entry point for the Ladder editor webview
 */
const container = document.getElementById("root");

if (!container) {
  throw new Error("Root element not found");
}

try {
  const root = createRoot(container);
  root.render(<LadderEditor />);
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  const stack = error instanceof Error ? error.stack : undefined;
  reportFatal(message, stack);
}

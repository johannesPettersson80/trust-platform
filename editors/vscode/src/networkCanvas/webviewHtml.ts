import * as path from "path";
import * as vscode from "vscode";

import type { NCGraph } from "./webview/types";

export function networkCanvasWebviewHtml(
  webview: vscode.Webview,
  context: vscode.ExtensionContext,
  initialGraph: NCGraph
): string {
  const scriptUri = webview.asWebviewUri(
    vscode.Uri.file(
      path.join(context.extensionPath, "media", "networkCanvasWebview.js")
    )
  );
  const styleUri = webview.asWebviewUri(
    vscode.Uri.file(
      path.join(context.extensionPath, "media", "networkCanvasWebview.css")
    )
  );
  const nonce = webviewNonce();
  const csp = `default-src 'none'; img-src ${webview.cspSource} data: https:; style-src ${webview.cspSource} 'unsafe-inline'; script-src ${webview.cspSource} 'unsafe-eval' 'nonce-${nonce}'; font-src ${webview.cspSource} data:;`;
  const initialHost = initialGraph.hosts[0];
  const initialRuntime = initialGraph.hosts.flatMap((host) => [
    ...host.runtimes,
    ...host.containers.flatMap((container) => container.runtimes),
  ])[0];
  const initialStatus = initialRuntimeStatus(
    initialRuntime?.health ?? "stopped",
    initialRuntime?.mode
  );
  const initialGraphJson = JSON.stringify(initialGraph).replace(
    /[<>&\u2028\u2029]/g,
    (character) => `\\u${character.charCodeAt(0).toString(16).padStart(4, "0")}`
  );
  return `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta http-equiv="Content-Security-Policy" content="${csp}" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Devices &amp; Connections</title>
    <link rel="stylesheet" href="${styleUri}" />
    <style>
      * { box-sizing: border-box; margin: 0; padding: 0; }
      html, body, #root {
        width: 100%; height: 100%; overflow: hidden;
        font-family: var(--vscode-font-family, -apple-system, "Segoe UI", sans-serif);
        background: var(--trust-canvas, var(--vscode-editor-background, #0f1116));
        color: var(--trust-text, var(--vscode-foreground, #eef1f5));
      }
      .initial-canvas {
        width: 100%; height: 100%; display: flex; flex-direction: column;
        background: var(--trust-canvas, var(--vscode-editor-background, #0f1116));
        color: var(--trust-text-muted, var(--vscode-descriptionForeground, #949cab));
      }
      .initial-canvas__header {
        height: 42px; display: flex; align-items: center; padding: 0 14px;
        border-bottom: 1px solid var(--vscode-panel-border, var(--vscode-widget-border, #3a3f4b));
        color: var(--trust-text, var(--vscode-foreground, #eef1f5)); font-size: 12px; font-weight: 650;
      }
      .initial-canvas__body { flex: 1; display: grid; place-items: center; }
      .initial-canvas__host {
        min-width: 310px; padding: 14px; border: 1px solid var(--vscode-panel-border, var(--vscode-widget-border, #3a3f4b));
        border-radius: 9px; background: var(--vscode-sideBar-background, var(--vscode-editor-background, #151922));
      }
      .initial-canvas__host-title { color: var(--trust-text, var(--vscode-foreground, #eef1f5)); font-size: 12px; font-weight: 650; }
      .initial-canvas__runtime {
        margin-top: 10px; padding: 10px; display: flex; align-items: center; justify-content: space-between; gap: 16px;
        border: 1px solid var(--vscode-panel-border, var(--vscode-widget-border, #3a3f4b)); border-radius: 7px;
        color: var(--trust-text, var(--vscode-foreground, #eef1f5));
      }
      .initial-canvas__status { color: var(--trust-text-muted, var(--vscode-descriptionForeground, #949cab)); font-size: 11px; font-weight: 600; }
      .initial-canvas__detail {
        margin-top: 8px; font-size: 10.5px;
        color: var(--trust-text-subtle, var(--vscode-disabledForeground, #6b7480));
      }
    </style>
  </head>
  <body>
    <div id="root">
      <div class="initial-canvas" role="status" aria-live="polite">
        <div class="initial-canvas__header">Devices &amp; Connections</div>
        <div class="initial-canvas__body">
          <div class="initial-canvas__host">
            <div class="initial-canvas__host-title">${escapeHtml(initialHost?.hostname ?? "This computer")}</div>
            <div class="initial-canvas__runtime">
              <strong>${escapeHtml(initialRuntime?.name ?? "Simulator")}</strong>
              <span class="initial-canvas__status">${escapeHtml(initialStatus)}</span>
            </div>
            <div class="initial-canvas__detail">Loading configured connections in the background.</div>
          </div>
        </div>
      </div>
    </div>
    <script nonce="${nonce}">window.__NC__ = ${initialGraphJson};</script>
    <script nonce="${nonce}" src="${scriptUri}"></script>
  </body>
</html>`;
}

function initialRuntimeStatus(health: string, mode?: string): string {
  switch (health) {
    case "connected":
      return mode === "online" ? "Connected" : "Running";
    case "starting":
      return "Starting…";
    case "error":
      return "Needs attention";
    default:
      return "Stopped";
  }
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function webviewNonce(): string {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let nonce = "";
  for (let index = 0; index < 32; index += 1) {
    nonce += alphabet.charAt(Math.floor(Math.random() * alphabet.length));
  }
  return nonce;
}

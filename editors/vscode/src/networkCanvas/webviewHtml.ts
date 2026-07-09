import * as path from "path";
import * as vscode from "vscode";

export function networkCanvasWebviewHtml(
  webview: vscode.Webview,
  context: vscode.ExtensionContext
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
  const csp = `default-src 'none'; img-src ${webview.cspSource} data: https:; style-src ${webview.cspSource} 'unsafe-inline'; script-src ${webview.cspSource} 'unsafe-eval'; font-src ${webview.cspSource} data:;`;
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
      .initial-loading {
        width: 100%; height: 100%;
        display: flex; flex-direction: column; align-items: center; justify-content: center;
        gap: 12px; text-align: center;
        background: var(--trust-canvas, var(--vscode-editor-background, #0f1116));
        color: var(--trust-text-muted, var(--vscode-descriptionForeground, #949cab));
      }
      .initial-loading__icon {
        width: 38px; height: 38px;
        color: var(--trust-text-subtle, var(--vscode-disabledForeground, #6b7480));
      }
      .initial-loading__title { font-size: 13.5px; font-weight: 600; }
      .initial-loading__detail {
        max-width: 300px; font-size: 12px;
        color: var(--trust-text-subtle, var(--vscode-disabledForeground, #6b7480));
      }
    </style>
  </head>
  <body>
    <div id="root">
      <div class="initial-loading" role="status" aria-live="polite">
        <svg class="initial-loading__icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect x="3" y="4.5" width="18" height="6" rx="1.5"></rect>
          <rect x="3" y="13.5" width="18" height="6" rx="1.5"></rect>
          <circle cx="6.6" cy="7.5" r="1" fill="currentColor" stroke="none"></circle>
          <circle cx="6.6" cy="16.5" r="1" fill="currentColor" stroke="none"></circle>
        </svg>
        <div class="initial-loading__title">Loading your devices...</div>
        <div class="initial-loading__detail">Reading the project's runtime and connections.</div>
      </div>
    </div>
    <script src="${scriptUri}"></script>
  </body>
</html>`;
}

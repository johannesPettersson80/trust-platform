import * as vscode from "vscode";

import { ioPanelStyles } from "./styles";

// Shipped Live Values webview document. The host controller stays in ioPanel.ts.

export function ioPanelHtml(webview: vscode.Webview, extensionUri: vscode.Uri): string {
  const nonce = getNonce();
  const codiconUri = webview.asWebviewUri(
    vscode.Uri.joinPath(
      extensionUri,
      "node_modules",
      "@vscode",
      "codicons",
      "dist",
      "codicon.css"
    )
  );
  const scriptUri = webview.asWebviewUri(
    vscode.Uri.joinPath(extensionUri, "media", "ioPanel.js")
  );
  const adsRowsScriptUri = webview.asWebviewUri(
    vscode.Uri.joinPath(extensionUri, "media", "ioPanelAdsRows.js")
  );
    return `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; font-src ${webview.cspSource}; script-src ${webview.cspSource} 'nonce-${nonce}';" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Live Values</title>
    <link href="${codiconUri}" rel="stylesheet" />
    <style>
${ioPanelStyles}    </style>
  </head>
  <body>
    <header>
      <div class="header-top">
        <div class="toolbar">
          <button id="releaseAllForces" type="button" class="release-all" style="display:none" title="Release every forced value on this target" aria-label="Release all forces">Release all forces</button>
          <button
            id="settings"
            class="icon-btn"
            title="Open runtime settings"
            aria-label="Open runtime settings"
            type="button"
          >
            <span class="codicon codicon-settings-gear" aria-hidden="true"></span>
          </button>
        </div>
        <div class="runtime-status">
          <span id="runtimeStatusText" class="status-pill disconnected">Stopped</span>
        </div>
      </div>
      <div class="target-strip" aria-label="Active Live Values target">
        <span>Target</span>
        <span id="targetLabel" class="target-label" title="Simulator">Simulator</span>
        <span id="scanLabel" class="scan-label" title="No runtime scan has been received yet">scan --</span>
      </div>
      <div
        id="forcePolicy"
        class="force-policy"
        aria-live="polite"
      >Force policy: simulator pins immediately; managed/remote targets require Arm force first.</div>
      <div class="header-search">
        <input id="filter" placeholder="Filter by name, address, or symbol" />
        <button id="forcedFilter" class="forced-filter" type="button" style="display:none" aria-pressed="false" title="No forced values">Forced</button>
        <div class="numeric-format" aria-label="Numeric display format">
          <span class="numeric-format-label">Format</span>
          <button class="format-toggle active" type="button" data-numeric-format="dec" aria-pressed="true" title="Show numeric values as decimal">DEC</button>
          <button class="format-toggle" type="button" data-numeric-format="hex" aria-pressed="false" title="Show BYTE/WORD/DWORD values as IEC hex literals">HEX</button>
          <button class="format-toggle" type="button" data-numeric-format="bin" aria-pressed="false" title="Show BYTE/WORD/DWORD values as IEC binary literals">BIN</button>
        </div>
      </div>
      <div class="status" id="status">Live Values loading...</div>
    </header>

    <div class="panel">
      <div id="runtimeView" class="runtime-view">
        <div id="sections" class="tree"></div>
        <div class="diagnostics" id="diagnostics" style="display:none">
          <div class="diagnostics-header">
            <div class="diagnostics-title">Runtime diagnostics</div>
            <div class="diagnostics-summary" id="diagnosticsSummary"></div>
          </div>
          <div class="diagnostics-runtime" id="diagnosticsRuntime"></div>
          <div class="diagnostics-list" id="diagnosticsList"></div>
        </div>
      </div>
      <div id="settingsPanel" class="settings-panel">
        <div class="settings-header">
          <div>
            <div class="settings-title">Runtime Settings</div>
            <div class="settings-subtitle">
              Stored in workspace settings for this project.
            </div>
          </div>
          <div class="settings-actions">
            <button id="settingsSave" title="Save runtime settings" aria-label="Save runtime settings">Save</button>
            <button id="settingsCancel" class="button-ghost" title="Close without saving" aria-label="Close without saving">Close</button>
          </div>
        </div>
        <div class="settings-grid">
          <section class="settings-section">
            <h2>Runtime Control</h2>
            <div class="settings-row">
              <label for="runtimeControlEndpoint">Endpoint</label>
              <input
                id="runtimeControlEndpoint"
                type="text"
                placeholder="unix:///tmp/trust-debug.sock or tcp://127.0.0.1:9901"
                autocomplete="off"
              />
            </div>
            <div class="settings-row">
              <label for="runtimeInlineValuesEnabled">Inline values</label>
              <input
                id="runtimeInlineValuesEnabled"
                type="checkbox"
              />
            </div>
            <div class="settings-help">
              Inline values show live runtime values in the editor.
            </div>
          </section>
          <section class="settings-section">
            <h2>Runtime Sources</h2>
            <div class="settings-row">
              <label for="runtimeIncludeGlobs">Include globs</label>
              <textarea
                id="runtimeIncludeGlobs"
                placeholder="**/*.{st,ST,pou,POU}"
              ></textarea>
            </div>
            <div class="settings-row">
              <label for="runtimeExcludeGlobs">Exclude globs</label>
              <textarea id="runtimeExcludeGlobs"></textarea>
            </div>
            <div class="settings-row">
              <label for="runtimeIgnorePragmas">Ignore pragmas</label>
              <textarea
                id="runtimeIgnorePragmas"
                placeholder="@trustlsp:runtime-ignore"
              ></textarea>
            </div>
            <div class="settings-help">
              One entry per line. Leave blank to use defaults.
            </div>
          </section>
          <section class="settings-section">
            <h2>Debug Adapter</h2>
            <div class="settings-row">
              <label for="debugAdapterPath">Adapter path</label>
              <input id="debugAdapterPath" type="text" autocomplete="off" />
            </div>
            <div class="settings-row">
              <label for="debugAdapterArgs">Adapter args</label>
              <textarea id="debugAdapterArgs"></textarea>
            </div>
            <div class="settings-row">
              <label for="debugAdapterEnv">Adapter env</label>
              <textarea
                id="debugAdapterEnv"
                placeholder="KEY=VALUE"
              ></textarea>
            </div>
            <div class="settings-help">
              Env entries can be KEY=VALUE per line or JSON.
            </div>
          </section>
          <section class="settings-section">
            <h2>Language Server</h2>
            <div class="settings-row">
              <label for="serverPath">Server path</label>
              <input id="serverPath" type="text" autocomplete="off" />
            </div>
            <div class="settings-row">
              <label for="traceServer">Trace level</label>
              <select id="traceServer">
                <option value="off">Off</option>
                <option value="messages">Messages</option>
                <option value="verbose">Verbose</option>
              </select>
            </div>
          </section>
        </div>
      </div>
    </div>

    <script nonce="${nonce}" src="${adsRowsScriptUri}"></script>
    <script nonce="${nonce}" src="${scriptUri}"></script>
  </body>
</html>`;
}

function getNonce(): string {
  let text = "";
  const possible =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  for (let i = 0; i < 32; i += 1) {
    text += possible.charAt(Math.floor(Math.random() * possible.length));
  }
  return text;
}

import * as vscode from "vscode";

export function getHtml(webview: vscode.Webview, extensionUri: vscode.Uri): string {
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
  return `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${
      webview.cspSource
    } 'unsafe-inline'; font-src ${webview.cspSource}; script-src ${
      webview.cspSource
    } 'nonce-${nonce}';" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Live Values</title>
    <link href="${codiconUri}" rel="stylesheet" />
    <style>
      :root {
        color-scheme: light dark;
        /* Kept aligned with the active Live Values panel and the shared truST
           product chrome; this legacy view must not reintroduce private colors. */
        --trust-canvas: var(--vscode-editor-background, #0f1116);
        --trust-surface: var(--vscode-editorWidget-background, #1b1f28);
        --trust-surface-raised: var(--vscode-editorHoverWidget-background, #222732);
        --trust-text: var(--vscode-foreground, #cfd6e0);
        --trust-text-muted: var(--vscode-descriptionForeground, #949cab);
        --trust-text-subtle: var(--vscode-disabledForeground, #6b7480);
        --trust-on-accent: var(--vscode-button-foreground, #ffffff);
        --trust-mono: var(--vscode-editor-font-family, ui-monospace, SFMono-Regular, Menlo, monospace);
        --trust-border: var(--vscode-editorWidget-border, var(--vscode-panel-border, #2a2f3a));
        --trust-accent: var(--vscode-focusBorder, #4a9eff);
        --trust-ok: var(--vscode-charts-green, var(--vscode-testing-iconPassed, #46c265));
        --trust-warn: var(--vscode-charts-yellow, var(--vscode-editorWarning-foreground, #e0b341));
        --trust-danger: var(--vscode-charts-red, var(--vscode-errorForeground, #f0584f));
        --trust-input-bg: var(--vscode-input-background, #10141b);
        --trust-input-border: var(--vscode-input-border, var(--vscode-editorWidget-border, #343b47));
        --trust-selected-bg: color-mix(in srgb, var(--trust-accent) 18%, transparent);
        --trust-selected-strong-bg: color-mix(in srgb, var(--trust-accent) 28%, transparent);
        --trust-radius-sm: 4px;
        --trust-radius: 6px;
        --trust-radius-lg: 8px;
        --trust-pill: 999px;
      }

      * {
        box-sizing: border-box;
      }

      body {
        font-family: var(--vscode-font-family);
        font-size: var(--vscode-font-size);
        margin: 0;
        padding: 0;
        color: var(--trust-text);
        background: var(--trust-canvas);
      }

      header {
        position: sticky;
        top: 0;
        z-index: 10;
        display: flex;
        flex-direction: column;
        gap: 8px;
        padding: 8px;
        background: var(--trust-canvas);
        border-bottom: 1px solid var(--trust-border);
      }

      h1 {
        margin: 0;
        font-size: 13px;
        font-weight: 600;
      }

      .header-top {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
      }

      .header-search {
        display: flex;
      }

      .runtime-status {
        display: flex;
        align-items: center;
        gap: 12px;
        font-size: 12px;
        color: var(--trust-text-muted);
        flex-wrap: wrap;
      }

      .target-strip {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 10px;
        min-height: 22px;
        color: var(--trust-text-muted);
        font-size: 11px;
      }

      .target-label {
        color: var(--trust-text);
        font-weight: 600;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

      .mode-toggle {
        display: inline-flex;
        align-items: center;
        border: 1px solid var(--trust-border);
        border-radius: 999px;
        overflow: hidden;
      }

      .mode-button {
        background: transparent;
        border: none;
        color: var(--trust-text);
        padding: 4px 10px;
        font-size: 11px;
        font-weight: 600;
        cursor: pointer;
      }

      .mode-button.active {
        background: var(--trust-accent);
        color: var(--trust-on-accent);
      }

      .mode-button:disabled {
        cursor: default;
        opacity: 0.5;
      }

      .mode-subtitle {
        font-size: 11px;
        color: var(--trust-text-muted);
        margin-right: 8px;
      }

      .status-group {
        display: flex;
        align-items: center;
        gap: 6px;
      }

      .status-pill {
        padding: 2px 8px;
        border-radius: 999px;
        border: 1px solid var(--trust-border);
        background: var(--trust-surface);
        color: var(--trust-text);
        white-space: nowrap;
      }

      .status-pill.on,
      .status-pill.running {
        background: var(--trust-accent);
        color: var(--trust-on-accent);
        border-color: transparent;
      }

      .status-pill.off {
        opacity: 0.7;
      }

      .status-pill.connected {
        border-color: var(--trust-accent);
      }

      .status-pill.disconnected {
        opacity: 0.7;
      }

      .status-action {
        border: 1px solid var(--trust-border);
        background: transparent;
        color: var(--trust-text);
        padding: 2px 8px;
        border-radius: 999px;
        font-size: 11px;
      }

      .status-action:hover {
        background: var(--trust-surface);
      }

      .status-action:disabled {
        cursor: default;
        opacity: 0.5;
      }

      input#filter {
        padding: 4px 8px;
        border: 1px solid var(--trust-input-border);
        border-radius: 4px;
        min-width: 220px;
        background: var(--trust-input-bg);
        color: var(--vscode-input-foreground, var(--trust-text));
      }

      input#filter::placeholder {
        color: var(--vscode-input-placeholderForeground, var(--trust-text-muted));
      }

      button {
        background: var(--trust-accent);
        border: none;
        color: var(--trust-on-accent);
        padding: 4px 10px;
        border-radius: 4px;
        cursor: pointer;
        font-weight: 600;
      }

      button:hover {
        background: var(--trust-selected-strong-bg);
      }

      button:disabled {
        background: var(--vscode-button-secondaryBackground, var(--trust-surface));
        border: 1px solid var(--trust-border);
        color: var(--trust-text-subtle);
        cursor: not-allowed;
        opacity: 1;
      }

      button:disabled:hover {
        background: var(--vscode-button-secondaryBackground, var(--trust-surface));
      }

      .panel {
        background: transparent;
        border: none;
        border-radius: 0;
        padding: 8px;
      }

      .toolbar {
        display: flex;
        align-items: center;
        gap: 8px;
      }

      .icon-btn {
        width: 28px;
        height: 28px;
        padding: 0;
        border-radius: 6px;
        border: 1px solid var(--trust-border);
        background: transparent;
        color: var(--trust-text);
        display: inline-flex;
        align-items: center;
        justify-content: center;
      }

      .icon-btn .codicon {
        font-size: 16px;
        line-height: 1;
      }

      .icon-btn:hover {
        background: var(--trust-selected-bg);
      }

      .icon-btn:active {
        background: var(--trust-surface);
      }

      .icon-btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
      }

      .icon-btn:disabled:hover {
        background: transparent;
      }

      .icon-btn.primary {
        border-color: transparent;
        background: var(--trust-accent);
        color: var(--trust-on-accent);
      }

      .icon-btn.primary:hover {
        background: var(--trust-selected-strong-bg);
      }

      .tree {
        display: flex;
        flex-direction: column;
        gap: 4px;
      }

      details.tree-node > summary {
        list-style: none;
        cursor: pointer;
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 2px 6px;
        border-radius: 4px;
        font-size: 12px;
        font-weight: 600;
        color: var(--trust-text);
      }

      details.tree-node > summary:hover {
        background: var(--trust-selected-bg);
      }

      details.tree-node > summary::-webkit-details-marker {
        display: none;
      }

      details.tree-node > summary::before {
        content: "▸";
        display: inline-block;
        width: 12px;
        color: var(--trust-text-muted);
        transform: translateY(-1px);
      }

      details.tree-node[open] > summary::before {
        content: "▾";
      }

      .tree-node.level-1 {
        padding-left: 12px;
      }

      .tree-node.level-2 {
        padding-left: 22px;
      }

      .tree-node.level-3 {
        padding-left: 32px;
      }

      /* One shared grid for the whole section so every row — BOOL or numeric, with or
         without a write-box — lines its VALUE/TYPE/STATE/ACTIONS up under the same headers.
         Rows use subgrid so the column tracks are shared, not re-derived per row. */
      .rows {
        display: grid;
        grid-template-columns:
          minmax(82px, 1fr)
          minmax(52px, auto)
          minmax(38px, auto)
          minmax(52px, auto)
          minmax(128px, auto);
        row-gap: 2px;
        padding: 2px 4px 2px 10px;
      }

      .row,
      .row-header {
        grid-column: 1 / -1;
        display: grid;
        grid-template-columns: subgrid;
        align-items: center;
        column-gap: 6px;
      }

      .row {
        padding: 2px 4px;
        border-radius: 4px;
        font-size: 12px;
      }

      .row-header {
        padding: 2px 4px;
        color: var(--trust-text-muted);
        font-size: 10px;
        font-weight: 700;
        text-transform: uppercase;
        letter-spacing: 0.04em;
      }

      .row-header .actions-heading {
        text-align: right;
      }

      .row:hover {
        background: var(--trust-selected-bg);
      }

      /* A forced value is ALWAYS visibly marked in the State column, not just via an action button. */
      .row.forced {
        background: color-mix(in srgb, var(--vscode-testing-iconPassed, #1f8f4e) 12%, transparent);
      }

      .state-cell,
      .type-cell {
        color: var(--trust-text-muted);
        font-size: 11px;
        white-space: nowrap;
      }

      .state-badge {
        display: inline-block;
        min-width: 64px;
        box-sizing: border-box;
        text-align: center;
        padding: 1px 6px;
        border-radius: 6px;
        border: 1px solid var(--trust-border);
        font-size: 10px;
        font-weight: 700;
        letter-spacing: 0.04em;
        line-height: 1.4;
      }

      .state-badge.live {
        color: var(--trust-text-muted);
        text-transform: uppercase;
      }

      .state-badge.forced {
        color: var(--trust-on-accent);
        background: var(--vscode-testing-iconPassed, #1f8f4e);
        border-color: var(--vscode-testing-iconPassed, #1f8f4e);
      }

      .row .name {
        display: flex;
        flex-direction: column;
        gap: 2px;
      }

      .row .name .type {
        font-size: 10px;
        color: var(--trust-text-muted);
      }

      .row .name .address {
        font-size: 10px;
        color: var(--trust-text-muted);
      }

      .row .value {
        color: var(--trust-text);
        font-family: var(--vscode-editor-font-family);
        font-size: 11px;
      }

      .row .actions {
        display: flex;
        align-items: center;
        gap: 4px;
        justify-content: flex-end;
        flex-wrap: nowrap;
      }

      .value-input {
        width: 52px;
        height: 24px;
        padding: 2px 4px;
        border: 1px solid var(--trust-input-border);
        border-radius: 3px;
        background: var(--trust-input-bg);
        color: var(--vscode-input-foreground, var(--trust-text));
        font-family: var(--vscode-editor-font-family);
        font-size: 11px;
      }

      .value-input:disabled {
        opacity: 0.55;
        cursor: not-allowed;
      }

      /* Invisible placeholder that reserves the write-box slot on rows without an editable
         field, so every section's actions column keeps the same width and the headers align. */
      .value-input-spacer {
        flex: 0 0 52px;
        height: 24px;
      }

      .value-input.bool-toggle {
        cursor: pointer;
        font-weight: 700;
        text-align: center;
      }

      .value-input.bool-toggle[aria-pressed="true"] {
        border-color: var(--trust-accent);
        background: var(--trust-selected-bg);
        color: var(--trust-text);
      }

      .mini-btn {
        min-width: 48px;
        height: 24px;
        padding: 0 5px;
        border-radius: 3px;
        font-size: 11px;
        font-weight: 600;
        border: 1px solid var(--trust-input-border);
        background: var(--trust-accent);
        color: var(--trust-on-accent);
        display: inline-flex;
        align-items: center;
        justify-content: center;
        line-height: 1;
        white-space: nowrap;
        cursor: pointer;
      }

      /* The force/release control keeps a fixed width so its label can change between
         "Force", "Arm force" and "Release" without resizing — and so every section's
         actions column stays the same width, keeping the tables aligned across sections. */
      .mini-btn.force-slot {
        width: 72px;
      }

      .mini-btn:hover {
        background: var(--trust-selected-strong-bg);
      }

      .mini-btn.active {
        background: var(--vscode-testing-iconPassed, #1f8f4e);
        color: var(--trust-on-accent);
        border-color: var(--vscode-testing-iconPassed, #1f8f4e);
        box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.18);
      }

      .mini-btn.armed {
        background: var(--trust-warn);
        color: var(--trust-canvas);
        border-color: var(--trust-warn);
      }

      .mini-btn:disabled {
        background: var(--trust-input-bg);
        border-color: var(--trust-input-border);
        color: var(--trust-text-subtle);
        box-shadow: none;
        opacity: 1;
        cursor: not-allowed;
      }

      .mini-btn:disabled:hover {
        background: var(--trust-input-bg);
      }

      .empty {
        grid-column: 1 / -1;
        font-size: 11px;
        color: var(--trust-text-muted);
        padding: 2px 6px 2px 24px;
      }

      .status {
        display: none;
        color: var(--trust-text);
        font-size: 12px;
        line-height: 1.35;
        padding: 4px 8px;
        border: 1px solid var(--trust-border);
        border-radius: 4px;
        background: var(--trust-surface);
      }

      .status:not(:empty) {
        display: block;
      }

      .status.status-ok {
        border-color: var(--trust-ok);
        background: color-mix(in srgb, var(--trust-ok) 12%, var(--trust-surface));
      }

      .status.status-error {
        border-color: var(--trust-danger);
        background: color-mix(in srgb, var(--trust-danger) 12%, var(--trust-surface));
      }

      .diagnostics {
        margin-top: 12px;
        border: 1px solid var(--trust-border);
        border-radius: 6px;
        background: var(--trust-surface);
        padding: 8px;
      }

      .diagnostics-header {
        display: flex;
        align-items: baseline;
        justify-content: space-between;
        gap: 8px;
        margin-bottom: 6px;
      }

      .diagnostics-title {
        font-size: 12px;
        font-weight: 600;
      }

      .diagnostics-summary {
        font-size: 11px;
        color: var(--trust-text-muted);
      }

      .diagnostics-runtime {
        font-size: 11px;
        color: var(--trust-text-muted);
        margin-bottom: 6px;
      }

      .diagnostics-list {
        display: flex;
        flex-direction: column;
        gap: 6px;
      }

      .diagnostic-item {
        padding: 6px 8px;
        border-radius: 4px;
        background: var(--trust-surface);
        border-left: 3px solid transparent;
      }

      .diagnostic-item.error {
        border-left-color: var(--trust-danger);
      }

      .diagnostic-item.warning {
        border-left-color: var(--trust-warn);
      }

      .diagnostic-message {
        font-size: 12px;
      }

      .diagnostic-meta {
        font-size: 11px;
        color: var(--trust-text-muted);
        margin-top: 2px;
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
      }

      .runtime-view.hidden {
        display: none;
      }

      .settings-panel {
        display: none;
        border: 1px solid var(--trust-border);
        border-radius: 8px;
        background: var(--trust-surface);
        padding: 12px;
      }

      .settings-panel.open {
        display: block;
      }

      .settings-header {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        gap: 12px;
        margin-bottom: 12px;
      }

      .settings-title {
        font-size: 13px;
        font-weight: 600;
      }

      .settings-subtitle {
        font-size: 11px;
        color: var(--trust-text-muted);
        margin-top: 2px;
      }

      .settings-grid {
        display: grid;
        gap: 12px;
      }

      .settings-section {
        border: 1px solid var(--trust-border);
        border-radius: 6px;
        padding: 10px;
        background: var(--trust-surface);
      }

      .settings-section h2 {
        margin: 0 0 8px;
        font-size: 11px;
        text-transform: uppercase;
        letter-spacing: 0.4px;
        color: var(--trust-text-muted);
      }

      .settings-row {
        display: grid;
        grid-template-columns: 160px 1fr;
        gap: 8px;
        align-items: center;
        margin-bottom: 8px;
      }

      .settings-row:last-child {
        margin-bottom: 0;
      }

      .settings-row label {
        font-size: 11px;
        color: var(--trust-text-muted);
      }

      .settings-row input,
      .settings-row textarea,
      .settings-row select {
        width: 100%;
        padding: 4px 6px;
        border: 1px solid var(--trust-input-border);
        border-radius: 4px;
        background: var(--trust-input-bg);
        color: var(--vscode-input-foreground, var(--trust-text));
        font-family: var(--vscode-editor-font-family);
        font-size: 12px;
      }

      .settings-row textarea {
        min-height: 56px;
        resize: vertical;
      }

      .settings-help {
        font-size: 11px;
        color: var(--trust-text-muted);
        margin-top: 4px;
      }

      .settings-actions {
        display: flex;
        align-items: center;
        gap: 8px;
      }

      .button-ghost {
        background: transparent;
        border: 1px solid var(--trust-border);
        color: var(--trust-text);
      }

      .button-ghost:hover {
        background: var(--trust-selected-bg);
      }
    </style>
  </head>
  <body>
    <header>
      <div class="header-top">
        <div class="toolbar">
          <div class="mode-toggle" role="group" aria-label="Runtime mode">
            <button id="modeSimulate" class="mode-button" type="button" title="Use the local runtime started by the debugger." aria-label="Use the local runtime started by the debugger">Local</button>
            <button id="modeOnline" class="mode-button" type="button" title="Connect to a running runtime at the configured endpoint." aria-label="Connect to a running runtime at the configured endpoint">External</button>
          </div>
          <button id="runtimeStart" type="button" title="Start or stop the selected runtime." aria-label="Start or stop the selected runtime">Start</button>
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
        <span id="targetLabel" class="target-label" title="Simulator (this computer)">Simulator (this computer)</span>
      </div>
      <div class="header-search">
        <input id="filter" placeholder="Filter by name or address" />
      </div>
      <div class="status" id="status">Live Values loading...</div>
    </header>

    <div class="panel">
      <div id="runtimeView" class="runtime-view">
        <div id="sections" class="tree"></div>
        <div class="diagnostics" id="diagnostics">
          <div class="diagnostics-header">
            <div class="diagnostics-title">Compile Diagnostics</div>
            <div class="diagnostics-summary" id="diagnosticsSummary">
              No compile run yet
            </div>
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
              <label for="runtimeControlAuthToken">Auth token</label>
              <input
                id="runtimeControlAuthToken"
                type="password"
                placeholder="Optional"
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

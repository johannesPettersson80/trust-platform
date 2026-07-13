export const foundationStyles = `      :root {
        color-scheme: light dark;
        /* Keep Live Values on the same product token layer as Devices & Connections and
           the visual editors. Every role ends in a hard fallback so missing VS Code theme
           variables cannot collapse to browser-default black text. */
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
        align-items: center;
        gap: 8px;
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

      .scan-label {
        color: var(--trust-text-muted);
        font-variant-numeric: tabular-nums;
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
        flex: 1 1 auto;
        min-width: 0;
        padding: 4px 8px;
        border: 1px solid var(--trust-input-border);
        border-radius: 4px;
        background: var(--trust-input-bg);
        color: var(--vscode-input-foreground, var(--trust-text));
      }

      /* Focus uses the panel accent (blue), not the browser default (amber = reads as a warning). */
      input#filter:focus {
        outline: none;
        border-color: var(--trust-accent);
        box-shadow: 0 0 0 1px var(--trust-accent);
      }

      input#filter::placeholder {
        color: var(--vscode-input-placeholderForeground, var(--trust-text-muted));
      }

      .numeric-format {
        display: inline-flex;
        align-items: center;
        gap: 3px;
        flex: 0 0 auto;
        border: 1px solid var(--trust-border);
        border-radius: 6px;
        padding: 2px;
        background: var(--trust-surface);
      }

      .numeric-format-label {
        color: var(--trust-text-muted);
        font-size: 10px;
        font-weight: 700;
        padding: 0 4px;
        text-transform: uppercase;
      }

      .format-toggle {
        min-width: 34px;
        height: 22px;
        padding: 0 6px;
        border: 1px solid transparent;
        border-radius: 4px;
        background: transparent;
        color: var(--trust-text-muted);
        font-size: 10px;
        font-weight: 700;
        line-height: 1;
      }

      .format-toggle:hover {
        background: var(--trust-selected-bg);
        color: var(--trust-text);
      }

      .format-toggle.active {
        background: var(--trust-selected-bg);
        border-color: var(--trust-input-border);
        color: var(--trust-text);
      }

      .forced-filter {
        height: 24px;
        flex: 0 0 auto;
        padding: 0 8px;
        border-radius: 999px;
        border: 1px solid var(--trust-input-border);
        background: var(--vscode-button-secondaryBackground, var(--trust-surface));
        color: var(--vscode-button-secondaryForeground, var(--trust-text));
        font-size: 11px;
        font-weight: 700;
        line-height: 1;
        white-space: nowrap;
      }

      .forced-filter:hover {
        background: var(--vscode-button-secondaryHoverBackground, var(--trust-selected-bg));
      }

      .forced-filter.active {
        border-color: var(--trust-warn);
        background: color-mix(in srgb, var(--trust-warn) 14%, var(--trust-surface));
        color: var(--trust-text);
        box-shadow: inset 2px 0 0 var(--trust-warn);
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

`;

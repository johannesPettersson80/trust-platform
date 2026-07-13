export const feedbackAndSettingsStyles = `      .status {
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

      .status.status-warn {
        border-color: var(--trust-warn);
        background: color-mix(in srgb, var(--trust-warn) 12%, var(--trust-surface));
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
`;

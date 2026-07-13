export const valueRowStyles = `      .row,
      .row-header {
        grid-column: 1 / -1;
        display: grid;
        grid-template-columns: subgrid;
        align-items: center;
        column-gap: 6px;
      }

      .row > *,
      .row-header > * {
        min-width: 0;
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

      /* A forced value is ALWAYS visibly marked (§0.5.5/§0.5.16): subtle amber wash + an amber
         left accent bar so overridden rows are unmistakable without shifting the columns. */
      .row.forced {
        background: color-mix(in srgb, var(--trust-warn) 13%, transparent);
        box-shadow: inset 2px 0 0 var(--trust-warn);
      }
		      .state-cell,
		      .type-cell {
		        color: var(--trust-text-muted);
		        font-size: 11px;
		        white-space: nowrap;
		      }

          .source-subtitle {
            color: var(--trust-text-muted);
            font-size: 10px;
            line-height: 1.2;
            overflow-wrap: anywhere;
            white-space: normal;
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

      .state-badge.good {
        color: var(--trust-ok);
        border-color: var(--trust-ok);
      }

      .state-badge.stale {
        color: var(--trust-warn);
        border-color: var(--trust-warn);
      }

      .state-badge.error {
        color: var(--trust-danger);
        border-color: var(--trust-danger);
      }

      .ads-quality {
        min-width: 0;
      }

      .quality-detail {
        max-width: 180px;
        margin-top: 2px;
        color: var(--trust-text-muted);
        font-size: 10px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

      /* A forced value is an operator OVERRIDE, not a healthy state (ISA-101 / TwinCAT / CODESYS
         convention): mark it amber (caution), never green. */
      .state-badge.forced {
        color: #161616;
        background: var(--trust-warn);
        border-color: var(--trust-warn);
      }
      /* Release clears an override → a restorative secondary action. Same ghost treatment per-row
         and for "Release all" so the two read as one control, distinct from the primary buttons. */
      .release-all,
      .mini-btn.release {
        background: transparent;
        color: var(--trust-text-muted);
        border: 1px solid var(--trust-input-border);
      }
      .release-all:hover,
      .mini-btn.release:hover {
        background: var(--trust-selected-bg);
        color: var(--trust-text);
        border-color: var(--trust-text-subtle);
      }

      .row .name {
        display: flex;
        flex-direction: column;
        gap: 2px;
        min-width: 0;
        overflow: hidden;
      }

      .row .name > div {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
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
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

      .row .actions {
        display: flex;
        align-items: center;
	        gap: 4px;
        justify-content: flex-end;
        flex-wrap: nowrap;
      }

      .value-input {
	        width: 46px;
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
	        flex: 0 0 46px;
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
	        min-width: 42px;
        height: 24px;
	        padding: 0 4px;
        border-radius: 3px;
        font-size: 11px;
        font-weight: 600;
        border: 1px solid var(--trust-input-border);
        background: var(--vscode-button-secondaryBackground, var(--trust-surface-raised));
        color: var(--vscode-button-secondaryForeground, var(--trust-text));
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
	        width: 62px;
      }

      .mini-btn:hover {
        background: var(--vscode-button-secondaryHoverBackground, var(--trust-selected-bg));
      }

      .mini-btn.active {
        background: color-mix(in srgb, var(--trust-warn) 14%, var(--trust-surface));
        color: var(--trust-text);
        border-color: var(--trust-warn);
        box-shadow: inset 2px 0 0 var(--trust-warn);
      }

      .mini-btn.armed {
        background: color-mix(in srgb, var(--trust-warn) 14%, var(--trust-surface));
        color: var(--trust-text);
        border-color: var(--trust-warn);
        box-shadow: inset 2px 0 0 var(--trust-warn);
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

`;

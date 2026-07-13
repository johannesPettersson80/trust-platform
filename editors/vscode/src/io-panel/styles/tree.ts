export const treeStyles = `      .panel {
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

      .write-hint {
        margin: 2px 4px 6px 10px;
        color: var(--trust-text-muted);
        font-size: 11px;
        line-height: 1.35;
      }

      .force-policy {
        margin: 4px 12px 8px;
        padding: 5px 8px;
        border: 1px solid var(--trust-border-subtle);
        border-left: 3px solid var(--trust-warn);
        border-radius: 4px;
        background: color-mix(in srgb, var(--trust-warn) 8%, var(--trust-surface));
        color: var(--trust-text);
        font-size: 11px;
        line-height: 1.35;
      }

      .force-policy.armed-target {
        background: color-mix(in srgb, var(--trust-warn) 12%, var(--trust-surface));
      }

      /* One shared grid for the whole section so every row — BOOL or numeric, with or
         without a write-box — lines its VALUE/TYPE/STATE/ACTIONS up under the same headers.
         Rows use subgrid so the column tracks are shared, not re-derived per row. */
      .rows {
		        display: grid;
		        grid-template-columns:
		          minmax(116px, 1fr)
		          minmax(52px, max-content)
		          minmax(38px, max-content)
		          minmax(64px, max-content)
          minmax(160px, max-content);
        column-gap: 6px;
        row-gap: 2px;
        padding: 2px 4px 2px 10px;
        overflow-x: auto;
      }

      .ads-rows {
        display: grid;
        grid-template-columns:
          minmax(180px, 1fr)
          minmax(72px, max-content)
          minmax(52px, max-content)
          minmax(88px, max-content);
        column-gap: 8px;
        row-gap: 2px;
        padding: 2px 4px 2px 10px;
        overflow-x: auto;
      }

      .ads-row,
      .ads-row-header {
        grid-column: 1 / -1;
        display: grid;
        grid-template-columns: subgrid;
        align-items: center;
        column-gap: 8px;
      }

      .ads-contract-problem {
        grid-column: 1 / -1;
        margin: 2px 4px 6px;
        padding: 7px 8px;
        border: 1px solid var(--trust-warn);
        border-left-width: 3px;
        border-radius: 4px;
        background: color-mix(in srgb, var(--trust-warn) 10%, var(--trust-surface));
      }

      .ads-contract-problem.incompatible_schema,
      .ads-contract-problem.invalid_snapshot {
        border-color: var(--trust-danger);
        background: color-mix(in srgb, var(--trust-danger) 10%, var(--trust-surface));
      }

      .ads-contract-problem-message {
        color: var(--trust-text);
        font-size: 11px;
        font-weight: 700;
      }

      .ads-contract-problem-detail {
        margin-top: 2px;
        color: var(--trust-text-muted);
        font-size: 10px;
        line-height: 1.35;
      }

      .ads-row {
        padding: 4px;
        border-radius: 4px;
        font-size: 12px;
      }

      .ads-row:hover {
        background: var(--trust-selected-bg);
      }

      .ads-row-header {
        padding: 2px 4px;
        color: var(--trust-text-muted);
        font-size: 10px;
        font-weight: 700;
        text-transform: uppercase;
        letter-spacing: 0.04em;
      }

      .ads-row .name {
        display: flex;
        flex-direction: column;
        gap: 2px;
        min-width: 0;
        overflow: hidden;
      }

      .ads-row .name > div {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

      .ads-row .value {
        color: var(--trust-text);
        font-family: var(--trust-mono);
        font-size: 11px;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

`;

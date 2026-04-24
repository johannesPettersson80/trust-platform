# Browser Analysis WASM OpenPLC Event Mapping

Purpose:

- Define how an OpenPLC-style web editor should map UI events to `WasmAnalysisEngine` calls.

Scope:

- Browser-side analysis only (`diagnostics`, `hover`, `completion`).
- No runtime execution or debug control in browser.

## Event-to-API Mapping

| OpenPLC-style editor event | Analyzer call | Request shape | Notes |
|---|---|---|---|
| Editor content changed (debounced 120-200ms) | `applyDocuments` then `diagnostics` | `{ documents: [{ uri, text }] }` then `{ uri }` | Keep document revision number client-side. Ignore stale responses from older revisions. |
| Cursor moved / selection changed | `completion` (if prefix token exists) | `{ uri, position, limit }` | Only request completion when current token prefix length >= 1. |
| Mouse hover settles on token | `hover` | `{ uri, position }` | Debounce hover ~120ms to prevent request flood. |
| Completion accepted | `applyDocuments` | `{ documents: [{ uri, text }] }` | Re-apply updated text, then refresh diagnostics. |
| Editor startup | `status` (optional) | `{}` | Use for diagnostics/logging panel visibility only. |
| In-flight request cancelled by UI | `cancel` | `{ requestId }` | Cooperative cancellation: ignore/suppress stale response in UI. |

## Reference Integration Loop

1. Worker starts and creates `WasmAnalysisEngine`.
2. Host applies initial document set via `applyDocuments`.
3. Host enters live loop:

- On edits: `applyDocuments` + `diagnostics`.
- On hover: `hover`.
- On typing/caret movement: `completion`.
4. Host renders diagnostics markers, hover popover, completion list.

## Performance/Robustness Baseline

- Edit debounce: 120-200ms.
- Completion debounce: 80-120ms.
- Hover debounce: 100-150ms.
- Per-request timeout: 1.2-2.2s depending on call type.
- Stale response policy: drop responses for non-current document revision.

## Artifacts

- Generic runtime-style browser demo:
  `docs/internal/prototypes/browser_analysis_wasm_spike/web/index.html`
- OpenPLC-shell integration demo:
  `docs/internal/prototypes/browser_analysis_wasm_spike/web/openplc-shell.html`
- Thin client wrapper module:
  `docs/internal/prototypes/browser_analysis_wasm_spike/web/analysis-client.js`
- Prototype npm package form:
  `docs/internal/prototypes/browser_analysis_wasm_spike/npm/trust-wasm-analysis-client/`

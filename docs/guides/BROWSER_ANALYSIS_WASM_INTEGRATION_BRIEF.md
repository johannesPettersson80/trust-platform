# Browser Analysis WASM Integration Brief (OpenPLC-style Web Editors)

Purpose:

- Define a low-risk integration path to embed truST static analysis in an existing browser editor.

Scope of this integration:

- `diagnostics`
- `hover`
- `completion`
- browser-host event mapping compatible with OpenPLC-style web editors

Out of scope:

- Runtime execution model replacement.
- Debug adapter/runtime stepping in browser.
- Deploy/control workflows.

## 1. Why this path

- Browser-native: analyzer executes inside a web worker via WASM.
- No backend analysis service required for core language intelligence.
- Clear contract boundary through JSON methods on `WasmAnalysisEngine`.

## 2. Integration Architecture

Host editor (for example OpenPLC web editor):

1. Maintains open document text in browser memory.
2. Sends updates to worker through request envelope:
   - `applyDocuments`
   - `diagnostics`
   - `hover`
   - `completion`
3. Renders results in existing editor UX.

Worker layer:

- Loads `trust_wasm_analysis.js` + `.wasm`.
- Owns engine lifecycle.
- Enforces request IDs, timeout, and cancellation envelope.

## 3. API Contract (Current)

`WasmAnalysisEngine` methods:

- `applyDocumentsJson`
- `diagnosticsJson`
- `hoverJson`
- `completionJson`
- `statusJson`

Request envelope:

```json
{
  "id": "req-42",
  "method": "completion",
  "params": {
    "uri": "memory:///main.st",
    "position": { "line": 7, "character": 3 },
    "limit": 25
  },
  "timeoutMs": 1500
}
```

Cancel envelope:

```json
{
  "method": "cancel",
  "params": { "requestId": "req-42" }
}
```

## 4. Minimal TypeScript Host Sketch

```ts
type Request = {
  id: string;
  method: "applyDocuments" | "diagnostics" | "hover" | "completion" | "status";
  params?: unknown;
  timeoutMs?: number;
};

const worker = new Worker("/worker.js", { type: "module" });

function send(request: Request): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const onMessage = (event: MessageEvent) => {
      const msg = event.data;
      if (!msg || msg.id !== request.id) return;
      worker.removeEventListener("message", onMessage);
      if (msg.error) reject(msg.error);
      else resolve(msg.result);
    };
    worker.addEventListener("message", onMessage);
    worker.postMessage(request);
  });
}
```

## 5. Integration Acceptance Criteria (Phase 1)

- `applyDocuments` round-trip works on every edit debounce cycle.
- Diagnostics markers update on syntax/semantic errors.
- Hover displays symbol/type details.
- Completion provides stable suggestions at cursor.
- Worker startup errors are surfaced clearly to user.

## 6. Known Constraints

- Cancellation is currently cooperative (response suppression), not true preemption.
- Filesystem crawling is not part of worker API; host provides document content.
- Full LSP parity (rename/code actions/formatting/semantic tokens) is deferred.

## 7. Next Step

- Use the existing OpenPLC-shell integration demo to validate event wiring and UX:
  - `docs/internal/prototypes/browser_analysis_wasm_spike/web/openplc-shell.html`
  - Served via `scripts/run_browser_analysis_wasm_spike_demo.sh`
- Reuse thin wrapper module/package to reduce host integration boilerplate:
  - `docs/internal/prototypes/browser_analysis_wasm_spike/web/analysis-client.js`
  - `docs/internal/prototypes/browser_analysis_wasm_spike/npm/trust-wasm-analysis-client/`
- Validate with shared acceptance checklist:
  - `docs/guides/BROWSER_ANALYSIS_WASM_PARTNER_ACCEPTANCE_CHECKLIST.md`

## 8. OpenPLC Event Mapping

Detailed event-to-API mapping is documented in:

- `docs/guides/BROWSER_ANALYSIS_WASM_OPENPLC_EVENT_MAPPING.md`

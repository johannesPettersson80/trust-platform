# Browser Analysis WASM Spike (Deliverable 10)

Last updated: 2026-02-15

This guide defines the Deliverable 10 browser-analysis spike contract:

- worker-based browser transport
- WASM analysis adapter API
- parity/performance evidence versus native analysis
- explicit unsupported scope and go/no-go decision

## Scope (v1 Spike)

Supported workflows:

- diagnostics
- hover
- completion
- runtime-web-UI visual parity (shared palette/logo/layout) with dark/light theme toggle

Supported transport model:

- browser `Worker` message protocol with request IDs
- timeout handling per request (`timeoutMs`)
- cancellation token path (`cancel` by request ID)
- startup/fatal error signaling

Non-goals for this spike:

- runtime/debug adapter integration in browser
- semantic tokens, code actions, rename, formatting parity
- filesystem-native workspace loading in sandboxed browser
- online deploy/control workflows

## Architecture

Core adapter crate:

- `crates/trust-wasm-analysis/`
- exposes `BrowserAnalysisEngine` (native API) and `WasmAnalysisEngine` (JSON boundary for browser bindings)

Browser host prototype assets:

- `docs/internal/prototypes/browser_analysis_wasm_spike/`
- worker transport shim:
  - `docs/internal/prototypes/browser_analysis_wasm_spike/web/worker.js`
  - `docs/internal/prototypes/browser_analysis_wasm_spike/web/main.js`
  - `docs/internal/prototypes/browser_analysis_wasm_spike/web/index.html`
  - `docs/internal/prototypes/browser_analysis_wasm_spike/web/openplc-shell.html`
  - `docs/internal/prototypes/browser_analysis_wasm_spike/web/analysis-client.js`

## API Boundary

`WasmAnalysisEngine` JSON methods:

- `applyDocumentsJson`
- `diagnosticsJson`
- `hoverJson`
- `completionJson`
- `statusJson`

Worker request envelope:

```json
{
  "id": "req-12",
  "method": "completion",
  "params": {
    "uri": "memory:///main.st",
    "position": { "line": 5, "character": 2 },
    "limit": 25
  },
  "timeoutMs": 1500
}
```

Cancellation envelope:

```json
{
  "method": "cancel",
  "params": { "requestId": "req-12" }
}
```

## Validation Commands

Core parity/performance gate:

```bash
scripts/check_mp010_browser_analysis.sh
```

Browser build pipeline:

```bash
scripts/build_browser_analysis_wasm_spike.sh
```

One-command demo runner:

```bash
scripts/run_browser_analysis_wasm_spike_demo.sh
```

Manual browser run:

```bash
python3 -m http.server 4173 --directory target/browser-analysis-wasm
```

Open:

```text
http://127.0.0.1:4173/web/
```

Partner demo docs:

- `docs/guides/BROWSER_ANALYSIS_WASM_DEMO_SCRIPT.md`
- `docs/guides/BROWSER_ANALYSIS_WASM_INTEGRATION_BRIEF.md`
- `docs/guides/BROWSER_ANALYSIS_WASM_OPENPLC_EVENT_MAPPING.md`
- `docs/guides/BROWSER_ANALYSIS_WASM_PARTNER_ACCEPTANCE_CHECKLIST.md`

## Parity and Performance Evidence (Local Spike)

Measured by `crates/trust-wasm-analysis/tests/mp010_parity.rs` on the
`examples/plant_demo/` corpus (24 iterations after warm-up):

- diagnostics: adapter `225us`, native `173us`
- hover: adapter `25203us`, native `25991us`
- completion: adapter `88276us`, native `63500us`

Spike budgets enforced by tests:

- adapter latency <= `4x native + 120ms` headroom per workflow
- absolute cap <= `2s` per measured workflow run

## Unsupported / Native-Only Features

- direct runtime process interaction/debug stepping remains native/runtime-socket only
- browser worker currently expects virtual-memory documents (no direct host filesystem walk)
- transport cancellation is cooperative (response suppression), not mid-analysis preemption
- full editor protocol parity (semantic tokens/actions/rename/format) deferred to next phase

## Go/No-Go Decision

Decision: **GO** for a production-scoped next phase limited to:

- diagnostics
- hover
- completion
- worker lifecycle hardening and transport reliability

Deferred to later scope:

- full LSP parity set
- runtime/debug flows in-browser
- advanced multi-file workspace indexing features requiring host capabilities

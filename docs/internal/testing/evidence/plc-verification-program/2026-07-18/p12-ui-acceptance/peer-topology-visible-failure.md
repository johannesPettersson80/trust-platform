# Peer Topology Visible-Failure Journey

Status: provisional rendered evidence; not `ux_accepted`

## Journey

- Surface: VS Code Devices & Connections
- Actor: engineer reviewing a running or stopped local runtime and discovered peers
- Entry point: `truST: Devices & Connections`
- Failure stimulus: one peer reports an unknown connector-confidence identifier
- Required visible outcome: the peer-topology failure remains visible and the usable local simulator topology is not discarded
- Linked invariant: `UI_STATUS_001`
- Source revision: `399e86a8feb1a61324ebeb7d5c0c0649410debd6`
- Extension version: `0.24.54`
- Runner: `docs/internal/testing/evidence/vscode-ui-ux-acceptance/2026-06-25/runners/cdp_peer_topology_failure.js`
- Runner SHA-256: `d0f5a1c04d30a14d68c01267e0a91e5c5bf5b212028460dea37e2a9d6458e608`

The runner launches the real Extension Development Host, opens the shipped
Devices & Connections webview, injects the reviewed malformed-peer graph at the
webview boundary, checks rendered text in the inner frame, and captures the
Xvfb root window. Each theme run passed the same assertions:

- `Peer topology degraded` is visible;
- `unknown connector confidence` is visible;
- `Simulator stopped` is visible, proving the local topology was preserved.

## Captures

| Theme | Screenshot | Screenshot SHA-256 | Result JSON SHA-256 |
| --- | --- | --- | --- |
| dark | `peer-topology-failure-dark.png` | `998209aeffbff2f03999e0bdc5fcbead04c6089abaf6c937d77a411051193e69` | `d8de1dca9ee0d45c946a41692b0b268e72a0a3eacfd4c67e514480331a46c6bd` |
| light | `peer-topology-failure-light.png` | `86fea0f057d66d2a81803d3899271adff6c1d6832f2a6d68fe901b1619adc266` | `dafd2f49a6af027877fb7129b67148d0c8c9ad1deac2285dafd0bd8ac5af8eb7` |
| high contrast | `peer-topology-failure-high_contrast.png` | `d01dca4a0d316d146d926aa7ae56a6cdf4c4832b1f3d5891a9ad33c2889f6d2b` | `0bdb17810b1a923852d6b4ea670167600b2414e7d7d6cf670d7ae09a4fded127` |

## Execution

Each theme used the same focused command shape:

```text
TRUST_CAPTURE_THEME=<dark|light|high_contrast> \
TRUST_CAPTURE_SCREENSHOT=<theme-output.png> \
ST_LSP_TEST_SERVER=/home/johannes/projects/trust-platform-ads-windows-fix/target/debug/trust-lsp \
xvfb-run -a -s '-screen 0 1920x1080x24' \
node docs/internal/testing/evidence/vscode-ui-ux-acceptance/2026-06-25/runners/cdp_peer_topology_failure.js
```

All three runs reported `1 passing` in approximately 12 seconds.

## Honesty Boundaries

- This proves the rendered malformed-peer failure slice only. It does not prove
  discovery transport, a live remote peer, or the complete Devices &
  Connections workflow.
- The activation binary was an existing local `trust-lsp` with SHA-256
  `516a134009f502faf7eb10f9ca600e1e4a5cbe3aceb8864020db952ef0799e6e`.
  LSP semantics are not exercised by this webview-boundary journey.
- The result is provisional because implementer and evidence operator are the
  same agent. An independent reviewer must inspect the three captures and the
  runner before the journey may become `ux_accepted`.

---
name: vscode-capture
description: Drive and screenshot the real truST VS Code extension. Use for VS Code UI proof, webview/panel screenshots, command-driven extension smoke tests, and browser-visible extension verification.
---

# VS Code Capture

Use the saved capture harness when a claim must be proven against the real truST VS Code extension rather than inferred from unit tests.

## Workflow

1. Locate the runner directory:
   - `docs/internal/testing/evidence/vscode-ui-ux-acceptance/2026-06-25/runners/`
2. Read `references/capture-harness.md` for runner families and prerequisites.
3. Build or point to `target/debug/trust-lsp`, `target/debug/trust-debug`, and `target/debug/trust-runtime` as needed.
4. Run the smallest runner that exercises the visible surface.
5. Save the screenshot/artifact path with the acceptance evidence or final report.
6. If the runner directory is absent in a fresh checkout, state that capture proof is unavailable and do not claim visual verification.

## Rules

- Use command-driven runners for first-run, ST editing, Check/Run, Live Values, Debugging, and HMI.
- Use CDP runners for webview DOM interaction such as Devices & Connections forms and node inspectors.
- Use Xvfb and the cached `.vscode-test` version when available.
- Do not substitute static DOM assertions for screenshot proof when layout/visual correctness is the claim.

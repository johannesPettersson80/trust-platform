---
name: trust-vscode-quality
description: Builds and verifies truST's VS Code extension - commands, webviews, snippets, debug flows, Control Studio, HMI Builder, Devices & Connections and the test harness. Use for any change under editors/vscode, for extension tests, for real screenshots of the running extension, and for UI acceptance-board journeys and ux_accepted evidence.
---

# VS Code extension

**Hard rule (AGENTS.md): all logic is in Rust.** Logic is anything whose result is stored, sent to
a runtime or PLC, or decides an engineering or operator outcome. TypeScript only presents (drawing,
layout, mouse interaction, view state) and forwards requests to Rust. Never add logic in
TypeScript. Existing TypeScript logic is debt to move into Rust, not to extend.

The extension is presentation and wiring only. A change that needs logic adds it in Rust, as a
`trust-lsp` command or a runtime-protocol operation; the extension calls it and renders the answer.
The TypeScript logic in Control Studio and in the HMI Builder is debt, listed in AGENTS.md: do not
extend it.

## Building a change

Behaviour changes follow `trust-test-authoring`: the written specification first, then the native
test.

- **Surface**: find the surface you are changing: a command, snippet, debug flow, webview, runtime
  lifecycle, diagnostics or the test harness. Update the `package.json` contributions when commands,
  views or snippets change.
- **Behaviour**:
  - handle cancellation and conflicts;
  - a disabled action shows its reason in the UI;
  - generated files and config writes are deterministic and idempotent.
- **Look**: new webview chrome uses the shared theme tokens, not one-off colours. Do not add a second
  Start/Run surface when the existing one can be wired by project kind.
- **Tests**: tests go in `editors/vscode/src/test/suite/**`. Register each new file in
  `src/test/suite/index.ts`, then run `python3 scripts/check_vscode_test_registration.py`. Cover:
  - the happy path and the cancel path;
  - conflict prompts;
  - single-root and multi-root workspaces;
  - debug start and attach, when debug code changes;
  - the rendered state of webviews.

  A source-text assertion does not prove rendered behaviour.

## Running the tests

Run them on the builder (see `trust-remote-builder`).

- **Compile**: `npm run compile` in `editors/vscode`. Before a push, also run `npm run lint`.
- **Focused**: `TRUST_VSCODE_TEST_GREP='<suite title or regex>' npm test`. It needs an X display,
  `TMPDIR` under home, and the cargo lease. `ST_LSP_TEST_SERVER=<path>/trust-lsp` reuses a built
  language server.
- **Full**: `npm test`, once, in the final round.

## Screenshots of the real extension

A claim about what the extension shows needs a screenshot of the real extension.

- **Saved runners**: `docs/internal/testing/evidence/vscode-ui-ux-acceptance/2026-06-25/runners/`.
  This folder is local and ignored by git. If it is missing, say that capture proof is unavailable.
  There are two kinds:
  - **Command-driven runners** (`*-runner.js`) start a headless Extension Development Host with
    `@vscode/test-electron`. A mocha test drives `vscode.commands.executeCommand` and captures the
    Xvfb root. Use them for:
    - the first run;
    - ST editing;
    - Check/Run;
    - Live Values (`trust-lsp.debug.io.write|force|release` with `{address, value}`);
    - debugging;
    - the HMI.
  - **CDP runners** (`cdp_*.js`) add `--remote-debugging-port`. They drive the webview's inner iframe
    (`document.querySelector('iframe').contentDocument`) to click and read the React DOM. One example
    is the Devices & Connections graph, where they click a `.react-flow__node` by its text with
    PointerEvents.
- **Running a runner**: `xvfb-run -a -s "-screen 0 1920x1080x24" node <runner>.js`.
  - It needs `target/debug/trust-lsp`, `trust-debug` and `trust-runtime`.
  - It needs a cached `.vscode-test/vscode-linux-*`.
  - Live Values and the HMI need addressed I/O: a `Configuration.st` with
    `VAR_CONFIG … AT %QX0.0` (or `%QW0`, `%MW0`).
- **On the Pi**: Xvfb screenshots come out black unless you use the SwiftShader ANGLE flags and
  `import -window root`.
- **Review**: look at every screenshot yourself and judge it. Correct code is not a correct picture.

## UI acceptance board

- **Location**: the board and its evidence live in
  `docs/internal/testing/evidence/vscode-ui-ux-acceptance/`.
- **What a row names**:
  - its VS Code surface;
  - the journey, as a user story;
  - the preconditions and project kind;
  - the tests or runners;
  - the screenshot paths;
  - the theme coverage: light, dark and high contrast for core surfaces.
- **Acceptance**: a row reaches `ux_accepted` only with its evidence attached and a reviewer who is
  not the implementer. CLI-only proof never closes a visible row.

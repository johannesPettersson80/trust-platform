# truST VS Code Capture Harness

Use this reference with `vscode-capture`.

## Runner Directory

`docs/internal/testing/evidence/vscode-ui-ux-acceptance/2026-06-25/runners/`

This directory is local evidence storage and may be gitignored. If it is absent, report the gap.

## Command-Driven Runners

Files matching `*-runner.js` use `@vscode/test-electron` to launch a headless Extension Dev Host. A mocha test drives the extension through `vscode.commands.executeCommand` and screenshots the Xvfb root.

Use these for:

- First Run
- ST editing
- Check/Run
- Live Values, including `trust-lsp.debug.io.write|force|release` with `{address,value}`
- Debugging
- HMI

## CDP Runners

Files matching `cdp_*.js` launch with `--remote-debugging-port` and use Chrome DevTools Protocol to drive the webview inner iframe:

`document.querySelector('iframe').contentDocument`

Use these for Devices & Connections graph flows such as protocol picker, add-device forms, and node inspectors.

## Run Shape

```bash
xvfb-run -a -s "-screen 0 1920x1080x24" node <runner>.js
```

Prerequisites:

- `target/debug/trust-lsp`
- `target/debug/trust-debug`
- `target/debug/trust-runtime`
- cached `.vscode-test/vscode-linux-arm64-*` when running on the Pi

Live Values/HMI need addressed I/O through a `Configuration.st` `VAR_CONFIG` using `%I`, `%Q`, or `%M`.

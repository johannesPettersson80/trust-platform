# Automated Docs Captures

This directory is the source of truth for automated screenshot generation used by
the public docs.

## Capture classes

- Browser product surfaces: Playwright against live truST browser endpoints such
  as `/ide` and `/hmi`.
- Terminal captures: VHS tapes rendered to checked-in GIFs for CLI/runtime
  workflows that should show exact shell output.
- VS Code browser surfaces: Playwright against `code-server` with the real
  truST extension installed from a locally packaged VSIX.
- Native desktop VS Code: use the existing Wayland/X11 scripts only when the
  capture genuinely depends on native desktop debugger chrome or OS dialogs.

The code-server lane currently captures command-palette proof. The first
runtime-panel capture remains checked in as a skipped test until that path is
stable under code-server's chat-focused shell.

## Current entry points

```bash
./scripts/captures/run-playwright-captures.sh browser
./scripts/captures/run-playwright-captures.sh vscode
./scripts/captures/run-terminal-captures.sh
```

These commands write checked-in assets directly to:

- `docs/public/assets/images/browser/`
- `docs/public/assets/images/terminal/`
- `docs/public/assets/images/vscode/`

Operator HMI pages (overview, daily checks, alarm, shift handover) are captured
by `scripts/captures/browser/hmi-operator-pages.spec.mjs`, which runs as part of
the `browser` mode. It navigates the live `/hmi` sidebar to Overview, Alarms,
and Trends and captures each page in dark mode with live values.

## Local prerequisites

- `cargo` with locally built binaries under `target/debug/`
- `docker` for the `code-server` capture path
- Node 20+
- `npm --prefix scripts/captures ci`
- `npm --prefix editors/vscode ci && npm --prefix editors/vscode run compile` for
  the VS Code capture path

## CI shape

- Browser captures run on GitHub-hosted Linux runners with Playwright.
- Terminal captures run on GitHub-hosted Linux runners with `vhs` inside its
  published container image.
- VS Code captures run on GitHub-hosted Linux runners with Playwright plus a
  disposable `code-server` Docker container.
- The checked-in completeness contract lives in
  `docs/public/assets/capture-inventory.json`.
- That inventory only enforces the subset of captures that are currently
  automated. A green check there means "automated assets are present," not
  "the docs screenshot backlog is complete."

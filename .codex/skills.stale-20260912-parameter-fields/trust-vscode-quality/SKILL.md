---
name: trust-vscode-quality
description: Implement and verify trust-platform VS Code extension quality gates. Use when adding commands, snippets, webviews, debug flows, extension tests, or CI wiring under editors/vscode.
---

# Trust VS Code Quality

Use this workflow for user-visible work under `editors/vscode`.
Use `trust-test-authoring` first for planner, catalog, invariant, oracle, and proof routing; this
skill adds the extension registration and end-to-end requirements.

## Implementation Workflow

1. Identify the surface: command contribution, snippet, debug flow, webview, runtime lifecycle, diagnostics, or test harness.
2. Define one observable behavior slice for the new feature, bug fix, or intentional behavior change.
3. Add or update the smallest focused test under `editors/vscode/src/test/suite/**`, register every new test file in `editors/vscode/src/test/suite/index.ts`, and run it before implementation.
4. Confirm the test reaches the expected behavior assertion and is red because the behavior is missing or wrong. Harness, compile, dependency, registration, timeout, and unrelated failures do not count.
5. Update `editors/vscode/package.json` contributions when commands/views/snippets change, then implement only the minimum behavior in `editors/vscode/src/**` with explicit cancellation, conflict, and disabled-with-reason handling.
6. Rerun the same focused test until green before starting another behavior slice.
7. Keep generated files and config writes deterministic and idempotent.

## Validation

- `cd editors/vscode && npm run lint`
- `cd editors/vscode && npm run compile`
- `cd editors/vscode && npm test` for user-visible VS Code changes.
- Prefer `ST_LSP_TEST_SERVER=<path>/trust-lsp npm test` when a known-good warmed server exists.
- Do not claim success for VS Code changes from compile/lint or backend cargo tests alone.
- For browser-visible webview/frontend changes, first use a focused rendered interaction, state, or layout test; static source-text checks alone do not prove the behavior. After it is green, verify the live rendered surface with Playwright or the saved capture harness, not Puppeteer MCP.
- Report the focused test's expected red result and the same test's green result.
- In trust-platform checkouts on a Raspberry Pi or other slow local host, use the remote builder for broad/full validation first and ask before broad local Rust gates.

## Reference

Read `references/vscode-testing.md` when you need concrete extension test commands or harness details.

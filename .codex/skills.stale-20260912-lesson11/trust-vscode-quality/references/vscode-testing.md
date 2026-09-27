# VS Code Extension Testing Notes

Use this reference with `trust-vscode-quality`.

## Files Commonly Touched

- `editors/vscode/package.json`
- `editors/vscode/src/extension.ts`
- `editors/vscode/src/test/suite/**/*.test.ts`
- `editors/vscode/snippets/st.code-snippets`

## Integration Test Focus

- Command happy-path and cancel-path behavior.
- Explicit conflict prompts and confirmation behavior.
- Single-root and multi-root workspace behavior.
- Debug start/attach regressions when debug code is touched.
- Webview rendering and disabled-with-reason honesty.

## Harness Pattern

- Build `trust-lsp`.
- Export `ST_LSP_TEST_SERVER` to that binary.
- Run `npm test` from `editors/vscode`.

## Visual Proof

For visible webview/panel changes, use the repo `vscode-capture` skill or a Playwright script against the live surface. Record the exact runner and screenshot/artifact path.

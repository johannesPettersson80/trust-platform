# Browser Analysis WASM Demo on GitHub Pages

This guide publishes the static browser demo in `docs/demo/` for client-facing
review.

## What gets deployed

- Static entrypoint: `docs/demo/index.html`
- Monaco bundle: `docs/demo/assets/ide-monaco.20260215.js`
- WASM worker + engine: `docs/demo/wasm/`
- Demo behavior (all 7 LSP features): `docs/demo/demo.js`

No backend runtime is required; the analyzer runs fully in-browser via
WebAssembly.

Language intelligence shown in the demo (diagnostics, hover, completion,
definition, references, highlights, rename) is sourced directly from the
WASM analyzer/LSP pipeline only. The demo does not synthesize local fallback
results when WASM requests fail or time out.

## Local verification

1. Rebuild/copy demo assets:
   `scripts/build_demo.sh`
2. Serve a local GitHub Pages replica (`/<repo>/` path prefix + no-cache):
   `scripts/run_demo_local_replica.sh --port 4175 --repo trust-platform`
3. Open:
   `http://127.0.0.1:4175/trust-platform/`

Alternative quick static host:
- `python3 -m http.server 8000 -d docs/demo`
- `http://127.0.0.1:8000/`

### Manual smoke checks (strict WASM/LSP path)

1. Diagnostics:
   introduce a temporary typo in `program.st`; expected squiggle appears, and status bar diagnostic count increases.
2. Hover:
   open `fb_pump.st` and hover `FB_Pump`; expected signature includes `Command : ST_PumpCommand;` and `Status : ST_PumpStatus;` (no `?` placeholders).
3. Completion:
   in `fb_pump.st`, type after `Status.` and press `Ctrl+Space`; expected members include `State`, `Running`, and `ActualSpeed`.
4. Go to definition:
   in `fb_pump.st`, `Ctrl+Left-click` `E_PumpState`; expected jump to `types.st` enum declaration.
5. Go to references:
   in `types.st`, first left-click `Enable`, then right-click `Enable` and choose `Go to References` (or press `Shift+F12`); expected references include `fb_pump.st` and `program.st`.
6. Document highlights:
   click `ramp` in `fb_pump.st`; expected all occurrences in that file are highlighted.
7. Rename:
   in `types.st`, first left-click `ActualSpeed`, then press `F2` (or `Fn+F2`), rename to `ActualSpeedRpm`; expected edits across files.

## GitHub Pages deployment

Workflow: `.github/workflows/demo-pages.yml`

- Triggered on pushes to `main` that modify `docs/demo/**`.
- Can also be started manually with **workflow_dispatch**.
- Uploads `docs/demo/` as the Pages artifact and deploys to
  the `github-pages` environment.
- The workflow uses `actions/configure-pages@v5` with `enablement: true` to
  auto-enable Pages for first-time repos.

If you still see `Get Pages site failed`:
1. Open repository settings -> **Pages**.
2. Ensure build/deploy source is **GitHub Actions**.
3. Re-run the **Demo Pages** workflow.

For local debugging of cursor-anchor retries, add `?debug_lsp=1` to the URL and
watch browser console logs prefixed with `[demo:lsp]`.

After the workflow succeeds, the demo URL is:

- `https://<org-or-user>.github.io/<repo>/`

For this repository that is typically:

- `https://johannespettersson80.github.io/trust-platform/`

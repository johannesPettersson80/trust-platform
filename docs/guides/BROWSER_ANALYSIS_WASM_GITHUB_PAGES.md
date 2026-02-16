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

## Local verification

1. Rebuild/copy demo assets:
   `scripts/build_demo.sh`
2. Serve the static directory:
   `python3 -m http.server 8000 -d docs/demo`
3. Open:
   `http://localhost:8000/`

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

After the workflow succeeds, the demo URL is:

- `https://<org-or-user>.github.io/<repo>/`

For this repository that is typically:

- `https://johannespettersson80.github.io/trust-platform/`

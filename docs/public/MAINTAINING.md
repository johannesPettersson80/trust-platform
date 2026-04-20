# Maintaining The Public Docs

## Rules

- add public pages under `docs/public/`
- keep internal plans, reports, and spike notes out of the public nav
- generated public screenshots and stills belong under `docs/public/assets/`; source captures live in `docs/internal/assets/`, `editors/vscode/assets/`, or example media directories
- do not create a second public entry point in `README.md`, `docs/README.md`, or `examples/README.md`; those files should point back to this site

## Before you add a page

1. Decide which user question it answers.
2. Put it under `start`, `develop`, `connect`, `operate`, `reference`, `concepts`, or `examples`.
3. Add it to `mkdocs.yml`.
4. Link it from the nearest section index.
5. If it needs a screenshot, make that screenshot come from an automated source or generated-media script, not a hand-dropped mystery file.

## Build checks

```bash
python scripts/generate_public_docs_media.py
python scripts/check_public_docs_links.py
python scripts/check_example_catalog_links.py
mkdocs build --strict
python scripts/check_public_docs_assets.py
python scripts/check_public_docs_search.py
```

## Publishing

GitHub Pages is built from the docs workflow on pushes to `main`.
That workflow must continue to watch public page files plus any included
source-of-truth inputs such as `docs/guides/**`, `docs/specs/**`,
`conformance/**`, and media source directories.

## Generated screenshots

- Browser product-surface screenshots come from `scripts/captures/browser/*.spec.mjs`.
- Terminal captures come from `scripts/captures/terminal/*.tape`.
- Code-server-based VS Code screenshots come from `scripts/captures/vscode/*.spec.mjs`.
- The current code-server proof capture is `scripts/captures/vscode/workspace-shell.spec.mjs`; the runtime-panel command-palette capture stays disabled until that interaction path is stable under code-server.
- Use `scripts/captures/README.md` as the source of truth for capture classes, local prerequisites, and CI expectations.
- Legacy desktop VS Code screenshots still come from `scripts/capture-readme-screenshots-auto.sh` when a capture truly requires native desktop VS Code rather than code-server.
- Visual-editor docs screenshots come from `scripts/capture-public-docs-visual-editors.sh`.
- Public-doc screenshots/stills are synchronized into `docs/public/assets/images/` by `scripts/generate_public_docs_media.py`.
- When a page needs a new public screenshot, add the source capture to an automated path first, then regenerate the public asset.

## Capture commands

```bash
python scripts/generate_public_docs_media.py --regenerate-browser-captures
python scripts/generate_public_docs_media.py --regenerate-terminal-captures
python scripts/generate_public_docs_media.py --regenerate-vscode-captures
```

The capture inventory lives in `docs/public/assets/capture-inventory.json`.
If an inventory entry is missing from the checked-in assets, `python scripts/check_public_docs_assets.py`
fails the docs build.
That inventory only covers the assets currently automated; it is not a claim
that the full screenshot backlog is complete.

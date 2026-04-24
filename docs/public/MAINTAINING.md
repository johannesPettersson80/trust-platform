# Maintaining The Public Docs

This page is for contributors who edit the docs site. Product readers should
start from [Start](start/index.md), [Concepts](concepts/index.md), or
[Migrate](migrate/index.md).

## Rules

- add public pages under `docs/public/`
- keep internal plans, reports, and spike notes out of the public nav
- generated public screenshots and stills belong under `docs/public/assets/`; source captures live in `docs/internal/assets/`, `editors/vscode/assets/`, or example media directories
- do not create a second public entry point in `README.md`, `docs/README.md`, or `examples/README.md`; those files should point back to this site
- if a page is promoted in body prose, include it in the nearest section nav
- if a page is linked from a section index, include it in that section's nav unless the index explicitly marks it as a cross-section reference
- snippet includes must not introduce a second `#` page title inside a public wrapper page; use PyMdown line ranges or sections to skip included titles
- UI-facing guides should include a maintained screenshot, GIF, diagram, or explicit reason why a visual is not useful
- procedural pages should end with a visible success state, command, or verification step
- public product claims should link to proof: tests, benchmarks, examples, screenshots, source, or an honest limits section

## Before you add a page

1. Decide which user question it answers.
2. Put it under `start`, `develop`, `connect`, `operate`, `reference`, `concepts`, or `examples`.
3. Add it to `mkdocs.yml`.
4. Link it from the nearest section index.
5. If it needs a screenshot, make that screenshot come from an automated source or generated-media script, not a hand-dropped mystery file.

## Wrapper page pattern

Snippet-backed pages still need their own reader value before the include. Use
this shape unless the page is a pure generated reference:

1. one sentence saying who the page is for
2. one short paragraph saying what the included guide/example teaches
3. time/prerequisite guidance when the page is an example or tutorial route
4. a visible success state or verification condition
5. related links that send the reader to the next decision, not a duplicate page

## Build checks

```bash
python scripts/generate_public_docs_media.py
python scripts/check_public_docs_ia.py
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

- Browser product-surface screenshots come from `scripts/captures/browser/*.spec.mjs`, captured live against a running `trust-runtime`. Operator overview, daily-checks, alarm, and shift-handover visuals come from `scripts/captures/browser/hmi-operator-pages.spec.mjs`.
- Terminal captures come from `scripts/captures/terminal/*.tape`.
- Code-server-based VS Code screenshots come from `scripts/captures/vscode/*.spec.mjs`.
- The current code-server proof capture is `scripts/captures/vscode/workspace-shell.spec.mjs`; the runtime-panel command-palette capture stays disabled until that interaction path is stable under code-server.
- Do not ship fabricated composites (ImageMagick text or UI overlays on an existing screenshot) as product proof. If a capture cannot run, use a labeled diagram or skip the figure instead of pretending the composited image is a real product surface.
- Use `scripts/captures/README.md` as the source of truth for capture classes, local prerequisites, and CI expectations.
- Legacy desktop VS Code screenshots still come from `scripts/capture-readme-screenshots-auto.sh` when a capture truly requires native desktop VS Code rather than code-server.
- Visual-editor docs currently use maintained diagrams unless a screenshot
  capture proves the custom editor rendered before the image was taken. Do not
  publish empty editors, raw JSON views, or placeholder panes as visual-editor
  proof.
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

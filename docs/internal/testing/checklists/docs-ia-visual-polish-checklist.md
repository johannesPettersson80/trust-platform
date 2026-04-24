# Docs IA, Visual Polish, And Gate Checklist

Scope: finish the documentation restructuring and proof work after the
`One Project`, `truST Mesh`, and surface-tour updates.

## Acceptance

- public docs navigation follows the user journey and persona model
- migration pages are exposed without breaking canonical URLs
- visual pages include maintained screenshots, GIFs, or diagrams where useful
- thin snippet-wrapper pages include original framing: who the page is for,
  what the included guide teaches, what success looks like, and where to go next
- docs maintenance rules are backed by automated checks where practical
- strategic docs decisions are written down as reviewable hypotheses
- public docs checks pass

## P2 - Information Architecture

- [x] 6. Operate persona restructure: sidebar and index grouped by Engineering Run Loop, Target Administration, Operator And Technician, and Fleet And Delivery.
- [x] 7. Concepts index split into Core Mental Models and Deep Dives.
- [x] 8. Connect index surfaces truST Mesh.
- [x] 9a. Reference index duplicate lists collapsed.
- [x] 9b. Specifications grouped by concern.
- [x] 9c. `14-sfc-profile` numbering collision resolved by moving SFC to a non-numbered `sfc-profile` slug.
- [x] 10. Examples index expanded from a plain category table into a journey-oriented page.
- [x] 11. Start section keeps Choose Your Workflow at position 3.

## P2.5 - Migration File Move Decision

- [x] 12. Migration ecosystem pages moved to canonical `migrate/*` URLs. The
  old `develop/interoperability/*` paths remain as compatibility pages that
  point to the new migration homes, so existing links resolve without requiring
  a redirects plugin.

## P3 - Visual And Polish

- [x] 13. Ladder/SFC/Blockly pages use accurate diagrams instead of misleading
  custom-editor screenshots until the capture pipeline can prove the editors
  rendered before the screenshot is taken.
- [x] 14. AI Assistance includes a user-facing tool-flow visual.
- [x] 15. Homepage static screenshot wall is trimmed after the surface-tour GIF.
- [x] 16. Migrate page includes a compatibility matrix.
- [x] 17. Operator runbooks include visuals where they help the user understand the surface.

## P4 - Lints And Gates

- [x] 18. Add lint for public pages linked from body prose but absent from nav.
- [x] 19. Add lint for snippet H1 collisions.
- [x] 20. Add lint for missing blank line before Markdown lists.
- [x] 21. Analytics decision recorded in `docs/internal/testing/checklists/docs-ia-strategy.md`.

## P5 - Strategy

- [x] 22. Success metrics defined for docs evaluation in `docs/internal/testing/checklists/docs-ia-strategy.md`.
- [x] 23. Docs versioning strategy for 1.0 recorded in `docs/internal/testing/checklists/docs-ia-strategy.md`.
- [x] 24. A11y and mobile pass expectations recorded in `docs/internal/testing/checklists/docs-ia-strategy.md`.

## P6 - Full Text Review

- [x] 25. Inventory all public Markdown pages and snippet-backed source files.
- [x] 26. Expand high-traffic thin wrappers with original reader framing.
- [x] 27. Split HMI authoring and operation pages so they no longer render the
  same guide body.
- [x] 28. Expand `First Project` into a concrete shipped-tutorial walkthrough.
- [x] 29. Clarify Troubleshooting vs FAQ scope.
- [x] 30. Platform-qualify command-palette shortcut language.
- [x] 31. Add jargon notes to concept pages that introduce PLC/runtime terms.
- [x] 32. Promote Learning Paths into the Start navigation path.
- [x] 33. Add explicit verification or success states to short procedural pages
  that previously ended at related links.
- [x] 34. Expand thin top-level/reference wrappers so snippet-backed pages still
  orient users before rendering included source material.

## P7 - 10/10 Polish Pass

- [x] 35. Replace the homepage's competing click paths with one reader-journey
  visual and a single Choose Your Path table.
- [x] 36. Add a terms box to `concepts/one-project.md`.
- [x] 37. Expand example category pages with outcome, time, prerequisites, and
  success guidance.
- [x] 38. Expand OSCAT and PLCopen Motion library wrappers with expected
  outcomes and boundaries.
- [x] 39. Add success criteria to Connect wrapper pages.
- [x] 40. Split public Project nav into product help pages and a Contributors
  subgroup.
- [x] 41. Keep migration compatibility stubs for old URLs, but hide them from
  MkDocs "not in nav" warnings with `not_in_nav`.
- [x] 42. Add distinct maintained operator visuals for guide, daily checks,
  alarm response, and shift handover.
- [x] 43. Add a VS Code AI tools visual and an AI capability matrix to
  `develop/ai-assistance.md`.
- [x] 44. Codify the snippet-wrapper framing pattern in `MAINTAINING.md`.
- [x] 45. Recheck snippet-backed public pages and eliminate remaining
  under-framed wrappers outside compatibility stubs and generated spec wrappers.

## P8 - Real-Surface Proof Pass

- [x] 46. Add a first-figure VS Code AI diagnostics tool-call PNG to
  `develop/ai-assistance.md`, derived from the checked-in code-server workspace
  capture and the `trust_get_diagnostics` LM tool contract.
- [x] 47. Add first-figure Browser HMI overview proof to
  `operate/operator-guide.md`.
- [x] 48. Add first-figure Browser HMI daily-check proof to
  `operate/operator-daily-checks.md`.
- [x] 49. Add first-figure Browser HMI alarm-page proof to
  `operate/operator-alarm-handbook.md`.
- [x] 50. Add first-figure Browser HMI shift-handover proof to
  `operate/operator-shift-handover.md`.
- [x] 51. Add a reproducible proof-image generator and inventory entries for
  the five new PNG assets.
- [x] 52. Record capture limitation: this sandbox cannot bind the runtime
  control endpoint or launch Chromium, so the new proof PNGs are derived from
  checked-in product screenshots and marked as derived in the inventory.

## Validation Log

- [x] `scripts/captures/generate-public-docs-proof-images.sh`
- [x] `bash -n scripts/captures/generate-public-docs-proof-images.sh`
- [x] `python scripts/check_public_docs_ia.py`
- [x] `python scripts/check_public_docs_links.py`
- [x] `python scripts/check_public_docs_assets.py`
- [x] `python scripts/check_public_docs_search.py`
- [x] `python scripts/check_example_catalog_links.py`
- [x] nav target existence check
- [x] `git diff --check`
- [x] `.venv-docs/bin/mkdocs build --strict`
- [x] `python -m py_compile` for the public-doc helper scripts
- [x] `just fmt`
- [x] `just clippy`
- [ ] `just test-all` - blocked in this sandbox: default run fails at
  `sccache` with `Operation not permitted`; rerun with `RUSTC_WRAPPER=` reaches
  `trust-lsp` inline-value tests and fails when the tests try to bind a local
  control stub socket with `PermissionDenied`.

# Public Docs And Specs Remediation Checklist

Merged work order for docs/public prose quality, docs site branding, README cleanup,
spec integrity, and docs-capture CI stability.

## Priority Order

### P0

- [ ] Fix the `Docs Captures` CI blocker for code-server on GitHub runners.
- [x] Fix spec text that currently contradicts implementation: `1.1`, `1.3`,
      `1.4`, `1.5`, `1.6`, `1.7`, `1.8`.
- [x] Split `docs/specs/10-runtime-semantics.md` and fix broken numbering/link ownership.
- [x] Fix duplicate `## 7` numbering in `03-variables.md`.
- [x] Add missing `JMP` chapter in `06-statements.md`.
- [x] Add `POINTER TO` section to `02-data-types.md`.
- [x] Move misplaced sections to the correct specs.
- [x] Apply global docs/public prose sweeps `G1`, `G2`, `G3`, `G6`.
- [x] Rewrite `docs/public/index.md` "What truST Replaces".
- [x] Wire docs-site branding: logo, favicon, and teal palette.

### P1

- [x] Fix spec issues `1.2`, `1.9`, `1.10`.
- [x] Stop `14-lsp.md` from restating lexer/parser/type-system specs.
- [x] Decide and execute the role of `09-semantic-rules.md`.
- [x] Collapse diagnostic-code ownership to one spec.
- [x] Consolidate duplicate call-statement chapters in `06-statements.md`.
- [x] Fold `Statement Sequences` into `06-statements.md` overview.
- [x] Standardize or remove per-spec "Implementation Notes" appendices.
- [x] Move spec extension rationale into `docs/IEC_DEVIATIONS.md`.
- [x] Apply public-doc prose sweeps `G4`, `G5`, `G8`, `G9`, `G10`, `G11`.
- [x] Fix high-priority README issues.
- [x] Apply key per-file rewrites in docs/public.

### P2

- [ ] Normalize bullet/list style across docs/public.
- [x] Convert config/reference prose blocks into option tables.
- [x] Add master index tables to standard functions / FB specs.
- [x] Add precedence anchors in `05-expressions.md`.
- [ ] Convert non-normative code blocks to tables where required.

### P3

- [x] Create missing specs: `18-configurations-resources-tasks.md`,
      `19-project-model.md`, `20-agent-api-v1.md`, `21-harness-protocol.md`,
      and `sfc-profile.md` or an explicit retirement decision.
- [x] Expand or retire specification stubs under
      `docs/public/reference/specifications/01...13`.
- [x] Populate or delete `operate/operator-alarm-handbook.md`.
- [ ] Consolidate thin examples index pages into a smaller set.
- [ ] Optional brand polish: proper compact mark.

## 0. CI / Capture Blocker

- [ ] `C0.1` Fix `Docs Captures` CI for code-server on GitHub runners.
      Files: `scripts/captures/vscode/start-code-server.sh`,
      `.github/workflows/docs-captures.yml`
      Problem: container user cannot write repo-mounted
      `scripts/captures/.cache/...` dirs on GitHub runners.
      Acceptance: `Docs Captures` passes on `main`; no `EACCES` for
      `code-server-user-data` or `code-server-extensions`.

## 1. Spec Text That Lies Today

- [x] `1.1` `docs/specs/10-runtime-semantics.md:134`
      Rewrite `POINTER TO` as a supported non-IEC extension; keep `Value::Null`
      wording accurate; add `IEC_DEVIATIONS` entry.
- [x] `1.2` `docs/specs/06-statements.md:22`
      Replace "full control-flow validation is not implemented" with a precise
      description of current reachability checks and missing full CFG work.
- [x] `1.3` `docs/specs/01-lexical-elements.md:160`
      Replace "SFC syntax/semantics are out-of-scope" with actual shipped scope:
      reserved keywords supported, visual editor shipped, textual SFC body
      syntax still out-of-scope.
- [x] `1.4` `docs/specs/03-variables.md:170`
      Delete `PROTECTED behaves like PRIVATE` fallback; inheritance exists.
- [x] `1.5` `docs/specs/10-runtime-semantics.md:3780`
      Replace "single-file reloads" wording with per-resource hot reload scope.
- [x] `1.6` `docs/specs/10-runtime-semantics.md:3781`
      Replace "inputs only" with input/output forcing support, citing DAP path.
- [x] `1.7` `docs/specs/10-runtime-semantics.md:2130`
      Remove or rewrite obsolete bytecode v1.0 compatibility note.
- [x] `1.8` `docs/specs/08-standard-function-blocks.md:498-508`
      Split claim clearly: `trust-hir` does not model internal state;
      `trust-runtime` executes full stateful behavior with documented
      deviations.
- [x] `1.9` `docs/specs/07-standard-functions.md:457-468`
      Add `DEV-ASSERT` entry for `ASSERT_TRUE` / `ASSERT_FALSE`.
- [x] `1.10` `docs/specs/coverage/standard-functions-coverage.md:10`
      Remove or automate stale "As of ..." dating.

## 2. Spec Structural Problems

- [x] `2.1` Split `docs/specs/10-runtime-semantics.md` into:
      - `10-runtime-semantics.md`
      - `11-runtime-engine.md`
      - `12-bytecode.md`
      - `13-debug-adapter.md`
      - `14-lsp.md`
      Then renumber current `11/12/13` specs to `15/16/17`, fix duplicate
      `§6.9`, and update all references.
- [x] `2.2` Remove lexer/parser/type-system duplication from the LSP spec and
      replace with links to `01-09`.
- [x] `2.3` Decide the role of `09-semantic-rules.md`:
      canonical diagnostics registry or delete/merge.
- [x] `2.4` Ensure diagnostic code tables live in exactly one spec.
- [x] `2.5` Fix duplicate section numbering in `03-variables.md`.
- [x] `2.6` Add missing `JMP` chapter to `06-statements.md`.
- [x] `2.7` Add `POINTER TO` section to `02-data-types.md`.
- [x] `2.8` Move misplaced sections:
      - debugger visibility from `03` -> `13-debug-adapter`
      - debugger safe points from `06` -> `13-debug-adapter`
      - string operations from `02` -> `07-standard-functions`
      - EN/ENO section into `04` function chapter
      - PLCopen LD interop duplication removed from LSP appendix
- [x] `2.9` Consolidate function/FB/method call sections in `06-statements.md`.
- [x] `2.10` Fold `Statement Sequences` orphan section into overview.
- [x] `2.11` Standardize "Implementation Notes" to zero or one per spec.
- [x] `2.12` Move extension rationale into `docs/IEC_DEVIATIONS.md`.

## 3. Spec Per-File Follow-Up

- [ ] `01-lexical-elements.md`
      Flatten keyword nesting; fix SFC note; add or cross-ref lexer diagnostics.
- [ ] `02-data-types.md`
      Add `POINTER TO`; move string operations out; clarify type table headers.
- [ ] `03-variables.md`
      Fix numbering; move debugger section; deduplicate access-specifier prose;
      add cross-refs for direct variables.
- [x] `04-pou-declarations.md`
      Move EN/ENO; keep access-specifier rules here; reduce duplicated
      rationale for test POUs.
- [ ] `05-expressions.md`
      Consolidate implementation notes; move Siemens SCL prefix rationale to
      deviations; add precedence anchors; reduce duplicate type-check prose.
- [x] `06-statements.md`
      Add `JMP`; move debugger content out; consolidate call sections; fold
      statement-sequence orphan.
- [x] `07-standard-functions.md`
      Add top index table; move extensions cross-ref up; absorb string
      functions; compress repetitive per-function prose into tables.
- [x] `08-standard-function-blocks.md`
      Add top index table; rewrite internal-state section; add deviation
      cross-ref block.
- [x] `09-semantic-rules.md`
      Execute chosen ownership model; move diagnostic severity section if kept.
- [ ] `10-runtime-semantics.md`
      Preserve normative runtime semantics content after split.
- [ ] `11-runtime-engine.md`
      Apply hot-reload/output-forcing fixes; convert tables; fix duplicate
      numbering.
- [ ] `12-bytecode.md`
      Apply v1.0 note fix; verify reserved opcode wording; add cross-ref back
      to runtime-engine overview.
- [x] `13-debug-adapter.md`
      Absorb debugger content moved from `03` and `06`.
- [x] `14-lsp.md`
      Shrink duplicated language sections; own diagnostics if selected.
- [ ] `15-ladder-diagram.md`
      Add LD-specific diagnostics table and stronger cross-links.
- [ ] `16-ladder-profile-trust.md`
      Own PLCopen LD interoperability content.
- [ ] `17-visual-editors-runtime-unification.md`
      Expand into a real cross-editor profile or merge into ladder-profile.

## 4. Missing Specs To Create

- [x] `18-configurations-resources-tasks.md`
- [x] `19-project-model.md`
- [x] `20-agent-api-v1.md`
- [x] `21-harness-protocol.md`
- [x] `sfc-profile.md` or explicit SFC retirement/limitation decision.
- [x] Rewrite `docs/specs/README.md` as a top-level ownership index.

## 5. Public Docs Global Prose Sweep

- [x] `G1` Delete all `Use this page when...` / `Read this when...` /
      `Start here when...` openers from `docs/public/`.
- [x] `G2` Remove filler `real ...` intensifiers.
- [x] `G3` Delete meta-sentences about what a page is not; remove
      `Do Not Start Here`, `When Not To Use`, and screenshot-removal excuses.
- [x] `G4` Remove self-congratulatory and marketing filler such as `honest`,
      `truthful`, `canonical`, `docs-first`, `workflow-first`.
      Review other adjectives case by case instead of blind grep deletion.
- [x] `G5` Replace `surface` as a UI noun with `page`, `panel`, `UI`, or URL.
- [x] `G6` Replace internal headings like `First Success Workflow`,
      `Good first proof`, `Honest Current-State Workflow`.
- [ ] `G7` Normalize bullet capitalization/list style per-list.
- [x] `G8` Remove weak hedges like `today`, `currently`, `typically`, `for now`
      where they add nothing.
- [x] `G9` Remove condescending phrasings.
- [x] `G10` Name vendors explicitly instead of euphemisms.
- [x] `G11` Delete meta-headings like `What this covers`, `How to use this
      section`, `Why This Exists`.

## 6. Public Docs Per-File Fixes

- [x] `docs/public/index.md`
      Rewrite `What truST Replaces`; split long hero sentence; rename
      `What You Can Do With It` to `Capabilities`; trim production warning.
- [x] `docs/public/about.md`
      Remove duplicate intro; rewrite support model without hedges; delete or
      compress production-users section; make beta cautions concrete.
- [x] `docs/public/faq.md`
      Remove meta opener; rewrite differentiators concretely; rewrite hardware
      answer without hedges.
- [x] `docs/public/start/installation.md`
      Lead with Marketplace install + Release download; keep Cargo/source build
      in `install-from-source.md`.
- [x] `docs/public/start/program-in-vscode.md`
      Rename filler headings; remove `truthful` / `real shipped tutorial`
      language.
- [x] `docs/public/start/create-new-project.md`
      Rename filler headings; rewrite tutorial shortcut section.
- [x] `docs/public/start/maintain-an-existing-project.md`
      Rename filler headings; replace `Do Not Start Here` with direct guidance.
- [x] `docs/public/start/operate-in-browser.md`
      Rename `What Acknowledge Does And Does Not Mean`; rename
      `What Not To Click Blindly`.
- [x] `docs/public/operate/operator-alarm-handbook.md`
      Populate or delete.
- [x] `docs/public/operate/lifecycle.md`
      Remove duplicate content already owned by `operate/index.md`.
- [x] `docs/public/operate/install-on-target.md`
      Remove placeholder wording unless `.deb` / Pi packaging exists.
- [x] `docs/public/reference/hardware-compatibility.md`
      Replace vague risk prose with direct validation guidance.
- [x] `docs/public/reference/benchmarks.md`
      Add actual numbers/ranges or link to benchmark results.
- [x] `docs/public/reference/agent-api/overview.md`
      Replace meta headings and `Surface map`.
- [x] `docs/public/reference/cli/trust-lsp.md`
      Replace meta headings.
- [x] `docs/public/examples/index.md`
      Delete usage framing and redundant heading.
- [x] `docs/public/examples/runbooks.md`
      Delete `Why This Exists`; rewrite `real plant` phrasing.

## 7. Public Docs Config Reference Tables

- [x] Convert rule prose blocks in `reference/config/runtime-toml.md` into
      tables for control auth, TLS, mesh, observability, and OPC-UA.
- [x] Convert `reference/config/hmi-directory.md` write-policy prose into a
      table.
- [x] Convert `reference/config/trust-lsp-toml.md` stdlib-forms prose into a
      table.
- [x] Audit every `reference/config/*.md` file so top-level options are
      documented with tables.

## 8. Public Docs Structural Cleanup

- [x] `S1` Specification stubs under
      `docs/public/reference/specifications/01...13`
      Decision for now: keep them and expand into short summaries with a full
      spec link. Revisit after the spec split lands.
- [x] `S2` Remove screenshot-removal explanation lines from visual-editor
      pages.
- [x] `S3` Populate or delete `operate/operator-alarm-handbook.md`.
- [ ] `S4` Remove duplicated sibling-page framing from runtime-to-runtime,
      interoperability, and specification clusters.
- [ ] `S5` Consolidate thin examples index pages into a smaller set; keep only
      pages with unique content beyond links.

## 9. Docs Branding And README

### Brand wiring

- [x] `B1` Add docs-site logo and favicon to `mkdocs.yml`.
- [x] `B2` Create square favicon assets:
      `docs/public/assets/images/brand/trust-favicon.png` and `.svg`.
- [x] `B3` Add branded social preview image:
      `docs/public/assets/images/brand/trust-og.png` and corresponding config.
- [x] `B4` Make docs-site palette match truST teal.
- [x] `B5` Add the wordmark to `docs/public/index.md`.

### README cleanup

- [x] `R1` Shorten the README title.
- [x] `R2` Remove duplicate docs URL mentions.
- [x] `R3` Add Marketplace and Release badges.
- [x] `R4` Verify or replace the Rust/MSRV badge wording.
- [x] `R5` Trim workflow list to 3 primary paths.
- [x] `R6` Decide whether README owns screenshots or points to the docs site.
- [x] `R7` Remove or verify time-to-value claims.
- [x] `R8` Rename filler headings like `Best Features`.
- [x] `R9` Simplify config explanation after the TOML sample.
- [x] `R10` Replace the mini sitemap with a short docs block.
- [x] `R11` Delete self-referential docs praise.
- [x] `R12` Shorten status language.
- [x] `R13` Reduce README length to around `<=100` lines if the docs site is
      the real entry point.

## Acceptance

- [ ] `Docs Captures` is green on `main`.
- [x] `mkdocs build` is clean with no broken cross-refs.
- [x] `grep -rn "10-runtime.md" docs/` returns zero.
- [x] `grep -rnE "^(Use (this|these)|Read this|Start here|Open this) (page|section|path|when)" docs/public/`
      returns zero.
- [x] `grep -rniE "\\b(real|honest|truthful|canonical|docs-first|workflow-first)\\b" docs/public/`
      returns only intentional exceptions.
- [x] `grep -rniE "\\bFirst Success (Workflow|Loop)\\b" docs/public/`
      returns zero.
- [x] `grep -rniE "(browser|operator|engineering|control|runtime)\\s+surface" docs/public/`
      returns zero.
- [x] Canonical diagnostic code rows live in `docs/specs/14-lsp.md`; other
      spec references are ownership/cross-reference tables rather than duplicate
      code registries.
- [x] Every extension mentioned in specs has a matching `DEV-*` entry.
- [x] Every `reference/config/*.md` top-level section has at least one options
      table.
- [ ] No spec file exceeds roughly `~1200` lines.
- [x] No duplicated section numbering remains inside any file.
- [x] `docs/specs/README.md` lists one clear `Owns` scope per spec.
- [x] `mkdocs.yml` contains `theme.logo` and `theme.favicon`.
- [ ] The live docs site shows truST branding instead of the default Material
      cube and default tab icon.
- [x] README top badge row includes Marketplace and Release.
- [x] README docs URL appears only once or twice max.
- [x] `grep -n "canonical docs entry now routes" README.md` returns nothing.

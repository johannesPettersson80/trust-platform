# AGENTS

Repo-specific instructions for Codex working on `truST`.

This is the canonical shared rulebook for repo agents. Claude imports this file
from `CLAUDE.md`; Codex reads it directly. Keep shared rules here or in
repo-local `.codex/skills/**`, not in tool-private memory.

## Branch and Worktree Bootstrap Rule (non-negotiable)

- As part of creating or starting work on any branch, worktree, clone,
  validation copy, temporary checkout, or remote-builder checkout, identify the
  primary checkout that holds the canonical agent files.
- Manually copy the root `AGENTS.md` and the complete `.codex/skills/**`
  directory from that primary checkout into the destination checkout. Never
  assume ignored files were transferred by Git, branch creation, worktree
  creation, cloning, rsync, or another synchronization step.
- Before editing, building, testing, committing, pushing, merging, or releasing,
  verify that the destination copies exist and match the canonical source, then
  read the destination `AGENTS.md` and every skill required for the task.
- Report the canonical source path, destination path, branch, HEAD, and copy
  verification result. If the source files are missing, stale, or cannot be
  verified, stop and restore them before continuing.
- Every sub-agent must independently verify its own checkout and must not edit
  until this bootstrap has passed.
- When switching branches inside an existing checkout, verify the files again.
  When creating a branch in a new worktree or checkout, always copy them
  manually.

## Test-First Development Rule (non-negotiable)

- Apply test-first development to every behavior-changing code task: new
  features, bug fixes, and intentional behavior changes. Do not write or change
  production code for a behavior before establishing the test for that
  behavior.
- Work one observable behavior slice at a time:
  1. Write the smallest focused automated test that expresses the requested
     behavior, and register it in the real test runner when registration is
     required.
  2. Run that test before implementation and confirm it fails for the expected
     reason: the behavior is missing or incorrect.
  3. Implement only the minimum production change needed for that behavior.
  4. Run the same test again. If it is red, fix the implementation and repeat;
     only a green result completes that behavior slice.
- A compile error, missing dependency, unregistered test, broken harness,
  timeout, or unrelated failure does not count as the required red result. Fix
  the test environment and rerun until the test reaches the expected behavior
  assertion.
- Do not add post-hoc tests to justify code that was already implemented. If
  production code exists before the required red result, establish an honest
  pre-fix baseline in an isolated checkout or revert only the unapproved change
  with the user's permission, then perform the red-green loop.
- For behavior-preserving refactors, first add or identify behavior-lock tests
  and confirm they are green before editing; keep them green throughout. If the
  refactor introduces any new behavior, use the red-green workflow for that
  behavior.
- Frontend and VS Code work follows the same rule. Use a rendered interaction,
  state, or layout test at the closest practical boundary; static source-text
  checks alone do not prove user-visible behavior. After the focused test is
  green, complete the required real rendered/browser verification separately.
- Record both commands and outcomes in the handoff: the expected red failure
  before implementation and the green result from the same focused test after
  implementation. Broad validation and release gates run after the focused
  red-green loop; they do not replace it.


## Codebase Orientation

- `crates/trust-syntax`: lexer/parser, rowan CST.
- `crates/trust-hir`: semantic model, type checking, and IEC rules.
- `crates/trust-ide`: diagnostics, completion, hover, references, and editor
  feature logic.
- `crates/trust-lsp`: LSP protocol boundary and command wiring.
- `crates/trust-dev`: developer/workbench CLI package.
- `crates/trust-plcopen`: PLCopen XML import/export helpers.
- `crates/trust-runtime-core`: portable runtime core/value/bytecode pieces.
- `crates/trust-runtime`: Linux host runtime, product CLI, IO/web/control
  surfaces.
- `editors/vscode`: VS Code extension.

Common conventions:

- Use `smol_str::SmolStr` for interned strings when the surrounding crate does.
- Use `rustc_hash::FxHashMap/FxHashSet` for hot internal maps/sets when already
  established locally.
- Use `thiserror` for library errors and `anyhow` for application/CLI errors.
- `unsafe_code = "forbid"` is expected for language/IDE/dev-tooling crates such
  as `trust-syntax`, `trust-hir`, `trust-ide`, `trust-lsp`, and `trust-dev`.
  Runtime/host and vendored low-level code may contain registered unsafe sites;
  those are governed by the unsafe register and board evidence, not by a
  workspace-wide unsafe ban.

When adding or changing tokens in `trust-syntax`, update the `TokenKind` enum,
the matching `SyntaxKind` conversion/table, docs/spec token coverage when
applicable, and lexer/parser tests or snapshots.

## Release Hygiene Rules

- For user-visible changes (runtime behavior, CLI flags/output, stdlib assertions, test harness behavior, tutorial/docs behavior), update `CHANGELOG.md` under `## [Unreleased]` before commit.
- Bump `[workspace.package].version` in `Cargo.toml` for release-notable changes unless explicitly told not to.
- When VS Code extension behavior changes, also bump `editors/vscode/package.json` and matching root version entries in `editors/vscode/package-lock.json`.
- Whenever `[workspace.package].version` changes, also synchronize `editors/vscode/package.json` and the matching root version fields in `editors/vscode/package-lock.json` to the same version.
- Keep docs/examples/spec coverage aligned with shipped behavior.
- Run and report these checks before declaring completion:
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && just fmt'`
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && just clippy'`
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && just test-all'`
  - feature-specific runtime checks when CLI/runtime behavior changed
- If version was bumped, release is not complete until tag/release flow is done:
  - create and push annotated tag `v<workspace-version>` from `main`
  - confirm Release workflow for that tag is running/completed
  - confirm GitHub "Latest release" reflects that tag

## Release Candidate State Machine (non-negotiable)

- Before preparing or pushing a candidate, inventory every required GitHub job and run every
  feasible job-specific command shape that is not already an identical required artifact command.
  A host-only `just clippy`, `just test`, or `just test-all` result is never cross-platform CI
  parity. For runtime changes, the exact-SHA artifact must include
  `./scripts/check_runtime_cross_target_warnings.sh --install-missing --require-cross`; a passing
  artifact is invalid without that recorded command.
- Integration, release, `main`, and version-bump pushes require the exact-SHA artifact produced by
  `.codex/skills/trust-ci-release-gates/scripts/release_candidate_guard.py prepare`; the shared
  pre-push hook must be installed and must not be bypassed.
- Freeze one clean candidate, run focused and exact strict preflight plus final remote broad gates,
  then push once. Any code, validator, metadata, base, or instruction-file change invalidates the
  artifact and returns the work to focused validation.
- Wait for every required GitHub check before editing a red candidate. Run
  `release_candidate_guard.py collect-failures --pr <number> --wait`, retain every failed job log,
  and repair the complete ledger as one batch. Never issue serial corrective pushes from partial
  CI results.
- Merge only through `release_candidate_guard.py check-merge --pr <number> --execute`, which must
  bind the exact validated head and require every check green.
- When a version changes, continue through main CI, annotated tag, Release workflow, GitHub Latest,
  asset/checksum verification, and VS Code Marketplace propagation; verify with
  `release_candidate_guard.py verify-release --candidate-head <reviewed-head> --branch <candidate-branch>`
  before reporting completion.
- After every merge, run `release_candidate_guard.py audit-post-merge` with the exact reviewed
  candidate head and branch. Remove only the clean exact targets it reports, fetch/prune, and rerun
  until the audit reports `clean`. A merge or release handoff is incomplete while candidate
  branches, validation worktrees, dirty work, or unique commits remain unresolved.
- After a second red candidate or two elapsed hours without merge readiness, stop and report the
  complete blocker ledger. Do not continue an unbounded push/wait/fix loop.

## Skills To Use

- `trust-test-authoring`
  - Use before every product behavior change, test addition/refactor, specification-gap decision,
    or proof-mapped verification change.
  - Skill file: `.codex/skills/trust-test-authoring/SKILL.md`
- `trust-release-hygiene`
  - Use for changelog/version/release evidence and user-facing behavior updates.
  - Skill file: `.codex/skills/trust-release-hygiene/SKILL.md`
- `trust-lsp-iec`
  - Use for IEC 61131-3 language behavior, diagnostics, stdlib functions/FBs, and compliance/deviation decisions.
  - Skill file: `.codex/skills/trust-lsp-iec/SKILL.md`
- `st-lsp-solid`
  - Use for refactors, module ownership changes, runtime/LSP/CLI structure changes, and large file splits.
  - Skill file: `.codex/skills/st-lsp-solid/SKILL.md`
- `trust-ci-release-gates`
  - Use for MP-000/MP-015/MP-032 work: CI wiring, nightly reliability, release-gate artifact aggregation, and cross-platform pre-push hardening.
  - Skill file: `.codex/skills/trust-ci-release-gates/SKILL.md`
- `trust-vscode-quality`
  - Use for `editors/vscode` commands/snippets/webviews/tests and extension CI coverage.
  - Skill file: `.codex/skills/trust-vscode-quality/SKILL.md`
- `trust-hmi-contracts`
  - Use for MP-050..053 HMI contract, schema/value/write API, mapping, and safety/authz guardrails.
  - Skill file: `.codex/skills/trust-hmi-contracts/SKILL.md`
- `trust-remote-builder`
  - Use for compiling, running `just`/cargo/npm gates, syncing local work, and reporting proof from the Hetzner CPU builder.
  - Skill file: `.codex/skills/trust-remote-builder/SKILL.md`
- `vscode-capture`
  - Use for real headless screenshots or scripted interaction with the shipped VS Code extension.
  - Skill file: `.codex/skills/vscode-capture/SKILL.md`
- `vscode-ui-acceptance`
  - Use for VS Code UI acceptance-board journeys, evidence rows, and `ux_accepted` gates.
  - Skill file: `.codex/skills/vscode-ui-acceptance/SKILL.md`
- `skill-creator`
  - Use when creating or updating any skill definition/resources.

## Trigger Guidance

- Use `trust-test-authoring` for bug fixes, features, refactors, malformed-input tests, runtime
  safety, VS Code behavior, hardware claims, docs-only verification, and supply-chain/release
  claims. Its written-specification and native-test route is mandatory; planner,
  catalog, and proof metadata are never product authority.
- Use `trust-release-hygiene` whenever a task includes any of: commit preparation, changelog updates, version bump requests, release readiness, or user-facing runtime/CLI behavior changes.
- Use `trust-remote-builder` whenever a task requires compile, test, clippy, npm, VS Code extension, or release-gate proof that can run on the shared CPU builder.
- Use `st-lsp-solid` for all runtime/LSP/IDE/CLI implementation or refactor tasks that change structure, ownership, boundaries, or large files.
- Use `vscode-capture` whenever a VS Code claim needs screenshot proof from the real extension.
- Use `vscode-ui-acceptance` when a task touches acceptance journeys, UX evidence rows, or `ux_accepted` status.
- `st-lsp-solid` is mandatory for runtime-cloud, realtime communication, gateway/plugin architecture, and major UI architecture implementation.
- `st-lsp-solid` also applies to xtask gate/proof/validator code. New validators go in a new per-slice module, never appended to an already-large file. Any task that would grow a single file past ~1k lines must split instead of append.
- For implementation checklists, include explicit SOLID/KISS/DRY acceptance checks and keep status updated per item.

## Architecture Program Workflow

- For architecture-program board work, do not use `docs/internal/masterPlan.md` for sequencing.
- Start every architecture-program resume from `docs/internal/testing/checklists/architecture-workboard-index.md`.
- Then read `docs/internal/testing/checklists/full-architecture-refactor-program-checklist.md`.
- Then read only the active dedicated board checklist named by the workboard index.
- If the workboard index, umbrella checklist, and dedicated board disagree, trust the dedicated board for detailed completion evidence, then reconcile the index and umbrella mirror rows before starting the next board.
- The current active board lives in the workboard index's `Current Board Pointer`; update that pointer when a board closes.

## HMI Implementation Rules

- HMI implementation work must follow `docs/guides/HMI_OPERATOR_FIRST_IMPLEMENTATION_CHECKLIST.md`.
- For older internal board evidence, also consult `docs/internal/testing/checklists/hmi-complete-implementation-checklist.md` when it is present in the checkout.

## Browser-Visible Verification Rule

- For any browser-visible change (`/hmi`, web UI, webview, frontend rendering, browser-side JS/CSS, Three.js/WebGL, or VS Code webview content), do not declare success from unit tests alone.
- Always verify the shipped surface after the change with a real browser session.
- Do not use Puppeteer MCP for that verification.
- Prefer Playwright CLI or a short local Playwright script against the live runtime:
  - use Playwright to open the live page and capture screenshot evidence,
  - use Playwright page evaluation or scripted assertions to confirm the actual rendered/resulting state,
  - if `playwright` is not on `PATH`, use the installed Playwright package via `npx playwright` or the local cached CLI entrypoint instead of falling back to Puppeteer MCP.
- If the browser fix touches assets embedded into a binary/runtime bundle, rebuild and restart the served runtime before Playwright verification.
- For WebGL/3D fixes, verify the rendered scene itself, not just HTTP 200 or DOM load success.
- If Playwright cannot verify due to an environment limitation, state the exact failure and do not claim the browser fix is confirmed.

## Remote Builder Rule

- Heavy Rust and Node work runs on the shared Hetzner CPU builder by default.
- CPU- or filesystem-heavy PLC verification work also runs on `trust-builder`
  by default. In particular, regenerate multi-report audit/evidence batches in
  a clean detached remote worktree, validate every report there, and copy only
  the validated artifacts back to the local checkout. Do not run a sequential
  full-report regeneration batch on the local Pi merely because a local script
  already exists. Use the local machine only when the builder is unavailable or
  the task is demonstrably cheaper locally, and state that exception before the
  run.
- SSH alias: `trust-builder`
- Remote repo path: `/home/johannes/projects/trust-platform`
- Every evidence-producing builder command must be visibly wrapped in
  `ssh trust-builder '...'`; a local `workdir` that resembles a remote path is
  not remote execution. Before a new evidence batch, confirm `hostname` and
  `pwd` inside that SSH command and record the remote checkout revision.
- Before running broad remote gates, check disk and clean stale generated artifacts first:
  - Run `ssh trust-builder 'df -hT /home/johannes /tmp && du -xhd1 "$HOME/projects" 2>/dev/null | sort -h | tail -20 && du -xhd1 "$HOME/.cache" 2>/dev/null | sort -h | tail -20'`.
  - All paths in these commands are on the remote `trust-builder` machine, not on the local workstation.
  - The builder will usually not have 100G free. Do not use an impossible threshold.
  - For a cold exact-candidate `just test-all` or large native-dependency change
    (ADS, OPC UA/OpenSSL, EtherCAT, WebGPU/Scena), require at least 80G free on
    `trust-builder:/home/johannes` before starting and aim for 3G on
    `trust-builder:/tmp` after cleanup. The exact-candidate guard enforces the
    home-space floor because an uncached all-target test build can exceed 55G.
  - For `just clippy`, `just test`, VS Code `npm test`, or broad `cargo test`, aim for at least 25G free on `trust-builder:/home/johannes`.
  - If below the practical threshold, delete only generated build/cache outputs such as the active isolated validation `target/`, `fuzz/target/`, `$HOME/.cache/sccache`, or `$HOME/.cache/codex-targets/*`; never delete source worktrees or non-generated files for cleanup.
  - For isolated validation copies, prefer one warmed target directory on the remote builder (`CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate`) instead of repeatedly creating huge cold `target/` trees.
  - If space is still below threshold after safe cleanup, report the real free space and either run a narrower gate or ask before deleting large unrelated generated targets.
  - If a gate fails with `No space left on device`, `Disk quota exceeded`, `mold: failed to write`, or `couldn't create a temp dir`, stop; kill any leftover cargo/rustc/linker processes, re-run the disk preflight, clean generated artifacts, and only then rerun.
- Keep the remote checkout matched to the work being validated. For uncommitted local work,
  sync with `rsync` and exclude heavy cache directories such as `target`, `fuzz/target`,
  `node_modules`, and `.venv-docs`.
- Do not store private SSH key material, cloud credentials, or provider tokens in this repo.
- The builder is CPU-only. Browser-visible and WebGL/WebGPU work still needs real rendered
  proof; use the builder for Playwright/browser automation only when the required rendering
  path is valid without a real GPU.

## VS Code End-To-End Rule

- For any user-visible change under `editors/vscode` (commands, import/export flows, diagnostics/completion/signature-help UX, snippets, debug flows, webviews, or test harness behavior), do not declare success from Rust unit tests, `npm run compile`, or `npm run lint` alone.
- Always add or update extension tests under `editors/vscode/src/test/suite/**` for the changed behavior.
- Always ensure new VS Code test files are registered in `editors/vscode/src/test/suite/index.ts`; an unreferenced test file does not count as coverage.
- Always run `ssh trust-builder 'cd "$HOME/projects/trust-platform/editors/vscode" && npm test'` for touched VS Code behavior.
  - If reusing an already-built server is materially faster, prefer
    `ssh trust-builder 'cd "$HOME/projects/trust-platform/editors/vscode" && ST_LSP_TEST_SERVER=$HOME/projects/trust-platform/target/debug/trust-lsp npm test'`.
- If the changed UX is not fully exercised by automated extension tests, perform a manual VS Code smoke pass and report exactly what was exercised.
- If environment limits prevent the automated or manual VS Code verification, state the exact blocker and do not claim end-to-end confirmation.
- For a manual/visual smoke pass (real screenshots of the extension headlessly on the Pi), reuse the saved capture harnesses at `docs/internal/testing/evidence/vscode-ui-ux-acceptance/2026-06-25/runners/` (local, git-ignored, on persistent disk; see its `README.md`). Two families:
  - **command-driven** (`*-runner.js`): `@vscode/test-electron` `runTests` headless Extension Dev Host; a mocha test drives the extension via `vscode.commands.executeCommand` and shoots the Xvfb root. Covers First Run, ST editing, Check/Run, Live Values (`trust-lsp.debug.io.write|force|release` with `{address,value}`), Debugging, HMI.
  - **CDP** (`cdp_*.js`): same launch + `--remote-debugging-port`, then Chrome DevTools Protocol over `ws` into the webview's inner iframe (`document.querySelector('iframe').contentDocument`) to click/read the React DOM. Covers the Devices & Connections graph: protocol picker, add-device forms, node inspectors (`.react-flow__node` clicked by text via PointerEvent).
  - Run: `xvfb-run -a -s "-screen 0 1920x1080x24" node <runner>.js` (needs `target/debug/trust-{lsp,runtime}`; pin a cached `.vscode-test/vscode-linux-arm64-*`). Live Values/HMI need addressed I/O (`Configuration.st` `VAR_CONFIG ... AT %QX0.0/%QW0/%MW0`). Acceptance board lives in the same `…/evidence/vscode-ui-ux-acceptance/` tree.

## Masterplan Workflow

- This workflow does not sequence the architecture-program boards. For architecture/refactor board work, use the Architecture Program Workflow above instead.
- Execute in `docs/internal/masterPlan.md` order unless explicitly redirected.
  - Start with Phase 0 (`MP-000`, `MP-001`), then move forward by phase.
- For each MP item:
  - Reference MP id in notes/PR text.
  - Follow the Test-First Development Rule for every behavior change; use
    green behavior-lock tests for refactor-only work.
  - Implement minimal change set.
  - Update related docs/checklists.
  - Update `docs/internal/masterPlan.md` checkboxes immediately after implementation and after validation (code checklist + detailed test checklist).

## Salsa Migration Workflow (staged, no big-bang)

- Follow `docs/internal/salsa-spike-checklist.md`.
- Do not proceed to next stage before current stage tests, perf gate, and docs updates are complete.
- Stage 1 scope is limited to Salsa-backed source/parse/`file_symbols` while keeping `SourceDatabase` and `SemanticDatabase` APIs stable.
- Stage 2 must run strict edit-loop latency+CPU benchmark gates before go/no-go decision.
  - Run `ssh trust-builder 'cd "$HOME/projects/trust-platform" && ./scripts/salsa_spike_gate.sh'` (defaults: `SALSA_SPIKE_SAMPLES=3`, median comparison, 5% regression budget, 5% clear-gain threshold).
- During Stage 3 incremental steps, run the same gate in regression-only mode after each step:
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && SALSA_SPIKE_REQUIRE_CLEAR_GAIN=0 ./scripts/salsa_spike_gate.sh'`
- Post-cutover state (2026-02-08): Salsa-only query path in `trust-hir`; backend toggles removed.
- Stage 3 decision:
  - Continue migration (`analyze`/diagnostics/`type_of`) only if gate results are clearly positive.
  - Otherwise remove stale Salsa claims/dependencies and keep a clean non-Salsa path.

## Architecture + Diagram Rule (non-negotiable)

- For any refactor/feature that changes ownership, data flow, or execution flow:
  - Update PlantUML sources in `docs/diagrams/**/*.puml`.
  - Regenerate diagram outputs on `trust-builder`
    (`ssh trust-builder 'cd "$HOME/projects/trust-platform" && scripts/render_diagrams.sh'`).
  - Refresh `docs/diagrams/manifest.json` and verify drift on `trust-builder`
    (`ssh trust-builder 'cd "$HOME/projects/trust-platform" && python scripts/check_diagram_drift.py'`).
  - Update `docs/internal/testing/checklists/architecture-improvements.md`.

## Runtime Vertical Test Rule (non-negotiable)

- For runtime-impacting changes, validate end-to-end from interfaces/control surfaces down to runtime core:
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test api_smoke'`
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test debug_control'`
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test complete_program'`
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test runtime_reliability'`
- For simulation-mode changes (MP-016), also run:
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test simulation_workflow'`
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime scheduler::tests::scaled_clock_now_is_monotonic'`
- If runtime protocol/debug behavior affects VS Code flows:
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform/editors/vscode" && ST_LSP_TEST_SERVER=$HOME/projects/trust-platform/target/debug/trust-lsp npm test'`
- If runtime web UI changes are included:
  - Execute relevant sections in `docs/internal/testing/manual-tests-ui.md` and capture evidence.
- If HMI API/UI changes are included (MP-050..053):
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --lib control::tests::hmi_'`
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --lib hmi::tests::widget_mapping_covers_required_type_buckets'`
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --lib hmi::tests::trend_downsample_preserves_bounds_and_window'`
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --lib hmi::tests::alarm_state_machine_covers_raise_ack_clear_history'`
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --lib control::tests::hmi_trends_and_alarm_contracts_support_ack_flow'`
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test hmi_readonly_integration'`
  - Verify `/hmi` renders widgets and freshness/connection badges, trend/alarm pages, and `/hmi/export.json`.

## Baseline Validation (every code change)

- `ssh trust-builder 'cd "$HOME/projects/trust-platform" && just fmt'`
- `ssh trust-builder 'cd "$HOME/projects/trust-platform" && just clippy'`
- `ssh trust-builder 'cd "$HOME/projects/trust-platform" && just test'` for the fast loop
- `ssh trust-builder 'cd "$HOME/projects/trust-platform" && just test-all'` before declaring completion
- `ssh trust-builder 'cd "$HOME/projects/trust-platform/editors/vscode" && npm run lint && npm run compile && npm test'` when extension is touched.
- Before pushing changes that touch `trust-lsp` tests or dependency/config resolution:
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && ./scripts/prepush_ci_gate.sh'`
  - This gate includes `./scripts/check_test_path_hygiene.sh` to block Windows-fragile test patterns.
- Before pushing changes that touch runtime networking/mesh/TLS paths:
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && ./scripts/runtime_mesh_tls_stability_gate.sh --iterations 8'`
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets'`
- Before pushing changes that touch runtime CI fixtures/contracts (`crates/trust-runtime/tests/ci_cicd_contract.rs` or `crates/trust-runtime/tests/fixtures/ci/**`):
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test ci_cicd_contract'`
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test config_schema_command'`
  - `ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test registry_command'`
  - If CI matrix includes Windows, run the suite with deterministic threading (`-- --test-threads=1`) in workflow gates.
  - Ensure fixtures are platform-safe:
    - Do not hardcode `unix://` control endpoints in cross-platform fixtures.
    - Use `tcp://127.0.0.1:0` or apply a platform rewrite in the test harness.
  - Ensure fixture temp paths are collision-proof under parallel test execution (no timestamp-only uniqueness).
  - In shell steps with `set -u`, do not expand potentially empty arrays (`"${arr[@]}"`); use branch functions/commands instead.
- For release/version-bump work (especially on `main`):
  - Keep `Cargo.toml` workspace version, changelog, and VS Code extension versions in sync (when extension behavior changed).
  - Create/push annotated tag `v<workspace-version>`.
  - Confirm release workflow succeeded for that tag and GitHub release is published as **Latest**.
  - Preserve CI gate `version-release-guard` in `.github/workflows/ci.yml` and `scripts/check_version_release_evidence.py`.

### Staged Test Cadence Exception (for large implementation checklists)

- If a dedicated checklist defines staged gates (targeted tests between steps, full tests at milestones/end), follow that cadence instead of running `ssh trust-builder 'cd "$HOME/projects/trust-platform" && just test'` after every micro-change.
- Before the first report regeneration or broad gate, run targeted tests and a
  cheap full-diff preflight, then declare the implementation frozen. Regenerate
  bound reports once from that frozen commit. If a later code or validator fix
  is required, return to targeted tests and preflight; do not immediately repeat
  report regeneration or broad gates until the implementation is frozen again.
- Minimum requirement in staged mode:
  - run targeted tests continuously while implementing,
  - run full gates at defined big milestones,
  - run final full gate (`ssh trust-builder 'cd "$HOME/projects/trust-platform" && just fmt'`,
    `ssh trust-builder 'cd "$HOME/projects/trust-platform" && just clippy'`,
    `ssh trust-builder 'cd "$HOME/projects/trust-platform" && just test-all'`) before declaring completion.

## PLC Verification Program

- Use `.codex/skills/trust-test-authoring/SKILL.md` for the mandatory
  specification-first and test-first execution route.
- Product work is established only by an observable behavior, its written
  specification, and its native executable test. Invariant status, catalog
  linkage, denominator disposition, evidence freshness, proof level, mutation
  result, scanner fact, file count, or function count cannot create product work
  or override direct specification-and-test evidence.
- The corrective post-closure audit lives at
  `docs/internal/testing/checklists/plc-verification-program/phase18-zero-debt-execution-board.md`.
  It may contain only directly confirmed missing specifications, missing native
  tests, behavior defects, or explicit external/manual contract boundaries.
- Planner, catalog, denominator, evidence, and proof tools may be used for
  archaeology or nonblocking maintenance. They are not prerequisites for a
  product change and cannot reject a change whose written specification and
  native executable test directly agree.
- For VS Code changes, also run `python3 scripts/check_vscode_test_registration.py`; for hardware
  or protocol features, execute the specified device-in-the-loop case on the real reviewed
  topology before push. A simulator or unit test does not replace that hardware case.
- Preserve an expected assertion red before a bug fix and paired green afterward. Use behavior-lock
  evidence for refactor-only work. Do not count compile, harness, dependency, timeout, or unrelated
  failures as red evidence.
- Pull-request workflows must enforce native tests and specification drift
  checks. Verification metadata may be reported for maintenance, but it must
  not reject a change solely because a native test lacks a catalog mapping.
- Never derive product work from `reviewed_nonmapping`, ignored, skipped,
  filtered-out, unrun, zero-test, or other metadata state. Inspect the owning
  written specification and native assertions directly.
- Do not invent behavior where the specification is missing or ambiguous.
  Specify first, add the smallest native test next, and change production only
  after an honest focused assertion failure.

## IEC-First Rules

- Map behavior to IEC 61131-3 sections/tables and cite in `docs/specs/*.md` when specs are edited.
- Record standard ambiguities in `docs/IEC_DECISIONS.md`.
- Record a behavior in `docs/IEC_DEVIATIONS.md` only when truST intentionally
  conflicts with, omits, or relaxes a cited normative IEC 61131-3 requirement.
  Before adding an entry, cite the exact IEC section/table, state the normative
  requirement, state truST's behavior, and explain the concrete conflict. If no
  conflict can be stated, do not add a deviation entry.
- IEC-silent, out-of-scope, implementation-specific, platform/runtime, or
  truST-only API behavior belongs in the relevant product specification, not in
  `docs/IEC_DEVIATIONS.md`. Record genuine IEC ambiguity in
  `docs/IEC_DECISIONS.md`.
- For PLCopen Motion profile ambiguities and truST-specific PLCopen choices, use `docs/PLCOPEN_DECISIONS.md` and `docs/PLCOPEN_DEVIATIONS.md`.
- If standard functions are touched, update `docs/specs/coverage/standard-functions-coverage.md`.
- IEC source files are not checked into GitHub. If available locally, keep under `docs/internal/standards/`:
  - `IEC 61131-3_2013 Ed3.pdf`
  - `iec61131-3.txt`
- If the IEC PDF is absent locally, use `docs/internal/standards/iec61131-3.txt`
  when present. If neither exists, state that the authoritative IEC source is
  unavailable in this checkout. If `/home/johannes/Downloads/iec-61131-3.ocr.txt`
  exists on the machine, copy it to `docs/internal/standards/iec61131-3.txt`
  before IEC proof work; otherwise restore the standard text on `trust-builder`
  or state the gap before claiming standard proof.

## MP-001 Specific Guardrails

- Split large test files by capability/semantic domain.
- Keep test names and snapshot names stable when possible.
- Run affected suites after split and confirm no behavioral delta.

## Tools

- Prefer `rg` for search (fallback `grep -R`).
- Use `jq` for JSON inspection and assertions.
- Use focused test runs first, then broader gates.

## Code Areas

- `crates/trust-syntax` for lexer/parser
- `crates/trust-hir` for types/semantics
- `crates/trust-ide` for diagnostics/completions/hover
- `crates/trust-lsp` for LSP wiring

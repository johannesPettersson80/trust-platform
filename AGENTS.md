# AGENTS.md — truST

truST is an IEC 61131-3 PLC platform: Structured Text tooling (parser, semantic model, language
server), a Rust runtime for PLC targets, and a VS Code extension. It is Rust plus Structured Text by
design; the graphical IEC languages (LD, FBD, SFC) are out of scope.

This file is the shared rulebook for coding agents. Codex reads it directly, and Claude Code loads it
as the project instructions. Workflows that only some tasks need live in
`.codex/skills/<name>/SKILL.md` (see the table at the end). Keep shared rules here or in those skills,
not in one tool's private memory.

## Hard rule: all logic is in Rust

truST's logic is written in Rust. There is no exception without the user's written approval.

- **Logic** is everything whose result is stored, sent to a runtime or PLC, or decides an engineering
  or operator outcome: numerics, identification, tuning, validation and assessment, workflow and state
  rules (when an action is allowed, what Apply or Restore does), protocol and transaction handling,
  data formats and their decoding, project and file edits, checks of user input, and alarm behaviour.
- **TypeScript** (`editors/vscode`, its webviews, web UIs) only presents and wires. It renders, takes
  user input, registers VS Code commands and panels, forwards each request to Rust and shows Rust's
  answer. It never keeps a second copy of a rule that Rust owns.
- **Presentation may stay in TypeScript**: drawing, layout and styling, mouse and keyboard interaction
  (drag, resize, snap), view state such as the open tab, and geometry computed only to draw, such as
  a pipe's route on screen. Rust checks the result before anything is stored.
- **Where logic goes**: a Rust crate, reached from the extension through a `trust-lsp` command or the
  runtime protocol. Code that must run inside a webview or browser is Rust compiled to WebAssembly,
  as `crates/trust-wasm-analysis` is.
- **Structured Text** stays the language of PLC programs and libraries.
- **No new logic in TypeScript.** A change that needs logic adds it in Rust, even when similar logic
  already exists in TypeScript.
- **Existing TypeScript logic is debt to remove.** The known debt is listed under F16 in
  `docs/internal/testing/checklists/rust-logic-migration.md` (U1 to U20, from the inventory of
  29 October 2026) and moves to Rust in its own lane. TypeScript logic found later is debt: move the
  touched logic into Rust within the task, or stop and tell the user before changing it.
- **Tests**: logic is proven by Rust tests (`cargo test`). VS Code and browser tests prove only the
  wiring and what is rendered.
- **Review**: a change that adds or grows logic in TypeScript is rejected, whoever wrote it. Report any
  existing violation you find instead of building on it.

## Where things live

- `crates/trust-syntax`: lexer, parser, rowan CST. A new or changed token needs the `TokenKind` enum,
  its `SyntaxKind` mapping, lexer/parser tests or snapshots, and the token coverage in `docs/specs`.
- `crates/trust-hir`: semantic model, type checking and IEC rules (Salsa queries).
- `crates/trust-ide`: diagnostics, completion, hover, references. `crates/trust-lsp`: the LSP
  protocol boundary and commands. `crates/trust-wasm-analysis`: the same analysis in the browser.
- `crates/trust-runtime`: the host runtime, product CLI, I/O, web, HMI and control surfaces; the
  bytecode VM (`src/runtime/vm/`) is the only execution backend. `crates/trust-runtime-core`: the
  portable core.
- `crates/trust-debug`: Debug Adapter Protocol and the VS Code simulator. `crates/trust-dev`:
  developer CLI. `crates/trust-plcopen`: PLCopen XML. `crates/trust-ads-*`, `trust-tcads-native`:
  Beckhoff ADS.
- `editors/vscode`: the extension (Control Studio, HMI Builder, Devices & Connections, debugging). It
  is presentation and wiring only; its logic belongs in Rust (see the hard rule above).
- `docs/specs`: the product specifications, which are the authority on behaviour.
- `docs/internal`: ignored by git by default, but some evidence and checklist paths are tracked.
  Check with `git check-ignore -v <path>` before committing anything there.

Conventions: `smol_str::SmolStr` and `rustc_hash::FxHashMap`/`FxHashSet` where the crate already uses
them; `thiserror` in libraries, `anyhow` in CLIs. `unsafe_code = "forbid"` holds in trust-syntax,
trust-hir, trust-ide, trust-lsp and trust-dev; runtime unsafe sites are registered and reviewed
separately.

## Machines

- The local machine is a Raspberry Pi 5. Use it for editing, git and small checks.
- `trust-builder` is an SSH alias for a shared Hetzner CPU machine. It runs every cargo, `just` and
  npm build or test. Wrap each command in `ssh trust-builder '…'`, because a local path that looks
  remote is not remote. Its copy of `main` is `~/projects/trust-platform`. Feature worktrees sync to
  their own copy with rsync, excluding `target`, `fuzz/target`, `node_modules` and `.venv-docs`.
  Details are in the `trust-remote-builder` skill.
- Shared cargo targets live under `~/.cache/codex-targets/` on the builder, and several checkouts
  build into them at once. Run cargo through `scripts/with_cargo_target_lease.sh TARGET …`. Delete a
  target only with `scripts/remove_cargo_target_if_idle.sh TARGET`; exit 75 means it is in use, so
  keep it. Never glob-delete that directory.
- Before broad runs, check the filesystems holding the selected Cargo target, checkout and
  `TMPDIR`, plus concurrent builder workloads. Allow about 25 GB for Clippy, tests or `npm test`,
  and an 80 GiB floor on the selected target filesystem for a cold `just test-all`.
  A free-space snapshot is not a reservation; coordinate large runs sharing that filesystem.
  Use a task-owned `TMPDIR` on suitable disk storage, not the small `/tmp` quota.
  After `No space left on device`:
  1. stop the leftover processes by PID;
  2. clean generated targets;
  3. rerun once the cause is fixed.
- The builder has no GPU and no GitHub push credentials, so use it to fetch and validate only.
- Never put keys, tokens or other credentials in the repo, and never print them. Filter config,
  auth, service and server-log files before reading them, and keep a needed secret in a file
  with mode 600.
- Never edit a script while it may be running: bash reads a script as it executes. Write
  `<script>.new`, check it with `bash -n`, and `mv` it over the old one.
- Run anything that may take longer than about a minute in the background and say what it is.
  When it is slower than expected, look at its process and log at once instead of waiting.

## Simplicity and clarity

- Use Ponytail for coding and design when available.
- Prefer clear, idiomatic code over the fewest lines or files.
- Reuse existing code, standard libraries, and established dependencies.
- Avoid speculative abstractions and unrelated refactoring.
- Complete the requested behavior; simplicity must not reduce scope.
- Preserve useful platform boundaries, error handling, and validation.
- Project specifications, required tests, and the user's validation cadence take precedence over
  plugin shortcuts.

## How to work

- **The owner's plan comes first.** A specification carries out what the owner asked for; it is not
  a place to change the plan. Never add, keep or build a restriction, a reduced function, or a
  separate way of working for one platform or product that the owner did not ask for, even when a
  draft, an earlier assistant or a review put it in a specification. Before building a
  specification section, check each product decision in it against the owner's own words. Show the
  owner any decision they have not approved, in plain words, and wait for their answer. Never cut
  functionality to fit hardware: report the limit, and the owner decides, for example on a bigger
  board. (Owner, 16 October 2026, after a board-only design was built without approval: "you can
  not just change the complete plan and spec".)
- **Specification first.** `docs/specs` defines product behaviour. If the behaviour you need is
  missing, ambiguous or contradictory in the written specification, update it (or ask) before
  writing code. A change that follows a section without changing it names that section in a commit
  trailer, `Specification: <number or path> <section>` (for example `Specification: 35 B3.4`); the
  Verification Gate accepts it in place of a changed specification (specification 30).
  A native executable test then pins the behaviour. Verification metadata cannot create product
  work or block it; that includes catalogs, planners, proof levels and mutation reports. Details are
  in the `trust-test-authoring` skill.
- **The workflow** (owner, 31 October 2026: "i want the most professional best workflow"): work
  goes to `main` in small pull requests, and CI is the only gate. Details are in the
  `trust-ci-release-gates` skill.
  - One lane is one pull request of a few hours' work, never days. Push it with its pull request
    from the first commit; Codex reviews it when it opens, and its findings are fixed in the same
    pull request.
  - Before each push, run the compile check below and the focused tests of what changed and of
    what uses it. The full suites (`just test-all`, VS Code, the documentation and, for a version
    change, the release rehearsal) run in CI on every push; never run them as a local batch.
  - A red check: read its log, fix the cause in the same pull request and push. A failure without
    a code cause is a flake: record it with its log on the checklist and fix it as its own item.
  - A pull request merges at its exact head when its required checks are green and Codex has no
    open finding: `gh pr merge <number> --merge --match-head-commit <sha>`; branch protection
    refuses it while a required check is not green. There is no merge queue, and the branch need
    not hold the newest `main` (owner, 1 November 2026: the repository stays under the personal
    account, and GitHub gives the queue only to organizations). `main`'s CI checks every merge: a
    red `main` is fixed before anything else merges, and a release comes only from a `main` commit
    whose CI passed.
- **Review your diff before each push**: tests, imports, feature gates and the callers and
  lifecycle paths that the change reaches. Review does not prove compilation.
- **Compile check before reporting** (owner, 17 October 2026: "proposal a approved"). Before an
  agent reports code as written, it runs a compile check on the crates it changed: format, Clippy
  with `-D warnings` on all targets and features, and rustdoc with warnings denied. The check runs
  on the builder, in the worktree's own check copy and target (`trust-remote-builder` skill), and
  runs no tests. The agent fixes what the check finds in its own files. It reports, and does not
  fix, errors in files that another agent is writing. The check does not replace CI.
- **Implementation agents** (owner, 17 October 2026): implementation work runs through opencode
  subagents on the free model DeepSeek V4.1 Flash, and the lead orchestrates
  (`trust-agent-orchestration` skill). Each brief names:
  - the files to read and the only files the agent may change;
  - numbered requirements, each with a named test;
  - the exact compile check.

  The lead verifies the scope, the real check output and the tests itself, never the agent's
  claims, and commits. A task that fails two rounds goes to a stronger agent.
- **Tests prove the specification, not the code.** Every expected value comes from the
  specification and is worked out by hand. Each test names the wrong behaviour it would catch. A
  test that only repeats what the code returns proves nothing (`trust-test-authoring`).
- **Failed checks**: report a failed or unrun check as unverified, with its log. Do not weaken a
  test, skip a check or bypass a gate. Fix every failure a run shows before pushing again.
- **Bug fixes**: identify and fix the owning root cause, including affected callers; do not mask
  failures with weakened assertions, suppressions or unrelated workarounds. Add the native
  regression assertion in the same pull request as the fix. Use existing pre-fix evidence when
  available; never invent a failing run.
  A compile, harness or timeout failure is not evidence of a failed behavior assertion.
- **Scope**: build the smallest version that ships. Say in one line when a fix needs more scope. Add
  no options, abstractions or settings that nobody asked for.
- **Structure**: give each module one responsibility, and keep transport separate from logic. Put
  new functionality in a new module rather than growing a file past about 1,000 lines (see the
  `st-lsp-solid` skill).
- **Diagrams**: when ownership, data flow or execution flow changes, update
  `docs/diagrams/**/*.puml`. Then run `scripts/render_diagrams.sh` and
  `python scripts/check_diagram_drift.py` on the builder.
- **Rendered proof**: verify a browser-visible change (`/hmi`, web UI, webviews, WebGL) in a real
  browser against the live surface:
  - use Playwright, never Puppeteer MCP;
  - look at the screenshots yourself;
  - when the assets are embedded in the runtime, rebuild and restart it first.
  If the browser check cannot run, say so.
- **VS Code**: add tests under `editors/vscode/src/test/suite/`, register new files in `index.ts`
  (`python3 scripts/check_vscode_test_registration.py` checks this), and run `npm test` on the
  builder. Compiling and linting prove nothing about behaviour. Details are in the
  `trust-vscode-quality` skill.
- **Runtime**: a runtime change also runs the runtime vertical on the builder:
  `cargo test -p trust-runtime --test api_smoke --test debug_control --test complete_program --test runtime_reliability`.
  Add `--test simulation_workflow` for simulation changes.
- **Hardware and protocols**: before pushing, run the specified device-in-the-loop case on the real
  reviewed topology. A simulator or unit test does not replace it.
- **Real time**: run examples and journeys at 1× unless the user chose another speed.
- **Reporting**: name the source identity and evidence behind each claim. A local check is not
  green CI, and green CI is not a release. Say which checks are green, red or still running and
  what remains. Never weaken a test to make it pass.
- **Stop** means stop: launch nothing new, safely stop what you started, and record where you
  stopped.
- Commit, push and open pull requests as the workflow needs them (owner, 30 October 2026: "you may
  push, you never need to ask that"); merge when the checks are green. Tags
  and releases come only from the Release workflow.

## Working in several checkouts

The primary checkout `/home/johannes/projects/trust-platform` holds the current `AGENTS.md` and
`.codex/skills/`. Other worktrees may carry older copies from their branch point. Before working in
another checkout, copy them from the primary checkout, folder to folder. `CLAUDE.md` contains only
`@AGENTS.md`, so that Claude Code loads these rules in every session:

```bash
cp /home/johannes/projects/trust-platform/AGENTS.md /home/johannes/projects/trust-platform/CLAUDE.md .
rsync -a --delete /home/johannes/projects/trust-platform/.codex/skills/ .codex/skills/
```

For control work, also copy `docs/specs/control-platform-purpose.md` when it is missing.

## IEC 61131-3

- Cite the IEC section or table in specification edits.
- A genuine ambiguity goes in `docs/IEC_DECISIONS.md`.
- `docs/IEC_DEVIATIONS.md` is only for an intentional conflict with a normative requirement. Cite the
  requirement, then state truST's behaviour and the conflict. IEC-silent or truST-specific behaviour
  belongs in the product specification.
- PLCopen choices go in `docs/PLCOPEN_DECISIONS.md` and `docs/PLCOPEN_DEVIATIONS.md`.
- A change to standard functions updates `docs/specs/coverage/standard-functions-coverage.md`.
- The standard is not in git. Use `docs/internal/standards/`, either the PDF or `iec61131-3.txt`. If
  both are missing, copy `~/Downloads/iec-61131-3.ocr.txt` there, or say that the source is
  unavailable. Details are in the `trust-lsp-iec` skill.

## Product areas

- **Control platform**: `Control.PID`, simulated plants, Control Studio and the control course.
  - Before starting, read `docs/specs/control-platform-purpose.md` and the specifications it names.
    Record which of its four parts the task advances.
  - The PID demonstration runs the existing `Control.PID`, not a handwritten fixture.
  - Control Studio is general-purpose: never hard-code a domain, plant or lesson into it.
  - Course examples are runnable projects. Never call a simulation a deployment or a physical
    qualification.
  - Never edit a lesson the user approved without asking first.
  - Review every required screenshot yourself for polish and ease of use. Only the user accepts the
    final course walkthrough.
  - If a checkout lacks the control code or specifications, find the existing feature worktree with
    `git worktree list` instead of recreating them. The release tracker is
    `docs/internal/testing/checklists/control-platform-release-readiness.md` in that worktree.
- **HMI**: specification 39 (the HMI Builder and runtime display) and the `trust-hmi-contracts`
  skill.
- **Architecture program**: start from the Current Board Pointer in
  `docs/internal/testing/checklists/architecture-workboard-index.md`.
- **Runtime portability**: the boards are truST runtimes and work exactly like trust-runtime on
  Linux, macOS and Windows (owner, 16 October 2026): no board-only protocol, commands, states,
  restrictions or VS Code panels. Specification 34 owns behavior;
  `docs/internal/testing/checklists/runtime-portability-implementation-checklist.md` owns the
  current scope, remaining work, and evidence. Read its current checkpoint on every resume and
  update it before handoff. A1–A4 are separate scopes, each shipped as its own pull requests; this
  does not authorize reopening the closed extraction board.
- **PLC verification program**: `docs/internal/testing/checklists/plc-verification-program/`, with
  the `trust-test-authoring` skill.

## Releases

- A user-visible change adds a changelog fragment, `changelog.d/<topic>.md`: Keep a Changelog sections (`### Fixed`
  and the others) with their entries (`changelog.d/README.md`). `## [Unreleased]` in `CHANGELOG.md` stays empty, so
  that open pull requests never conflict over the changelog. A released section never changes, so an entry written
  before a release goes into a fragment too. CI checks all three (`scripts/changelog_fragments.py check`).
  The version bump (`scripts/bump_version.py OLD NEW DATE`) gathers the fragments into `## [<version>] - <date>`
  under the empty `## [Unreleased]`, the newest first, and removes them; the release's text on GitHub is that
  section, at most 125,000 characters.
- A release-notable change bumps `[workspace.package].version` in `Cargo.toml`. Keep
  `editors/vscode/package.json` and the root entries of `editors/vscode/package-lock.json` at the same
  version.
- A release is a pull request that only bumps the version and moves the changelog section. It
  merges like any other once CI, including the release rehearsal of its packages, is green; the
  Release workflow then tags the merge commit, publishes the GitHub release and the Marketplace
  packages, and verifies them. Nobody pushes a tag.
- Release blockers are regressions and severe bugs: safety, data loss, a broken user journey. Any
  other bug found during a release goes on the checklist for the next release (owner, 31 October
  2026; this replaces "all bugs fixed before push").
- The pre-push hook (`release_candidate_guard.py check-push`) refuses only a direct push to `main`
  and a pushed release tag; every other push goes through, and CI is the gate. The acceptance
  journeys run in CI on the linux-x64 package (`editors/vscode/src/acceptance`, specification 22):
  for a pull request that changes what the packages hold, and for every rehearsal and release
  before anything is published. Run them yourself with `npm run acceptance -- --vsix <file> --out
  <folder>` under a display when a journey fails or you change one.
- A version bump is finished only when all of these hold, checked with
  `release_candidate_guard.py verify-release`:
  - the annotated tag `v<version>` is on `main`;
  - the Release workflow passed;
  - GitHub shows the release as Latest;
  - the assets and checksums match;
  - the Marketplace shows the new version.
- Details are in the `trust-ci-release-gates` skill.

## Skills

| Skill | Use it for |
| --- | --- |
| `trust-test-authoring` | any behaviour change, bug fix or test work: from specification to native test |
| `trust-remote-builder` | builder sync, cargo target leases, disk, toolchain parity, reporting proof |
| `trust-ci-release-gates` | the workflow: pull requests, CI, merging, changelog, versions, releases |
| `trust-lsp-iec` | IEC language behaviour, the standard library, IEC decisions and deviations |
| `st-lsp-solid` | refactors, module boundaries, runtime/LSP/CLI structure, architecture tooling |
| `trust-vscode-quality` | VS Code extension work, extension tests, screenshots, UI acceptance |
| `trust-hmi-contracts` | HMI Builder, runtime HMI, bindings, commands, alarms, HMI tests |
| `trust-agent-orchestration` | implementation subagents: briefs, launching, follow-ups, reviewing their work |

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
- **Existing TypeScript logic is debt to remove.** Known debt:
  - most of `editors/vscode/src/controlStudio/` (about 36,000 lines);
  - in `editors/vscode/src/hmiBuilder/agent/` (about 3,000 lines): the HMI agent's authoring engine,
    that is its composition, edits, drafts, reviews and request handling (checklist H4).

  Do not extend it. When a task touches it, move the touched logic into Rust within the task, or stop
  and tell the user before changing it. The staged plan is
  `docs/internal/testing/checklists/rust-logic-migration.md` in the primary checkout.
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
- Check disk space before broad runs. You need about 25 GB free on `/home/johannes` for Clippy, tests
  or `npm test`, and 80 GB for a cold `just test-all`. `/tmp` is a small quota, so set `TMPDIR` to a
  directory under home for browser and VS Code tests. After `No space left on device`:
  1. stop the leftover processes by PID;
  2. clean generated targets;
  3. rerun only with the user's explicit authorization for another validation run.
- The builder has no GPU and no GitHub push credentials, so use it to fetch and validate only.
- Never put keys, tokens or other credentials in the repo.

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

- **Specification first.** `docs/specs` defines product behaviour. If the behaviour you need is
  missing, ambiguous or contradictory in the written specification, update it (or ask) before
  writing code.
  A native executable test then pins the behaviour. Verification metadata cannot create product
  work or block it; that includes catalogs, planners, proof levels and mutation reports. Details are
  in the `trust-test-authoring` skill.
- **Implementation first; one validation batch.** Finish the entire authorized implementation and
  necessary test authoring before running checks. Freeze the implementation, then execute one
  consolidated batch of the required builds, lint, tests, and browser/VS Code or hardware evidence.
  Deduplicate overlapping suites and build once where practical. Do not run per-edit checks,
  red/green loops, milestone reruns, or speculative extra checks. This order overrides conflicting
  cadence instructions in skills, plugins, checklists, and historical notes unless the user
  explicitly changes it.
- **Failed validation**: retain the complete failure ledger and report failed or unrun checks as
  unverified. Fixes may be prepared within the authorized scope, but another validation run needs
  the user's explicit authorization. Do not weaken tests, bypass release guards, or relabel a retry
  as a new batch.
- **Bug fixes**: add the native regression assertion alongside the fix and include it in the
  consolidated batch. Use existing pre-fix evidence when available; never invent a failing run.
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
- **Reporting**: say what you did, what you verified and how, and what you did not run and why. An
  unrun or failed check is unverified. Never weaken a test to make it pass.
- **Stop** means stop: launch nothing new, safely stop what you started, and record where you
  stopped.
- Commit, push, tag, merge or open pull requests only when the user asks.

## Working in several checkouts

The primary checkout `/home/johannes/projects/trust-platform` holds the current `AGENTS.md` and
`.codex/skills/`. Other worktrees may carry older copies from their branch point. Before working in
another checkout, copy them from the primary checkout, folder to folder. `CLAUDE.md` contains only
`@AGENTS.md`, so that Claude Code loads these rules in every session:

```bash
cp /home/johannes/projects/trust-platform/AGENTS.md /home/johannes/projects/trust-platform/CLAUDE.md .
rsync -a --delete /home/johannes/projects/trust-platform/.codex/skills/ .codex/skills/
```

For control work, also copy `docs/specs/control-platform-purpose.md` when it is missing. The release
guard rejects a candidate whose copies differ.

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
- **Runtime portability**: specification 34 owns behavior;
  `docs/internal/testing/checklists/runtime-portability-implementation-checklist.md` owns the
  current scope, remaining work, and evidence. Read its current checkpoint on every resume and
  update it before handoff. A1–A4 are separate scopes with one final validation batch each;
  this does not authorize implementation, extra test runs, or reopening the closed extraction board.
- **PLC verification program**: `docs/internal/testing/checklists/plc-verification-program/`, with
  the `trust-test-authoring` skill.

## Releases

- A user-visible change gets a `CHANGELOG.md` entry under `## [Unreleased]`.
- A release-notable change bumps `[workspace.package].version` in `Cargo.toml`. Keep
  `editors/vscode/package.json` and the root entries of `editors/vscode/package-lock.json` at the same
  version.
- Before pushing, run `just fmt`, `just clippy` and `just test-all` on the builder, plus the area
  checks above.
- Integration, release, `main` and version-bump pushes need the exact-SHA artifact from
  `.codex/skills/trust-ci-release-gates/scripts/release_candidate_guard.py prepare`. The installed
  pre-push hook enforces this; never bypass it.
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
| `trust-ci-release-gates` | changelog and versions, CI, pre-push, release candidates, tags, releases |
| `trust-lsp-iec` | IEC language behaviour, the standard library, IEC decisions and deviations |
| `st-lsp-solid` | refactors, module boundaries, runtime/LSP/CLI structure, architecture tooling |
| `trust-vscode-quality` | VS Code extension work, extension tests, screenshots, UI acceptance |
| `trust-hmi-contracts` | HMI Builder, runtime HMI, bindings, commands, alarms, HMI tests |

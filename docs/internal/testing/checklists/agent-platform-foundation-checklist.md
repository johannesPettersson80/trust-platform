# Agent Platform Foundation Checklist

Date opened: 2026-04-18
Last updated: 2026-04-19
Owner: Johannes + Codex
Status: Implementation complete for the documented v1 in-repo; remaining follow-up is VS Code workflow reuse plus external validation/deploy
Audience: Internal product/engineering execution
Scope: Make truST the best substrate for Copilot, local agents, CI automation, and a future website before any custom-model work.

## Goal

- Build one stable agent-facing platform surface around the existing LSP, runtime, debugger, and deterministic harness capabilities.
- Remove contract drift and product-surface ambiguity that currently make agent integration brittle.
- Productize the loop `write -> diagnose -> fix -> test -> reload`.
- Improve onboarding and vendor-authoring guidance so external agents and users can succeed without repo-internal knowledge.
- Finish with one canonical public documentation surface that is searchable, easy to navigate, and auto-published from the repo.

## Out Of Scope

- Training or fine-tuning a custom model.
- Building a large hosted website before the platform contract is stable.
- Adding unrelated new runtime/editor features before contract cleanup is complete.
- Broadening `trust-lsp` into an orchestration server; LSP remains the language-protocol surface.

## Pace Assumption

- [x] `PACE-01` This checklist assumes full-time focused execution for the `9-11 week` target.
- [x] `PACE-02` If work is part-time/evenings, re-baseline the schedule to `20-26 weeks` before kickoff.
- [x] `PACE-03` Track slip explicitly at the end of each phase instead of silently carrying schedule debt forward.

## Execution Board

Status legend: `Not Started` | `In Progress` | `Blocked` | `Done`

| Lane | Status | Target Window | Exit Gate |
| --- | --- | --- | --- |
| Global Locks + Baseline Audit | Done | Week 0 | Transport/auth/test/scope locks agreed and current-state evidence captured |
| Phase 0 - VS Code Contract Drift | Done | Week 1 | `package.json` / registrations / activation events aligned with regression coverage |
| Phase 1 - External Agent Contract | Done | Weeks 1-3 | `trust-dev agent serve` stdio JSON-RPC contract merged with integration tests |
| Phase 2 - Deterministic Harness Protocol | Done | Weeks 2-4 | `trust-harness` exposes programmable executor surface with docs + tests |
| Phase 3 - Compile / Diagnose / Reload Loop | In Progress | Weeks 3-5 | One machine-readable command closes the core edit/validate/reload loop |
| Phase 4 - Clean Workflow Surfaces | Done | Weeks 4-6 | Stable documented surface split between LSP, agent contract, and VS Code-only paths |
| Phase 5 - Agent Onboarding | Done | Weeks 5-7 | Public quickstart + three golden paths + starter examples |
| Phase 6 - Vendor Authoring Docs | Done | Weeks 6-9 | CODESYS/TwinCAT authoring docs shipped; Siemens path shipped or explicitly re-baselined |
| Phase 7 - Week-8 User Validation | Blocked | Week 8 | Real-user signal gathered and next product direction chosen |
| Phase 8 - Documentation Platform Normalization | In Progress | Weeks 9-10 | Canonical docs site live with normalized IA, search, auto-publish, and asset updates on push |
| Final Validation + Follow-On Decision | In Progress | Weeks 10-11 | Final gates passed and follow-on path selected from evidence |

## Current-State Audit Baseline

- [x] `BASE-01` `trust-lsp` currently exposes only four `workspace/executeCommand` commands:
  - `trust-lsp.moveNamespace`
  - `trust-lsp.projectInfo`
  - `trust-lsp.hmiInit`
  - `trust-lsp.hmiBindings`
  - Evidence: `crates/trust-lsp/src/main.rs`, `crates/trust-lsp/src/handlers/commands.rs`
- [x] `BASE-02` VS Code LM tool naming drift is real and must be treated as a bug:
  - declared names include `trust_get_linked_editing`, `trust_get_on_type_formatting_edits`, `trust_call_hierarchy_prepare`, `trust_call_hierarchy_incoming`, `trust_call_hierarchy_outgoing`, `trust_type_hierarchy_prepare`, `trust_type_hierarchy_supertypes`, `trust_type_hierarchy_subtypes`
  - registered names include `trust_get_linked_editing_ranges`, `trust_get_on_type_formatting`, `trust_prepare_call_hierarchy`, `trust_get_call_hierarchy_incoming`, `trust_get_call_hierarchy_outgoing`, `trust_prepare_type_hierarchy`, `trust_get_type_hierarchy_supertypes`, `trust_get_type_hierarchy_subtypes`
  - Evidence: `editors/vscode/package.json`, `editors/vscode/src/lm-tools.ts`
- [x] `BASE-03` `trust-harness` is intentionally minimal today:
  - binary surface: `load`, `cycle`
  - evidence comment: "Minimal JSON-line harness for deterministic cycle driving."
  - Evidence: `crates/trust-runtime/src/bin/trust-harness.rs`
- [x] `BASE-04` The richer deterministic executor already exists in Rust API only:
  - `set_input`
  - `get_output`
  - `set_access`
  - `bind_direct`
  - `advance_time`
  - `run_until`
  - `restart`
  - `reload_source`
  - `reload_sources`
  - `assert_eq`
  - Evidence: `crates/trust-runtime/src/harness/harness.rs`
- [x] `BASE-05` The compile/reload loop exists, but is internal VS Code runtime-panel code rather than a reusable contract.
  - Evidence: `editors/vscode/src/io-panel/compile.ts`
- [x] `BASE-06` CLI surfaces already provide useful structured outputs that should be reused instead of reinvented:
  - `trust-runtime build --ci`
  - `trust-runtime validate --ci`
  - `trust-runtime test --output json` / `--ci`
  - Evidence: `crates/trust-runtime/src/bin/trust-runtime/build.rs`, `crates/trust-runtime/src/bin/trust-runtime/run/commands.rs`, `crates/trust-runtime/src/bin/trust-runtime/test_cmd/output.rs`
- [x] `BASE-07` Docs drift already exists and should be corrected as part of platform cleanup:
  - HMI guide uses `--root` while current CLI uses `--project` / auto-detect
  - internal note references future `trust-runtime reload --project ...`
  - internal CLI spec still references `sources/` while scaffolds use `src/`
  - Evidence: `docs/guides/HMI_DIRECTORY_WORKFLOW.md`, `docs/internal/next-feature-priorities.md`, `docs/internal/runtime/trust-runtime-cli-specification.md`, `editors/vscode/src/newProject.ts`

## Product Decision Locks

- [x] `LOCK-01` The primary bet is `truST as platform substrate`, not `truST as model company`.
- [x] `LOCK-02` The external agent contract ships before any custom-model work resumes.
- [x] `LOCK-03` The website decision is deferred until the Week-8 validation gate.
- [x] `LOCK-04` The external contract is designed so a future local/on-prem model can plug in without platform rewrites.
- [x] `LOCK-05` Air-gap and IP-sensitive workflows are treated as real future requirements even if v1 uses frontier APIs in development.
- [x] `LOCK-06` Documentation normalization is a product surface, not cleanup debt; the platform is not done until the public docs are state-of-the-art and easy to navigate.

## Architecture Locks

- [x] `ARCH-01` Keep `trust-lsp` focused on standard LSP semantics and a small `workspace/executeCommand` surface.
- [x] `ARCH-02` Put agent orchestration in `trust-runtime`, not in the VS Code extension.
- [x] `ARCH-03` Use `JSON-RPC over stdio` for the first external agent transport.
  - Rationale: works for CLI agents, CI, local IDE integrations, and future hosted backends/workers without requiring editor-specific LSP transport assumptions.
- [x] `ARCH-04` Do not overload `workspace/executeCommand` as the external automation protocol.
  - Rationale: editor-centric transport semantics and capability discovery are the wrong long-term boundary for non-editor agents and future hosted services.
- [x] `ARCH-05` If a network transport is added later, it must wrap the same command handlers as stdio; do not fork business logic by transport.
- [x] `ARCH-06` Transport remains thin; runtime/LSP/business logic stays in reusable services.
- [x] `ARCH-07` Avoid expanding a runtime or extension "god object"; introduce small subsystem wrappers where orchestration crosses boundaries.
- [ ] `ARCH-08` VS Code must consume the same underlying orchestration services where practical; do not maintain parallel semantics for editor and non-editor agent flows.
- [x] `ARCH-09` Public docs have one canonical information architecture and one deploy target; do not keep multiple competing public entry points alive indefinitely.
- [x] `ARCH-10` Canonical public docs source lives under `docs/public/` with a single site config at repo root (`mkdocs.yml`); `docs/internal/`, `docs/reports/`, raw fixtures, and planning artifacts stay outside the public navigation tree.
- [x] `ARCH-11` Public docs SSG is `MkDocs + Material`.
  - Config lives at repo root in `mkdocs.yml`.
  - Theme/SSG switching is not allowed mid-build without an explicit replan.

## Security Locks

- [x] `SEC-01` Phase 1 external agent transport defaults to `stdio` only.
- [x] `SEC-02` If socket transport is added later, default bind is loopback only (`127.0.0.1`, `::1`, or Unix domain socket).
- [x] `SEC-03` Refuse non-loopback listen unless an explicit auth token is provided.
- [x] `SEC-04` Document clearly that unauthenticated remote execution is unsupported.
- [x] `SEC-05` Ensure any executor surface that can compile/run user ST is treated as privileged and audited accordingly.
- [x] `SEC-06` If tokens are introduced later, they must be opt-in and never logged in plaintext by helper commands or tests.

## Week-8 Validation Gate

- [ ] `W8-01` By the end of Week 8, an agent given only public docs completes all of:
  - create/open a project
  - read diagnostics
  - preview/apply formatting
  - run tests
  - reload runtime
- [ ] `W8-02` Each task completes in `<= 5 minutes` on a clean setup.
- [ ] `W8-03` Validation uses at least `2` sample projects:
  - one generic IEC project
  - one vendor-profile project
- [ ] `W8-04` Validation is run without repo-internal knowledge beyond the published docs.
- [ ] `W8-05` Use the Week-8 signal to decide between:
  - Path A: VS Code/platform polish
  - Path B: website MVP
  - Path C: later on-prem/local-model follow-up
- [ ] `W8-06` Use the Week-8 findings to prioritize the final docs-site information architecture and landing-page hierarchy.

## Success Criteria

- [x] `SC-01` Copilot/agents can use truST without reading extension internals or repo source.
- [x] `SC-02` The same core contract works from shell automation, VS Code-adjacent flows, and future hosted flows.
- [x] `SC-03` A user can reliably complete `edit -> diagnose -> fix -> test -> reload`.
- [x] `SC-04` Vendor-authoring guidance exists for the most important real-world authoring paths, not only import/export.
- [x] `SC-05` The custom-model plan remains frozen until real user demand proves it is necessary.
- [x] `SC-06` Public documentation is searchable, versionable, easy to scan, and auto-updated on push, including new screenshots and other static assets committed to the repo.

## Phase 0: VS Code Contract Drift

### Scope

- [x] `P0-01` Audit VS Code LM tool names across:
  - `editors/vscode/package.json`
  - `editors/vscode/src/lm-tools.ts`
  - activation events
  - tool declarations
- [x] `P0-02` Fix all mismatches between declared tool names and registered tool names.
- [x] `P0-03` Normalize naming conventions:
  - one verb scheme
  - one preview/apply naming pattern
  - one hierarchy naming pattern
- [x] `P0-04` Add a regression test that fails when:
  - activation events reference undeclared tools
  - declared tools are not registered
  - registered tools are not declared
- [x] `P0-05` Record the canonical naming rules in extension docs or a short developer note.

### Acceptance

- [x] `P0-A-01` Zero mismatches remain between activation, declaration, and registration.
- [x] `P0-A-02` The regression test fails on a synthetic mismatch and passes on the corrected tree.
- [x] `P0-A-03` Future extension work has one obvious place to add or rename LM tools.

### Validation

- [x] `P0-V-01` `cd editors/vscode && npm run lint`
- [x] `P0-V-02` `cd editors/vscode && npm run compile`
- [x] `P0-V-03` Add/update extension tests under `editors/vscode/src/test/suite/**`.

## Phase 1: External Agent Contract

### Scope

- [x] `P1-01` Add a `trust-runtime` external agent entrypoint, preferred form:
  - `trust-dev agent serve`
- [x] `P1-02` Implement `JSON-RPC over stdio` transport first.
- [x] `P1-03` Define the v1 command set:
  - `workspace.read`
  - `workspace.write`
  - `lsp.diagnostics`
  - `lsp.format`
  - `lsp.code_actions`
  - `runtime.build`
  - `runtime.validate`
  - `runtime.test`
  - `runtime.reload`
  - `harness.load`
  - `harness.set_input`
  - `harness.get_output`
  - `harness.advance_time`
  - `harness.run_until`
- [x] `P1-03a` The shipped v1 runtime/harness subset now covers:
  - `workspace.read`
  - `workspace.write`
  - `workspace.project_info`
  - `lsp.diagnostics`
  - `lsp.format`
  - `runtime.build`
  - `runtime.compile_reload`
  - `runtime.validate`
  - `runtime.test`
  - `runtime.reload`
  - `harness.load`
  - `harness.reload`
  - `harness.cycle`
  - `harness.set_input`
  - `harness.get_output`
  - `harness.advance_time`
  - `harness.run_until`
  - pending in Phase 1/4: `lsp.code_actions`
- [x] `P1-04` Map v1 methods to existing trusted surfaces where possible instead of reimplementing logic:
  - `runtime.build` -> current build path / `--ci` report
  - `runtime.validate` -> current validate path / `--ci` report
  - `runtime.test` -> current JSON test output path
  - `lsp.*` -> existing LSP or analysis services
  - `workspace.project_info` -> shared bundle/source/dependency/runtime config inspection helpers
- [x] `P1-05` Define stable structured responses:
  - success payload
  - typed error payload
  - stable error codes
  - cancellation behavior
- [x] `P1-06` Keep command handlers transport-agnostic so later socket/HTTP wrappers can reuse them.
- [x] `P1-07` Add explicit localhost/auth policy docs for any future non-stdio mode.

### Acceptance

- [x] `P1-A-01` An external process can call the server without VS Code present.
- [x] `P1-A-02` All shipped v1 methods are documented and return stable JSON payloads.
- [x] `P1-A-03` No business logic is duplicated between stdio transport and command handlers.
- [x] `P1-A-04` The external contract is narrow enough to stabilize, but broad enough to support the Week-8 task gate.

### Integration Tests

- [x] `P1-T-01` Happy-path stdio JSON-RPC smoke test.
- [x] `P1-T-02` Invalid-method / invalid-params error contract test.
- [x] `P1-T-03` Cancellation / timeout behavior test.
- [x] `P1-T-04` Live-runtime reload integration test (`runtime.reload` rebuilds `program.stbc`, issues `bytecode.reload`, and verifies the running output changes).

### Validation

- [x] `P1-V-01` Focused runtime tests for new agent-server modules.
- [x] `P1-V-02` `cargo test -p trust-runtime --test api_smoke`
- [x] `P1-V-03` `cargo test -p trust-runtime --test complete_program`

## Phase 2: Deterministic Harness Protocol

### Scope

- [x] `P2-01` Expand `crates/trust-runtime/src/bin/trust-harness.rs` beyond `load` and `cycle`.
- [x] `P2-02` Expose the useful `TestHarness` operations:
  - `load_source`
  - `load_sources`
  - `set_input`
  - `get_output`
  - `set_access`
  - `bind_direct`
  - `advance_time`
  - `cycle`
  - `run_cycles`
  - `run_until`
  - `reload_source`
  - `reload_sources`
  - `restart`
  - `snapshot`
- [x] `P2-03` Add structured typed value serialization for all relevant PLC value kinds.
- [x] `P2-04` Add optional trace/watch output for later debugging and reward/evidence uses.
- [x] `P2-05` Document `trust-harness` as the canonical deterministic executor for:
  - agents
  - CI
  - future website runtime sandbox
  - future local-model reward/eval loops

### Acceptance

- [x] `P2-A-01` An external script can drive a multi-cycle ST program deterministically over stdin/stdout JSON.
- [x] `P2-A-02` Reload preserves retained state where current harness semantics already support it.
- [x] `P2-A-03` The protocol is documented independently of the Rust API.
- [x] `P2-A-04` The protocol can be embedded later without requiring direct Rust API access.

### Integration Tests

- [x] `P2-T-01` Load -> cycle -> read outputs.
- [x] `P2-T-02` Set input -> cycle -> read outputs.
- [x] `P2-T-03` Advance time -> cycle -> timer-dependent output.
- [x] `P2-T-04` Run-until condition / bounded cycle guard.
- [x] `P2-T-05` Reload source(s) and verify expected preserved state contract.

### Validation

- [x] `P2-V-01` Focused `trust-harness` protocol tests.
- [x] `P2-V-02` `cargo test -p trust-runtime --test api_smoke`
- [x] `P2-V-03` `cargo test -p trust-runtime --test complete_program`
- [x] `P2-V-04` `cargo test -p trust-runtime --test runtime_reliability`

## Phase 3: Compile / Diagnose / Reload Loop

### Scope

- [x] `P3-01` Extract the runtime-panel compile/reload workflow into a reusable service.
- [x] `P3-02` Expose one first-class operation that:
  - saves or checks dirty files
  - collects diagnostics
  - builds when needed
  - reloads active runtime/debug session when safe
  - returns structured results
- [x] `P3-03` Reuse existing runtime reload semantics; do not create a second conflicting reload path.
- [ ] `P3-04` Make the workflow available through:
  - VS Code
  - external agent contract
- [x] `P3-05` Ensure the machine-readable result is good enough for iterative repair loops, not just human UI.
- [x] `P3-04a` The external agent contract now ships `runtime.compile_reload`; VS Code runtime-panel reuse is still pending.

### Acceptance

- [x] `P3-A-01` One command closes the loop `write -> diagnose -> build -> reload`.
- [x] `P3-A-02` Structured results include:
  - target
  - errors
  - warnings
  - issues
  - runtime status
  - runtime message
- [x] `P3-A-03` Failure modes are actionable and machine-readable.

### Integration Tests

- [x] `P3-T-01` Clean compile + reload success path.
- [x] `P3-T-02` Compile with diagnostics blocks reload and returns structured issues.
- [x] `P3-T-03` Runtime reload failure surfaces clearly while diagnostics still return.

### Validation

- [ ] `P3-V-01` Focused extension/runtime tests for compile-reload orchestration.
- [x] `P3-V-02` `cargo test -p trust-runtime --test debug_control`
- [x] `P3-V-03` `cd editors/vscode && ST_LSP_TEST_SERVER=<path>/trust-lsp npm test`

## Phase 4: Clean Workflow Surfaces

### Scope

- [x] `P4-01` Expose the existing useful capabilities as stable documented surfaces:
  - diagnostics
  - formatting preview
  - on-type formatting
  - code actions
  - rename preview
  - project info
  - test discovery/execution
- [x] `P4-02` Decide and document which surfaces are:
  - pure LSP
  - external agent contract
  - VS Code-only
- [x] `P4-03` Ensure consistent argument and result shape across adjacent commands.
- [x] `P4-04` Remove or deprecate low-value overlapping names where the surface is confusing.
- [x] `P4-05` Publish the surface map in docs so an external integrator does not need to guess where a capability lives.
- [x] `P4-01a` The external agent contract now documents and tests:
  - `workspace.project_info`
  - `lsp.diagnostics`
  - `lsp.format`
  - `runtime.build`
  - `runtime.compile_reload`
  - `runtime.test`
  - `runtime.reload`
  - full harness protocol methods

### Acceptance

- [x] `P4-A-01` An agent can discover project structure, diagnostics, fixes, tests, and reload paths from public docs alone.
- [x] `P4-A-02` Naming and payload conventions are consistent across the platform.

### Integration Tests

- [x] `P4-T-01` Diagnostics retrieval scenario.
- [x] `P4-T-02` Format preview scenario.
- [x] `P4-T-03` Code-action retrieval scenario.
- [x] `P4-T-04` Project-info retrieval scenario.
- [x] `P4-T-05` Test discovery/execution scenario.

### Validation

- [x] `P4-V-01` Focused tests for the touched LSP/extension surfaces.
- [x] `P4-V-02` `cd editors/vscode && npm run lint`
- [x] `P4-V-03` `cd editors/vscode && npm run compile`
- [x] `P4-V-04` `cd editors/vscode && ST_LSP_TEST_SERVER=<path>/trust-lsp npm test`

## Phase 5: Agent Onboarding

### Scope

- [x] `P5-01` Write a short public `Agent Quickstart`.
- [x] `P5-02` Collapse onboarding into three golden paths:
  - generic IEC authoring
  - CODESYS/TwinCAT-style authoring
  - Siemens/Mitsubishi migration path
- [x] `P5-03` Add one tiny starter example per golden path.
- [x] `P5-04` Ensure docs explain the minimum useful loop:
  - create/open project
  - ask model/agent to edit
  - run diagnostics
  - apply fix/format
  - run tests
  - reload runtime
- [x] `P5-05` Remove or demote low-signal doc branches from the first-run path.
- [x] `P5-06` Ensure the public docs used in Week 8 are the same docs this phase produces; no side-channel setup notes.
- [x] `P5-07` Write onboarding pages in a site-ready structure so Phase 8 is normalization and publishing, not a full docs rewrite.

### Acceptance

- [ ] `P5-A-01` A first-time user can reach a successful agent-assisted workflow in `<= 15 minutes`.
- [x] `P5-A-02` The first-run path does not require browsing large tutorial catalogs before success.

## Phase 6: Vendor Authoring Docs

### Scope

- [x] `P6-01` Ship day-to-day authoring guidance for `CODESYS/TwinCAT` first.
- [x] `P6-02` Ship Siemens day-to-day authoring guidance second.
- [x] `P6-03` Do not spend this phase expanding Mitsubishi import/export docs unless a concrete authoring gap blocks the above paths.
- [x] `P6-04` For each vendor path, document:
  - `vendor_profile`
  - formatting expectations
  - common diagnostics / quick fixes
  - example project
  - migration limits
- [x] `P6-05` Ensure docs match actual formatter/profile behavior in code.
- [x] `P6-06` Keep vendor docs written in canonical public-doc form so they can drop into the final docs site without duplication.

### Acceptance

- [x] `P6-A-01` A PLC engineer can choose a vendor path and know how to author code in truST without guessing.
- [x] `P6-A-02` The docs are stronger on everyday authoring, not just import/export.
- [x] `P6-A-03` If Siemens slips beyond Week 9, re-baseline explicitly rather than silently stretching the phase.

## Phase 7: Week-8 User Validation

### Scope

- [ ] `P7-01` Run interviews or guided usage sessions with at least `10` PLC engineers / target users.
- [ ] `P7-02` Show:
  - VS Code + truST + Copilot/agent flow
  - external agent contract demo
  - deterministic harness demo
- [ ] `P7-03` Ask which direction has the strongest pull:
  - local VS Code workflow
  - hosted browser IDE with AI
  - local/on-prem model workflow
- [ ] `P7-04` Record recurring blockers, missing commands, and missing docs.
- [ ] `P7-05` Write a short decision memo with evidence, not intuition.

### Acceptance

- [ ] `P7-A-01` Product direction for the next phase is chosen from actual user signal.
- [ ] `P7-A-02` Any decision to resume custom-model work is backed by real usage demand, not speculation.

## Phase 8: Documentation Platform Normalization

### Scope

- [x] `P8-01` Ship one canonical public docs site with:
  - one site config at repo root
  - one public docs source root at `docs/public/`
  - strong navigation
  - fast full-text search
  - clean desktop/mobile reading experience
  - version-ready information architecture
- [x] `P8-02` Normalize the public documentation structure around user intent, not repo ownership. Top-level navigation means:
  - `start/` = "How do I get to first success?"
  - `develop/` = "How do I author PLC code, HMIs, visual editors, libraries, and interop assets?"
  - `connect/` = "What can truST talk to, and how do I wire it up?"
  - `operate/` = "How do I build, test, debug, simulate, reload, deploy, observe, and operate it?"
  - `reference/` = "What exactly is supported, available, and configurable?"
  - `concepts/` = "How does it work under the hood?"
  - `examples/` = "Show me a working project or learning path."
- [x] `P8-02b` The landing page (`docs/public/index.md`) must:
  - explain "what is truST" in `<= 2` sentences
  - surface exactly `3` primary paths above the fold:
    - PLC engineer
    - agent developer
    - integrator
  - put install, first project, and agent quickstart above the fold
  - avoid marketing filler and avoid forcing scroll before a reader can start
- [x] `P8-02c` `connect/` vs `operate/` boundary is explicit and enforced:
  - `connect/` = setup, topology, protocols, wiring, drivers, and transport choices
  - `operate/` = day-to-day running, monitoring, debugging, rollout, and incident handling
  - pages that exist in both domains (for example runtime-cloud) must link to their counterpart explicitly
- [x] `P8-02d` Cap subsection sprawl:
  - target `<= 6` child pages per subsection (excluding `index.md`)
  - if a subsection grows past that, merge thin pages or introduce sectioned index pages
  - `reference/specifications/` is allowed to stay larger, but it must remain collapsed under one index page and never flood primary navigation
- [x] `P8-02a` Every top-level area must map to a real shipped surface in the codebase:
  - `start/` -> `trust-runtime setup`, `trust-runtime wizard`, `trust-runtime ide serve`, VS Code quickstart, Neovim/Zed setup
  - `develop/` -> `trust-lsp` authoring, visual editors (`ladder`, `statechart`, `blockly`, `sfc`), HMI authoring, libraries, PLCopen/OpenPLC/vendor interop, package registry, generated project docs
  - `connect/` -> runtime-to-runtime discovery/mesh/realtime/runtime-cloud, network access, I/O drivers, fieldbus, external protocols
  - `operate/` -> `build`, `validate`, `test`, `run`/`play`, `ui`, `ctl`, HMI/web UI, simulation, safety, observability, deploy/rollback, CI/CD
  - `reference/` -> CLI binaries, config files, agent API, harness protocol, conformance, benchmarks, specs
  - `examples/` -> tutorials, communication examples, runtime-cloud packs, visual editor examples, vendor examples, capstones
- [x] `P8-03` The required end-state tree is:

```text
mkdocs.yml
docs/
  public/
    index.md
    start/
      index.md
      installation.md
      choose-your-workflow.md
      editors.md
      first-project.md
      first-run-and-setup.md
      agent-quickstart.md
    develop/
      index.md
      project-layout.md
      vendor-profiles.md
      hmi-authoring.md
      package-registry.md
      generate-project-docs.md
      visual-editors/
        index.md
        ladder.md
        statechart.md
        blockly.md
        sfc.md
        companion-st.md
      libraries/
        index.md
        oscat.md
        plcopen-motion.md
      interoperability/
        index.md
        plcopen.md
        openplc.md
        codesys-twincat.md
        siemens.md
        mitsubishi.md
        vendor-libraries.md
    connect/
      index.md
      protocol-matrix.md
      networking-and-remote-access.md
      runtime-to-runtime/
        index.md
        transport-matrix.md
        discovery-and-pairing.md
        mesh-zenoh.md
        realtime-t0.md
        runtime-cloud-federation.md
        security.md
      external-systems/
        index.md
        modbus-tcp.md
        mqtt.md
        opc-ua.md
      devices-and-fieldbus/
        index.md
        driver-matrix.md
        io-binding.md
        multi-driver.md
        ethercat.md
        gpio.md
        simulated-and-loopback.md
    operate/
      index.md
      build-validate-test.md
      compile-validate-reload.md
      debugging-and-runtime-panel.md
      runtime-ui-and-control.md
      hmi-and-web-ui.md
      simulation.md
      safety-and-commissioning.md
      observability.md
      operator-guide.md
      ci-cd.md
      deploy-rollback.md
      runtime-cloud.md
    reference/
      index.md
      cli/
        trust-runtime.md
        trust-lsp.md
        trust-debug.md
        trust-harness.md
        trust-bundle-gen.md
      config/
        index.md
        runtime-toml.md
        io-toml.md
        simulation-toml.md
        trust-lsp-toml.md
        hmi-directory.md
        runtime-cloud.md
        vendor-profiles.md
      agent-api/
        overview.md
        v1.md
      harness/
        protocol.md
      conformance.md
      benchmarks.md
      specifications/
        index.md
        01-lexical-elements.md
        02-data-types.md
        03-variables.md
        04-pou-declarations.md
        05-expressions.md
        06-statements.md
        07-standard-functions.md
        08-standard-function-blocks.md
        09-semantic-rules.md
        10-runtime-semantics.md
        15-ladder-diagram.md
        16-ladder-profile-trust.md
        17-visual-editors-runtime-unification.md
    concepts/
      index.md
      architecture.md
      project-model.md
      scan-cycle.md
      deterministic-harness.md
      runtime-model.md
      visual-companion-model.md
      communication-planes.md
    examples/
      index.md
      learning-paths.md
      tutorials.md
      test-and-debug.md
      hmi.md
      connectivity.md
      runtime-cloud.md
      visual-editors.md
      vendor-profiles.md
      libraries-and-motion.md
      capstones.md
    changelog.md
    troubleshooting.md
    assets/
      images/
      diagrams/
      demo/
  internal/
  reports/
```

- [x] `P8-03a` The example catalog must align with the docs IA. Target repo-facing example taxonomy is:

```text
examples/
  tutorials/
  test-and-debug/
  hmi/
  connectivity/
  runtime-cloud/
  visual-editors/
  vendor-profiles/
  libraries-and-motion/
  capstones/
  archive/
```

  - This taxonomy is the default landing shape for public examples.
  - New top-level example roots outside this shape require explicit justification.
  - Existing roots may be migrated incrementally, but the public docs must present examples through this taxonomy from day one.

- [x] `P8-04` Navigation must answer common user questions directly:
  - "How do I install truST and choose the right workflow?" -> `start/installation.md` and `start/choose-your-workflow.md`
  - "How do I use VS Code / Neovim / Zed / the browser IDE?" -> `start/editors.md`
  - "How do I let Copilot or another agent work against truST?" -> `start/editors.md` and `start/agent-quickstart.md`
  - "How do I create my first project and get it running?" -> `start/first-project.md` and `start/first-run-and-setup.md`
  - "How do I structure a project?" -> `develop/project-layout.md`
  - "How do I use Ladder / Statecharts / Blockly / SFC?" -> `develop/visual-editors/`
  - "How do the visual editors relate to generated ST files?" -> `develop/visual-editors/companion-st.md`
  - "How do I scaffold or edit HMI pages?" -> `develop/hmi-authoring.md`
  - "How do I use OSCAT / PLCopen Motion / package registry / generated API docs?" -> `develop/libraries/`, `develop/package-registry.md`, and `develop/generate-project-docs.md`
  - "How do I import/export PLCopen XML or work with Siemens / CODESYS / TwinCAT / Mitsubishi?" -> `migrate/`
  - "How do I connect two runtimes?" -> `connect/runtime-to-runtime/`
  - "How do I build a multi-runtime system that also uses EtherCAT or other hardware I/O?" -> `connect/index.md` with clear links to both `runtime-to-runtime/` and `devices-and-fieldbus/`
  - "How do I expose the runtime safely over the network?" -> `connect/networking-and-remote-access.md`
  - "What runtime-to-runtime transports exist?" -> `connect/runtime-to-runtime/transport-matrix.md`
  - "What communication protocols are available?" -> `connect/protocol-matrix.md`
  - "How do I use EtherCAT?" -> `connect/devices-and-fieldbus/ethercat.md`
  - "How do I use loopback / simulated / multi-driver I/O?" -> `connect/devices-and-fieldbus/driver-matrix.md`, `multi-driver.md`, and `simulated-and-loopback.md`
  - "How do I connect to external systems like MQTT / Modbus / OPC UA?" -> `connect/external-systems/`
  - "What built-in I/O drivers exist?" -> `connect/devices-and-fieldbus/driver-matrix.md`
  - "How do I write code for Siemens / CODESYS / TwinCAT / Mitsubishi?" -> `migrate/` and `develop/vendor-profiles.md`
  - "How do I build / validate / test / hot-reload?" -> `operate/build-validate-test.md` and `operate/compile-validate-reload.md`
  - "How do I debug or use the runtime panel?" -> `operate/debugging-and-runtime-panel.md`
  - "How do I control a running runtime from terminal or CLI?" -> `operate/runtime-ui-and-control.md`
  - "How do I use the HMI and web UI?" -> `operate/hmi-and-web-ui.md`
  - "How do I simulate a plant before hardware?" -> `operate/simulation.md`
  - "How do I commission safely or understand fault behavior?" -> `operate/safety-and-commissioning.md`
  - "How do I turn on metrics / historian / observability?" -> `operate/observability.md`
  - "How do I deploy and roll back?" -> `operate/deploy-rollback.md`
  - "How do I run fleet/runtime-cloud operations?" -> `operate/runtime-cloud.md`
  - "Something is not working. Where do I start?" -> `troubleshooting.md`
  - "What changed in the latest release?" -> `changelog.md`
  - "What config files do I need and what do they mean?" -> `reference/config/`
  - "What exact CLI command or machine interface exists?" -> `reference/cli/`, `reference/agent-api/`, and `reference/harness/protocol.md`
- [x] `P8-04a` `changelog.md` is linked from the primary navbar and the landing page; a user can find recent changes in `1` click.
- [ ] `P8-04b` `troubleshooting.md` is linked from the primary navbar and from major guide templates as a cross-cutting escape hatch.
- [x] `P8-05` Consolidate the current scattered public entry points into that tree:
  - root `README.md`
  - `docs/README.md`
  - `examples/README.md`
  - `editors/vscode/README.md`
  - relevant public guides under `docs/guides/**`
- [x] `P8-06` Migrate current content by user task instead of by old folder name:
  - root `README.md` -> `docs/public/index.md` + `docs/public/start/*`
  - `docs/guides/GETTING_STARTED.md` + `docs/guides/PLC_QUICK_START.md` -> `docs/public/start/*`
  - `editors/vscode/README.md`, `docs/guides/EDITOR_SETUP_NEOVIM_ZED.md`, `docs/guides/WEB_IDE_FULL_BROWSER_GUIDE.md` -> `docs/public/start/editors.md` with matrix + anchored sections
  - `examples/README.md` + `examples/tutorials/README.md` -> `docs/public/examples/index.md` + example category pages
  - `docs/guides/PLC_DEVELOPER_GUIDE.md` + `docs/guides/UX_GLOSSARY.md` -> `docs/public/develop/project-layout.md` + `docs/public/reference/config/*`
  - `examples/ladder/README.md`, `examples/statecharts/README.md`, `examples/blockly/README.md`, `examples/sfc/**`, `docs/guides/PLCOPEN_LD_INTEROP.md`, `docs/specs/17-visual-editors-runtime-unification.md` -> `docs/public/develop/visual-editors/*` and `docs/public/migrate/plcopen.md`
  - `docs/guides/HMI_DIRECTORY_WORKFLOW.md` + HMI tutorials/examples -> `docs/public/develop/hmi-authoring.md` and `docs/public/operate/hmi-and-web-ui.md`
  - `docs/guides/OSCAT_LIBRARY_GUIDE.md`, `PLCOPEN_MOTION_LIBRARY_GUIDE.md`, and matching examples -> `docs/public/develop/libraries/*`
  - `docs/guides/PLCOPEN_*`, `OPENPLC_INTEROP_V1.md`, `SIEMENS_*`, `MITSUBISHI_*`, `VENDOR_LIBRARY_COMPATIBILITY.md`, `examples/vendor_library_stubs/README.md`, `examples/plcopen_xml_st_complete/README.md` -> `docs/public/migrate/*`
  - `docs/guides/PLC_CI_CD.md` line for `trust-runtime docs` -> `docs/public/develop/generate-project-docs.md`
  - registry CLI/profile docs and package examples -> `docs/public/develop/package-registry.md`
  - `docs/guides/PLC_IO_BINDING_GUIDE.md` -> `docs/public/connect/devices-and-fieldbus/io-binding.md` + `driver-matrix.md`
  - `examples/tutorials/17_io_backends_and_multi_driver/README.md` + `examples/communication/multi_driver/README.md` -> `docs/public/connect/devices-and-fieldbus/multi-driver.md`
  - `examples/communication/gpio/README.md` -> `docs/public/connect/devices-and-fieldbus/gpio.md`
  - `examples/communication/modbus_tcp/README.md`, `mqtt/README.md`, `opcua/README.md` -> `docs/public/connect/external-systems/*`
  - `docs/guides/ETHERCAT_BACKEND_V1.md` -> `docs/public/connect/devices-and-fieldbus/ethercat.md`
  - `docs/guides/PLC_NETWORKING.md` + `examples/tutorials/16_secure_remote_access/README.md` -> `docs/public/connect/networking-and-remote-access.md` and `docs/public/connect/runtime-to-runtime/security.md`
  - `docs/guides/PLC_MULTI_NODE.md` + `examples/tutorials/15_multi_plc_discovery_mesh/README.md` -> `docs/public/connect/runtime-to-runtime/discovery-and-pairing.md`, `mesh-zenoh.md`, and `transport-matrix.md`
  - `docs/guides/RUNTIME_CLOUD_*` + `examples/runtime_cloud/**` -> `docs/public/connect/runtime-to-runtime/runtime-cloud-federation.md` and `docs/public/operate/runtime-cloud.md`
  - `docs/guides/PLC_DEVELOPER_GUIDE.md` + `examples/tutorials/10_unit_testing_101/README.md` + `examples/tutorials/11_unit_testing_102/README.md` -> `docs/public/operate/build-validate-test.md`
  - `docs/guides/PLC_SIMULATION_WORKFLOW.md` + `examples/tutorials/18_simulation_toml_fault_injection/README.md` -> `docs/public/operate/simulation.md`
  - `docs/guides/PLC_CI_CD.md` + `examples/tutorials/21_ci_cd_project_pipeline/README.md` -> `docs/public/operate/ci-cd.md`
  - `docs/guides/PLC_OPERATOR_GUIDE.md` -> `docs/public/operate/operator-guide.md`
  - `docs/guides/PLC_SAFETY_GUIDE.md` + `examples/tutorials/19_safety_commissioning/README.md` -> `docs/public/operate/safety-and-commissioning.md`
  - `examples/tutorials/23_observability_historian_prometheus/README.md` -> `docs/public/operate/observability.md`
  - deploy docs under `docs/deploy/**` + `examples/tutorials/14_deploy_and_rollback/README.md` -> `docs/public/operate/deploy-rollback.md`
  - runtime panel / debug docs + VS Code commands -> `docs/public/operate/debugging-and-runtime-panel.md`
  - `trust-runtime ui` / `trust-runtime ctl` behavior + operator docs -> `docs/public/operate/runtime-ui-and-control.md`
  - `docs/specs/**` -> `docs/public/reference/specifications/**`
  - config guides and project examples -> `docs/public/reference/config/*`
  - CLI surfaces from `trust-runtime`, `trust-lsp`, `trust-debug`, `trust-harness`, `trust-bundle-gen` -> `docs/public/reference/cli/*`
  - bench / conformance docs, examples, and reports -> `docs/public/reference/benchmarks.md` and `docs/public/reference/conformance.md`
  - `conformance/*.md` -> `docs/public/reference/conformance.md` plus links back to raw conformance assets where needed
- [x] `P8-06a` Run an explicit example-catalog audit across every public example family and major standalone example under `examples/`.
  - Produce a review table with:
    - current path
    - proposed docs category
    - audience (`starter`, `intermediate`, `advanced`, `operator`, `integrator`)
    - hardware requirement (`none`, `simulated`, `optional hardware`, `required hardware`)
    - status decision (`keep`, `merge`, `tweak`, `archive`, `remove`)
    - reason for the decision
  - The review must answer directly:
    - what is good
    - what should stay
    - what should be removed
    - what should be merged
    - what should be renamed
    - what should be rewritten or tightened
- [x] `P8-06b` Normalize example families so they map cleanly onto the docs example categories:
  - `tutorials/` = step-by-step learning path
  - `test-and-debug/` = tests, diagnostics, runtime panel, debug loops
  - `hmi/` = HMI/web UI/process page examples
  - `connectivity/` = protocols, fieldbus, multi-driver, multi-runtime basics
  - `runtime-cloud/` = fleet/federation/topology demos
  - `visual-editors/` = ladder, statechart, blockly, sfc
  - `vendor-profiles/` = Siemens, Mitsubishi, PLCopen/import-export, vendor-interop
  - `libraries-and-motion/` = OSCAT, PLCopen Motion, reusable library patterns
  - `capstones/` = larger multi-file/full-system showcase projects
- [x] `P8-06c` Avoid duplicate-example sprawl.
  - Keep one canonical example per concept where possible.
  - Variants should live under the same family as advanced/field-validated/locale-specific variants, not as unrelated top-level examples.
  - Low-signal duplicates, stale proofs-of-concept, or unclear one-off demos should be merged, archived, or removed.
- [ ] `P8-06d` Define a minimum quality bar for every public example that survives the audit.
  - Each public example must have a README that states:
    - what it demonstrates
    - who it is for
    - prerequisites
    - hardware/simulation requirements
    - exact validation commands
    - expected result
    - where it sits in the docs IA
  - If an example cannot meet this bar, it is an archive/remove candidate.
- [x] `P8-06e` Example README and docs-link discipline:
  - every example category page in `docs/public/examples/*` links to real runnable example folders
  - every kept example README links back to its owning docs category page
  - every kept example belongs to exactly one primary docs category, even if it is cross-linked elsewhere
- [x] `P8-06f` Create an archive policy for examples that are still useful historically but not good public defaults.
  - Archived examples stay out of primary public navigation.
  - Archived examples are marked clearly as non-canonical and non-default.
  - Public docs should never send first-time users into archived material first.
- [x] `P8-07` Keep the following out of the public navigation tree unless they are deliberately promoted:
  - `docs/internal/**`
  - `docs/reports/**`
  - raw deployment assets under `docs/deploy/**`
  - spike/prototype docs such as `BROWSER_ANALYSIS_WASM_*`
  - internal checklists, roadmaps, rollout notes, and partner-only acceptance docs
  - raw conformance cases/expected/reports
- [x] `P8-08` Ensure public docs have one canonical deploy target.
  - If GitHub Pages remains the host, do not keep a separate long-lived public Pages surface that competes with the docs site.
  - Fold the existing demo/deployed public surface into the docs IA or make its relationship explicit.
- [x] `P8-09` Add missing public pages that are required by the shipped code surfaces and do not exist yet in usable public form:
  - `docs/public/start/agent-quickstart.md`
  - `docs/public/start/choose-your-workflow.md`
  - `docs/public/start/editors.md`
  - `docs/public/develop/hmi-authoring.md`
  - `docs/public/develop/package-registry.md`
  - `docs/public/develop/generate-project-docs.md`
  - `docs/public/develop/visual-editors/index.md`
  - `docs/public/develop/visual-editors/ladder.md`
  - `docs/public/develop/visual-editors/statechart.md`
  - `docs/public/develop/visual-editors/blockly.md`
  - `docs/public/develop/visual-editors/sfc.md`
  - `docs/public/develop/visual-editors/companion-st.md`
  - `docs/public/migrate/vendor-libraries.md`
  - `docs/public/connect/protocol-matrix.md`
  - `docs/public/connect/networking-and-remote-access.md`
  - `docs/public/connect/runtime-to-runtime/index.md`
  - `docs/public/connect/runtime-to-runtime/transport-matrix.md`
  - `docs/public/connect/runtime-to-runtime/discovery-and-pairing.md`
  - `docs/public/connect/runtime-to-runtime/mesh-zenoh.md`
  - `docs/public/connect/runtime-to-runtime/realtime-t0.md`
  - `docs/public/connect/runtime-to-runtime/runtime-cloud-federation.md`
  - `docs/public/connect/external-systems/modbus-tcp.md`
  - `docs/public/connect/external-systems/mqtt.md`
  - `docs/public/connect/external-systems/opc-ua.md`
  - `docs/public/connect/devices-and-fieldbus/driver-matrix.md`
  - `docs/public/connect/devices-and-fieldbus/multi-driver.md`
  - `docs/public/connect/devices-and-fieldbus/simulated-and-loopback.md`
  - `docs/public/operate/build-validate-test.md`
  - `docs/public/operate/compile-validate-reload.md`
  - `docs/public/operate/debugging-and-runtime-panel.md`
  - `docs/public/operate/runtime-ui-and-control.md`
  - `docs/public/operate/hmi-and-web-ui.md`
  - `docs/public/operate/safety-and-commissioning.md`
  - `docs/public/operate/observability.md`
  - `docs/public/operate/operator-guide.md`
  - `docs/public/reference/agent-api/overview.md`
  - `docs/public/reference/agent-api/v1.md`
  - `docs/public/reference/harness/protocol.md`
  - `docs/public/reference/cli/trust-runtime.md`
  - `docs/public/reference/cli/trust-lsp.md`
  - `docs/public/reference/cli/trust-debug.md`
  - `docs/public/reference/cli/trust-harness.md`
  - `docs/public/reference/cli/trust-bundle-gen.md`
  - `docs/public/reference/config/index.md`
  - `docs/public/reference/config/runtime-toml.md`
  - `docs/public/reference/config/io-toml.md`
  - `docs/public/reference/config/simulation-toml.md`
  - `docs/public/reference/config/hmi-directory.md`
  - `docs/public/reference/config/runtime-cloud.md`
  - `docs/public/reference/benchmarks.md`
  - `docs/public/concepts/project-model.md`
  - `docs/public/concepts/scan-cycle.md`
  - `docs/public/concepts/deterministic-harness.md`
  - `docs/public/concepts/visual-companion-model.md`
  - `docs/public/concepts/communication-planes.md`
  - `docs/public/examples/tutorials.md`
  - `docs/public/examples/test-and-debug.md`
  - `docs/public/examples/hmi.md`
  - `docs/public/examples/connectivity.md`
  - `docs/public/examples/runtime-cloud.md`
  - `docs/public/examples/visual-editors.md`
  - `docs/public/examples/vendor-profiles.md`
  - `docs/public/examples/libraries-and-motion.md`
  - `docs/public/examples/capstones.md`
  - `docs/public/troubleshooting.md`
- [x] `P8-09a` Pages that are not yet content-complete must ship as explicit placeholders with:
  - a short status note
  - links to the nearest filled page
  - no dead-end nav nodes and no `404` outcome from internal navigation
- [x] `P8-10` Add automatic docs build + publish on push to `main`.
- [x] `P8-11` Ensure new screenshots, diagrams, and other static assets committed to the repo are published automatically with the next docs deploy.
- [x] `P8-12` Add docs quality gates:
  - docs build check
  - broken-link check
  - navigation sanity check
  - image/static-asset render/path sanity check
- [x] `P8-13` Keep generated or contract-driven reference material as close to source-of-truth as possible.
  - avoid hand-maintained duplicate API reference where generation is possible
  - version the agent contract reference from day one
- [x] `P8-14` Add a short publishing/maintenance note so future contributors know how to add pages, screenshots, and references without drifting structure.

### Acceptance

- [ ] `P8-A-01` A first-time reader can find install, first project, workflow selection, agent quickstart, visual editor docs, vendor guide, connectivity docs, CLI/reference, and compile/validate/reload workflow in `<= 3` clicks from the landing page or via search.
- [ ] `P8-A-02` Search returns the relevant page in the top results for common tasks such as:
  - critical 15 that must be correct and top-ranked:
    - install
    - first project
    - agent quickstart
    - vscode
    - hmi
    - ladder
    - CODESYS
    - connect runtimes
    - protocols
    - EtherCAT
    - MQTT
    - Modbus TCP
    - OPC UA
    - runtime.toml
    - deploy
  - extended search set that should also work well:
    - browser ide
    - neovim
    - agent / Copilot
    - statechart
    - blockly
    - sfc
    - TwinCAT
    - Siemens
    - Mitsubishi
    - PLCopen
    - OpenPLC
    - package registry
    - io.toml
    - simulation.toml
    - trust-lsp.toml
    - runtime-to-runtime transports
    - remote access
    - simulated
    - loopback
    - Zenoh
    - mesh
    - discovery
    - pairing
    - validate
    - debug
    - runtime panel
    - rollback
    - observability
    - metrics
    - historian
    - conformance
    - benchmarks
    - harness
- [x] `P8-A-03` Public docs no longer require users to hop between multiple README files to understand the product.
- [x] `P8-A-04` A commit that adds or updates a referenced screenshot or other docs asset causes the published docs site to show the new asset without any manual deploy step.
- [x] `P8-A-05` The site reflects the Phase 1/2/3/5/6 deliverables and the exact file structure above as the canonical public surface.
- [x] `P8-A-06` `examples/` remains a catalog of runnable repo examples and tutorials, not a second copy of the guide system; example catalog pages must always link back to real example folders/READMEs.
- [x] `P8-A-07` Every kept public example is reachable from exactly one primary example category in the docs, and every example category page points to runnable examples instead of vague placeholders.
- [x] `P8-A-08` The example audit produces a concrete decision log for keep / tweak / merge / archive / remove, and the resulting catalog feels intentionally curated rather than like a dump of everything that ever existed.

### Integration Tests

- [x] `P8-T-01` Docs build succeeds in CI and locally from a clean checkout.
- [x] `P8-T-02` Broken-link checker passes on the generated site.
- [x] `P8-T-03` Static asset smoke test proves newly committed screenshots/images are bundled and served by the docs site.
- [x] `P8-T-04` Navigation smoke test proves the required landing paths are reachable and correctly titled.
- [x] `P8-T-05` Example-link audit proves every docs example entry points to an existing repo example path and every kept example README links back to a valid docs category page.

### Validation

- [ ] `P8-V-01` Verify the deployed docs site after rollout, not just the local build artifact.
- [ ] `P8-V-02` Verify landing page, search, at least one guide page, one reference page, and one page with images/screenshots.
- [ ] `P8-V-03` Record the public docs URL in this checklist and in the repo root docs pointers after launch.

## SOLID / KISS / DRY Acceptance

- [x] `SKD-01` Transport code is separate from orchestration/business logic.
- [ ] `SKD-02` No duplicate orchestration semantics between VS Code-only code and the external agent contract.
- [x] `SKD-03` New files/modules keep single responsibility and avoid central "manager" bloat.
- [x] `SKD-04` Public contract behavior is documented before downstream consumers depend on it.
- [ ] `SKD-05` Any unavoidable architectural debt is recorded in `docs/internal/testing/checklists/architecture-improvements.md` or `docs/internal/notes/runtime-refactor-notes.md`.
- [x] `SKD-06` Public docs have one source of truth per topic; avoid duplicated onboarding/reference content across multiple public markdown files.

## Final Validation Gate

- [x] `FG-01` `just fmt`
- [x] `FG-02` `just clippy`
- [x] `FG-03` `just test`
- [x] `FG-04` `just test-all`
- [x] `FG-05` `cd editors/vscode && npm run lint`
- [x] `FG-06` `cd editors/vscode && npm run compile`
- [x] `FG-07` `cd editors/vscode && ST_LSP_TEST_SERVER=<path>/trust-lsp npm test` when extension/LM tool surfaces change
- [x] `FG-08` Runtime vertical tests when runtime behavior changes:
  - `cargo test -p trust-runtime --test api_smoke`
  - `cargo test -p trust-runtime --test debug_control`
  - `cargo test -p trust-runtime --test complete_program`
  - `cargo test -p trust-runtime --test runtime_reliability`
- [x] `FG-09` Docs-site build + link/image/navigation gates pass once Phase 8 lands.
- [ ] `FG-10` Public docs deploy has completed successfully and serves the current commit content.

## Deliverables

- [x] `D-01` Drift-free VS Code LM tool contract.
- [x] `D-02` External agent contract outside VS Code.
- [x] `D-03` Expanded deterministic harness protocol.
- [x] `D-04` First-class compile/diagnose/reload flow.
- [x] `D-05` Public agent quickstart.
- [x] `D-06` CODESYS/TwinCAT authoring guide.
- [ ] `D-07` Week-8 product-direction decision memo based on user signal.
- [x] `D-08` Canonical public docs site with normalized structure and search.
- [x] `D-09` Automatic docs deployment pipeline that publishes new screenshots and other docs assets on push.
- [x] `D-10` Curated example catalog aligned to the docs IA, with explicit keep / tweak / merge / archive / remove decisions recorded.

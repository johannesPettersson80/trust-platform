# Runtime CLI Product / Workbench Split Checklist

Status: Complete; final full merge gate passed for `v0.24.12`
Owner: Runtime/dev tooling
Scope: address audit F10 by separating field/product runtime commands from developer/workbench commands.

Boundary note: post-closeout follow-up promoted `trust-dev` from a binary implementation tree inside `trust-runtime` into its own Cargo package at `crates/trust-dev/`. The shipped binary name remains `trust-dev`, and `trust-runtime` keeps the deprecated forwarding aliases required by this board.

## Command Variant Classes

- [x] `RTCLI-CLASS-01` Product runtime commands: `run`, `play`, `ctl`, `validate`, `build`, `hmi`, `plcopen`, `registry`, `setup`, `deploy`, `rollback`.
- [x] `RTCLI-CLASS-02` UI product commands: `ui`, `ide`, `config-ui`, `wizard` retained as shipped product surfaces.
- [x] `RTCLI-CLASS-03` Conformance/benchmark commands: `bench`, `conformance`.
- [x] `RTCLI-CLASS-04` Workbench/dev command variants: `agent`, `commit`, `docs`, `test`.
- [x] `RTCLI-CLASS-05` Shell completion command: `completions` remains CLI infrastructure / product-supporting until Phase 3 decides packaging exclusions.

## Bin Module Classes

These are source modules under `crates/trust-runtime/src/bin/trust-runtime/`, not necessarily CLI enum variants.

- [x] `RTCLI-MOD-01` Product runtime modules: `run.rs`, `ctl.rs`, `build.rs`, `hmi.rs`, `plcopen.rs`, `registry.rs`, `setup.rs`, `setup_web.rs`, `deploy.rs`.
- [x] `RTCLI-MOD-02` UI product modules: `config_ui.rs`, `wizard.rs`.
- [x] `RTCLI-MOD-03` Conformance/benchmark modules: `bench.rs`, `conformance.rs`.
- [x] `RTCLI-MOD-04` Workbench/dev implementation modules: `agent.rs`, `commit.rs`, `docs.rs`, `workflow.rs`, and `test.rs`; moved implementations belong under the `trust-dev` package with deprecated `trust-runtime` forwarding wrappers during the migration window.
- [x] `RTCLI-MOD-05` CLI infrastructure modules: `cli.rs`, `completions.rs`, `git.rs`, `prompt.rs`, `style.rs`, and `ci.rs`; allowed dependency rules remain explicit in Phase 2.

## Subcommand Action Classes

Nested `*Action` enums inherit their parent command class unless a row below explicitly overrides them.

- [x] `RTCLI-ACTION-01` Product/runtime action enums: `ControlAction`, `HmiAction`, `PlcopenAction`, `RegistryAction`.
- [x] `RTCLI-ACTION-02` UI product action enum: `ConfigUiAction` when used through `ide` or `config-ui`.
- [x] `RTCLI-ACTION-03` Conformance/benchmark action enum: `BenchAction`.
- [x] `RTCLI-ACTION-04` Workbench/dev action enum: `AgentAction`.
- [ ] `RTCLI-ACTION-05` Any new nested `*Action` enum must either inherit a parent class or declare an explicit override before merge.

## Phase 0 - Full-Map Prerequisite

- [x] `RTCLI-P0-001` Hard prerequisite: `architecture-doctor --full-map` MVP implements `FULLMAP-CHECK-06` for runtime command, nested action, and bin-module ownership before Phase 2 or any command movement starts.
- [x] `RTCLI-P0-002` No waiver required: `FULLMAP-CHECK-06` passed on baseline `ade92b185` and wrote `target/gate-artifacts/full-software-map-ade92b185/full-map-report.json`.
- [x] `RTCLI-P0-GATE-01` Do not claim `ARCHPROG-C-01` or `ARCHPROG-C-03` complete until `FULLMAP-CHECK-06` or its waiver is recorded.

## Stop Rules

- [ ] `RTCLI-STOP-01` Do not add a new runtime binary command variant without a command class.
- [ ] `RTCLI-STOP-01A` Do not add a new `src/bin/trust-runtime/*.rs` module without a module class.
- [ ] `RTCLI-STOP-01B` Do not add a new nested `*Action` enum or variant that changes product/workbench ownership without an action class.
- [ ] `RTCLI-STOP-02` Do not let product runtime commands import git/docs/agent/workflow helpers.
- [ ] `RTCLI-STOP-03` Do not move commands without a compatibility/deprecation decision.
- [ ] `RTCLI-STOP-04` Do not use the runtime-core split to hide workbench code behind re-exports.

## Phase 1 - Inventory

- [x] `RTCLI-P1-001` List every `BenchAction` and main `trust-runtime` command enum variant.
- [x] `RTCLI-P1-002` Map each command variant to its implementation file.
- [x] `RTCLI-P1-003` Record line counts for each command implementation file.
- [x] `RTCLI-P1-004` List every top-level bin module under `crates/trust-runtime/src/bin/trust-runtime/*.rs`.
- [x] `RTCLI-P1-005` Map each bin module to the command variant, helper, or internal infrastructure that uses it.
- [x] `RTCLI-P1-006` Identify modules that shell out to git, docs, CI, or source-bundling logic.
- [x] `RTCLI-P1-007` Identify modules safe for field-deployed runtime artifacts.
- [x] `RTCLI-P1-008` Record current CLI help output before moving anything.
- [x] `RTCLI-P1-009` List every nested CLI `*Action` enum and classify each as inherited or override.

Phase 1 evidence captured on 2026-04-29 from released baseline `ade92b185`:

- Full-map prerequisite: `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map` passed `FULLMAP-CHECK-06` with 22 command variants, 24 top-level bin modules, and 7 nested action enums classified. Artifacts: `target/gate-artifacts/full-software-map-ade92b185/software-map.json` and `target/gate-artifacts/full-software-map-ade92b185/full-map-report.json`.
- CLI help baseline: `RUSTUP_TOOLCHAIN=1.95 cargo run -p trust-runtime --bin trust-runtime -- --help > target/gate-artifacts/runtime-cli-product-workbench-split-ade92b185/trust-runtime-help.txt`.
- Main command variants and dispatch:
  - Product: `Run -> run::run_runtime` (`run.rs` / `run/`); `Play -> run::run_play` (`run/commands.rs`, compatibility alias, no `play.rs`); `Ctl -> ctl::run_control` (`ctl.rs`); `Validate -> run::run_validate` (`run/commands.rs`, no `validate.rs`); `Build -> build::run_build` (`build.rs`); `Hmi -> hmi::run_hmi` (`hmi.rs`); `Plcopen -> plcopen::run_plcopen` (`plcopen.rs`); `Registry -> registry::run_registry` (`registry.rs`); `Setup -> setup::run_setup` (`setup.rs`, `setup/`, `setup_web.rs`); `Deploy -> deploy::run_deploy` (`deploy.rs`, `deploy/`); `Rollback -> deploy::run_rollback` (`deploy/commands.rs`, no `rollback.rs`).
  - UI product: `Ui -> trust_runtime::ui::run_ui` (`crates/trust-runtime/src/ui.rs`, library surface); `Ide -> config_ui::run_ide_serve` (`config_ui.rs`, shared handler, no `ide.rs`); `ConfigUi -> config_ui::run_config_ui_serve` (`config_ui.rs`); `Wizard -> wizard::run_wizard` (`wizard.rs`).
  - Conformance/benchmark: `Bench -> bench::run_bench` (`bench.rs`, `bench/`); `Conformance -> conformance::run_conformance` (`conformance.rs`, `conformance/`).
  - Workbench/dev: `Agent -> agent::run_agent_serve` (`agent.rs`); `Commit -> commit::run_commit` (`commit.rs`, `git.rs` helper); `Docs -> docs::run_docs` (`docs.rs`, `docs/`); `Test -> test::run_test` (`test.rs`, `test_cmd/`).
  - CLI support: `Completions -> completions::run_completions` (`completions.rs`).
- BenchAction inventory: `Project`, `Init`, `T0Shm`, `MeshZenoh`, `Dispatch`; all inherit `conformance_benchmark`.
- Nested action inventory: `ControlAction` product (`BreakpointsClear`, `BreakpointsList`, `BreakpointsSet`, `ConfigGet`, `ConfigSet`, `Eval`, `Health`, `IoForce`, `IoRead`, `IoUnforce`, `IoWrite`, `Pause`, `Restart`, `Resume`, `Set`, `Shutdown`, `Stats`, `Status`, `StepIn`, `StepOut`, `StepOver`); `HmiAction` product (`Init`, `Reset`, `Update`); `PlcopenAction` product (`Export`, `Import`, `Profile`); `RegistryAction` product (`Download`, `Init`, `List`, `Profile`, `Publish`, `Verify`); `ConfigUiAction` UI product (`Serve`); `BenchAction` conformance/benchmark (`Project`, `Init`, `T0Shm`, `MeshZenoh`, `Dispatch`); `AgentAction` workbench/dev (`Serve`).
- Top-level bin module line counts and ownership:
  - Product: `run.rs` 53, `ctl.rs` 308, `build.rs` 75, `hmi.rs` 114, `plcopen.rs` 311, `registry.rs` 237, `setup.rs` 25, `setup_web.rs` 555, `deploy.rs` 19.
  - UI product: `config_ui.rs` 251, `wizard.rs` 463.
  - Conformance/benchmark: `bench.rs` 40 with `bench/` total 2013, `conformance.rs` 21 with `conformance/` total 821.
  - Workbench/dev: `agent.rs` 1371, `commit.rs` 191, `docs.rs` 27 with `docs/` total 1061, `workflow.rs` 586, `ci.rs` 124, `test.rs` 150 with `test_cmd/` total 1281.
  - CLI infrastructure: `cli.rs` 16 with `cli/commands.rs` 341 and `cli/tests.rs` 542; `completions.rs` 12; `git.rs` 58; `prompt.rs` 114; `style.rs` 53.
- Shell-out / source-bundling / developer helper modules:
  - `git.rs` shells out to `git` and is used by setup/product and workbench/dev paths; it needs Phase 2 allowed-dependency rules before movement.
  - `commit.rs` is a workbench/dev command layered on `git.rs`.
  - `agent.rs` uses `workflow.rs`, `build.rs`, and runtime harness/source collection for external agent workflows.
  - `docs.rs` / `docs/` collect and render API docs from ST source comments; classified workbench/dev for split purposes.
  - `test.rs` / `test_cmd/` discover and execute ST tests from project sources; classified workbench/dev until compatibility policy says otherwise.
  - `ci.rs` classifies CI exit codes and is currently pulled by the main CLI error path; it needs an explicit Phase 2 rule because product commands use `--ci`.
  - `prompt.rs` and `style.rs` are shared CLI infrastructure used by product setup/onboarding and workbench paths.
- Field artifact safety at inventory time:
  - Safe for field/runtime artifacts: `product`, `ui_product`, and `support`/shared CLI infrastructure modules when their dependencies stay within approved product/support boundaries.
  - Runtime-adjacent but not field-minimal: `bench` and `conformance`; keep only if Phase 3 accepts benchmark/conformance commands in shipped runtime artifacts.
  - Not field-minimal without an explicit retained rationale: `agent`, `commit`, `docs`, `workflow`, `test`, and dev-only parts of `ci`.

## Phase 2 - Policy And Doctor

- [x] `RTCLI-P2-001` Add command ownership metadata in code or a doctor config.
- [x] `RTCLI-P2-002` Add a doctor rule failing unclassified command variants.
- [x] `RTCLI-P2-003` Add bin-module ownership metadata in code or a doctor config.
- [x] `RTCLI-P2-004` Add a doctor rule failing unclassified bin modules.
- [x] `RTCLI-P2-005` Add forbidden import checks from product commands/modules to workbench modules.
- [x] `RTCLI-P2-006` Add a packaging/profile rule if field runtime artifacts must exclude workbench commands/modules.
- [x] `RTCLI-P2-007` Add a compatibility/deprecation rule for moved commands.
- [x] `RTCLI-P2-008` Add a doctor rule failing unclassified nested `*Action` enums or action variants with explicit ownership overrides.

Phase 2 policy evidence already present before command movement:

- `xtask/config/full_map_policy.json` contains `runtime_command_classes`, `runtime_bin_module_classes`, `runtime_action_classes`, and explicit route exceptions in `runtime_command_module_routes`.
- `FULLMAP-CHECK-06` fails unclassified runtime command variants, unclassified top-level runtime bin modules, and unclassified nested action enums.
- `FULLMAP-P4-003` / `known_bad_product_bin_importing_workbench_module_fails` cover product command/module imports from workbench modules.
- `xtask/config/full_map_policy.json` now declares two runtime artifact profiles: `release-host-runtime` preserves the current release bundle surface and ships `trust-dev` during migration, while `field-runtime-minimal` excludes `workbench_dev` and `conformance_benchmark` classes. `known_bad_field_runtime_profile_including_workbench_fails` proves the doctor fails if the field profile includes workbench/dev behavior.
- `xtask/config/full_map_policy.json` now declares the workbench command migration policy for `Agent`, `Commit`, `Docs`, and `Test`: current binary `trust-runtime`, destination binary `trust-dev`, compatibility plan `deprecated_forwarding_alias`. `known_bad_workbench_command_without_migration_policy_fails` and `documented_workbench_command_migration_policy_passes` lock the rule.

## Phase 3 - Target Split

- [x] `RTCLI-P3-001` Decide destination for workbench commands: `xtask`, `trust-dev`, or another explicit tool binary. Decision: `trust-dev` owns the workbench/dev commands as a separate package at `crates/trust-dev`, with `trust-runtime` retaining deprecated forwarding aliases during the migration window so current public commands do not disappear abruptly.
- [x] `RTCLI-P3-002` Move `agent` implementation out of product runtime binary or wrap it behind a deprecated forwarding alias. Evidence: `trust-dev agent serve` now owns the JSON-RPC agent server and workflow helper under `crates/trust-dev/src/`; `trust-runtime agent serve` is a deprecated forwarding wrapper through `dev_forward.rs`; product `build`, `ctl`, and `test` no longer retain agent-only JSON helper functions.
- [x] `RTCLI-P3-003` Move `commit` command implementation and `git.rs` helper implementation out of product runtime binary or wrap them behind deprecated forwarding aliases. Evidence: `trust-dev commit` now owns the commit implementation and dev-only git repo/status helpers under `crates/trust-dev/src/`; `trust-runtime commit` is a deprecated forwarding wrapper through `dev_forward.rs`; product `git.rs` only retains `git_init` for wizard/setup flows.
- [x] `RTCLI-P3-004` Move `docs` command implementation and remaining `prompt.rs`, `style.rs`, `ci.rs`, and dev-only `test` command implementation as decided. Evidence: `trust-dev docs` now owns the ST API documentation generator under `crates/trust-dev/src/docs.rs` and `crates/trust-dev/src/docs/`; `trust-dev test` now owns the ST test runner under `crates/trust-dev/src/test.rs` and `crates/trust-dev/src/test_cmd/`; `trust-runtime docs` and `trust-runtime test` are deprecated forwarding wrappers. `prompt.rs` and `style.rs` remain product CLI infrastructure in `trust-runtime`, and `crates/trust-dev/src/ci.rs` owns the workbench copy of CI exit-code classification.
- [x] `RTCLI-P3-005` Keep product runtime commands behavior-compatible. Evidence: `trust-runtime docs` and `trust-runtime test` forwarding aliases are covered by focused compatibility tests; `trust-dev test --ci` preserves the deterministic CI failure code `12`; CI templates still run product `trust-runtime build/validate --ci` and workbench `trust-dev test --ci`.
- [x] `RTCLI-P3-006` Keep benchmark/conformance commands only if explicitly accepted as runtime-adjacent. Evidence: `xtask/config/full_map_policy.json` classifies `Bench` and `Conformance` as `conformance_benchmark`; `release-host-runtime` currently includes that class for compatibility, while `field-runtime-minimal` excludes `conformance_benchmark` and `workbench_dev`.

## Phase 4 - Tests

- [x] `RTCLI-P4-001` CLI help snapshot or assertion covers command list/classes.
- [x] `RTCLI-P4-002` Product command smoke tests still pass.
- [x] `RTCLI-P4-003` Moved command compatibility wrappers are tested if retained.
- [x] `RTCLI-P4-004` Doctor rejects a synthetic unclassified command.
- [x] `RTCLI-P4-005` Doctor rejects a synthetic unclassified bin module.
- [x] `RTCLI-P4-006` Doctor rejects product command/module import of workbench module.
- [x] `RTCLI-P4-007` Doctor rejects a synthetic nested `*Action` enum or action variant with no inherited or explicit class.

Phase 4 evidence:

- `cargo test -p trust-runtime --test commit_command -- --nocapture` covers `trust-dev commit --dry-run` and the retained `trust-runtime commit` forwarding alias with the deprecation warning.
- `cargo test -p trust-runtime --test agent_command -- --nocapture` covers `trust-dev agent serve` for the full JSON-RPC agent contract and the retained `trust-runtime agent serve` forwarding alias with the deprecation warning.
- `cargo test -p trust-runtime --test docs_command --test st_test_cli_command --test ci_cicd_contract --test oscat_oop_library --test plcopen_motion_oop_library -- --nocapture` covers `trust-dev docs`, `trust-dev test`, retained `trust-runtime docs/test` aliases, stable `--ci` exit codes, and OSCAT/PLCopen ST test execution through the new workbench binary.
- `cargo test -p trust-dev` covers the extracted workbench package directly after the post-closeout crate/package split.
- `cargo test -p xtask full_map -- --nocapture` covers known-bad doctor fixtures for unclassified command variants, unclassified bin modules, product imports of workbench modules, unclassified nested action enums, stale command routes, and missing workbench migration policy.
- `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map` passes `FULLMAP-CHECK-06` with 22 command variants, 24 bin modules, 7 nested action enums, explicit workbench migrations for `Agent`/`Commit`/`Docs`/`Test`, and runtime artifact profile policy for `release-host-runtime` versus `field-runtime-minimal`.
- `just fmt`, `just clippy`, and `just test-all` passed for the `v0.24.12` release gate after freeing generated cache space; the earlier `just test-all` attempt failed only because the filesystem filled during `mold` linking.

## Exit Criteria

- [x] `RTCLI-EXIT-01` Every command is classified.
- [x] `RTCLI-EXIT-02` Every bin module is classified.
- [x] `RTCLI-EXIT-03` Product runtime binary no longer silently grows workbench/dev behavior.
- [x] `RTCLI-EXIT-04` Workbench/dev commands and modules have an explicit home or explicit retained rationale.
- [x] `RTCLI-EXIT-05` Runtime-core split is not blocked by CLI workbench dependencies.
- [x] `RTCLI-EXIT-06` Every nested CLI action enum is classified by inheritance or explicit override.

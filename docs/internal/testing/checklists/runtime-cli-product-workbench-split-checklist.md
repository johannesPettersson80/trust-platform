# Runtime CLI Product / Workbench Split Checklist

Status: Planned
Owner: Runtime/dev tooling
Scope: address audit F10 by separating field/product runtime commands from developer/workbench commands.

## Command Variant Classes

- [ ] `RTCLI-CLASS-01` Product runtime commands: `run`, `play`, `ctl`, `validate`, `build`, `hmi`, `plcopen`, `registry`, `setup`, `deploy`, `rollback`.
- [ ] `RTCLI-CLASS-02` UI product commands: `ui`, `ide`, `config-ui`, `wizard` if retained as shipped product surfaces.
- [ ] `RTCLI-CLASS-03` Conformance/benchmark commands: `bench`, `conformance`.
- [ ] `RTCLI-CLASS-04` Workbench/dev command variants: `agent`, `commit`, `docs`, `test` unless explicitly reclassified.
- [ ] `RTCLI-CLASS-05` Shell completion command: `completions`; decide whether it remains product-supporting or moves to tooling.

## Bin Module Classes

These are source modules under `crates/trust-runtime/src/bin/trust-runtime/`, not necessarily CLI enum variants.

- [ ] `RTCLI-MOD-01` Product runtime modules: `run.rs`, `ctl.rs`, `build.rs`, `hmi.rs`, `plcopen.rs`, `registry.rs`, `setup.rs`, `setup_web.rs`, `deploy.rs`.
- [ ] `RTCLI-MOD-02` UI product modules: `config_ui.rs`, `wizard.rs` if retained as shipped product surfaces.
- [ ] `RTCLI-MOD-03` Conformance/benchmark modules: `bench.rs`, `conformance.rs`.
- [ ] `RTCLI-MOD-04` Workbench/dev modules: `agent.rs`, `commit.rs`, `git.rs`, `docs.rs`, `prompt.rs`, `workflow.rs`, `style.rs`, `ci.rs`, `test.rs` unless explicitly reclassified.
- [ ] `RTCLI-MOD-05` CLI infrastructure modules: `cli.rs`, `completions.rs`; classify ownership and allowed dependencies separately.

## Subcommand Action Classes

Nested `*Action` enums inherit their parent command class unless a row below explicitly overrides them.

- [ ] `RTCLI-ACTION-01` Product/runtime action enums: `ControlAction`, `HmiAction`, `PlcopenAction`, `RegistryAction`.
- [ ] `RTCLI-ACTION-02` UI product action enum: `ConfigUiAction` when used through `ui`, `ide`, or `config-ui`.
- [ ] `RTCLI-ACTION-03` Conformance/benchmark action enum: `BenchAction`.
- [ ] `RTCLI-ACTION-04` Workbench/dev action enum: `AgentAction`.
- [ ] `RTCLI-ACTION-05` Any new nested `*Action` enum must either inherit a parent class or declare an explicit override before merge.

## Phase 0 - Full-Map Prerequisite

- [ ] `RTCLI-P0-001` Hard prerequisite: `architecture-doctor --full-map` MVP implements `FULLMAP-CHECK-06` for runtime command, nested action, and bin-module ownership before Phase 2 or any command movement starts.
- [ ] `RTCLI-P0-002` If `FULLMAP-CHECK-06` is unavailable, record an owner-approved waiver with the local replacement rule, fixture, owner, and expiration date.
- [ ] `RTCLI-P0-GATE-01` Do not claim `ARCHPROG-C-01` or `ARCHPROG-C-03` complete until `FULLMAP-CHECK-06` or its waiver is recorded.

## Stop Rules

- [ ] `RTCLI-STOP-01` Do not add a new runtime binary command variant without a command class.
- [ ] `RTCLI-STOP-01A` Do not add a new `src/bin/trust-runtime/*.rs` module without a module class.
- [ ] `RTCLI-STOP-01B` Do not add a new nested `*Action` enum or variant that changes product/workbench ownership without an action class.
- [ ] `RTCLI-STOP-02` Do not let product runtime commands import git/docs/agent/workflow helpers.
- [ ] `RTCLI-STOP-03` Do not move commands without a compatibility/deprecation decision.
- [ ] `RTCLI-STOP-04` Do not use the runtime-core split to hide workbench code behind re-exports.

## Phase 1 - Inventory

- [ ] `RTCLI-P1-001` List every `BenchAction` and main `trust-runtime` command enum variant.
- [ ] `RTCLI-P1-002` Map each command variant to its implementation file.
- [ ] `RTCLI-P1-003` Record line counts for each command implementation file.
- [ ] `RTCLI-P1-004` List every top-level bin module under `crates/trust-runtime/src/bin/trust-runtime/*.rs`.
- [ ] `RTCLI-P1-005` Map each bin module to the command variant, helper, or internal infrastructure that uses it.
- [ ] `RTCLI-P1-006` Identify modules that shell out to git, docs, CI, or source-bundling logic.
- [ ] `RTCLI-P1-007` Identify modules safe for field-deployed runtime artifacts.
- [ ] `RTCLI-P1-008` Record current CLI help output before moving anything.
- [ ] `RTCLI-P1-009` List every nested CLI `*Action` enum and classify each as inherited or override.

## Phase 2 - Policy And Doctor

- [ ] `RTCLI-P2-001` Add command ownership metadata in code or a doctor config.
- [ ] `RTCLI-P2-002` Add a doctor rule failing unclassified command variants.
- [ ] `RTCLI-P2-003` Add bin-module ownership metadata in code or a doctor config.
- [ ] `RTCLI-P2-004` Add a doctor rule failing unclassified bin modules.
- [ ] `RTCLI-P2-005` Add forbidden import checks from product commands/modules to workbench modules.
- [ ] `RTCLI-P2-006` Add a packaging/profile rule if field runtime artifacts must exclude workbench commands/modules.
- [ ] `RTCLI-P2-007` Add a compatibility/deprecation rule for moved commands.
- [ ] `RTCLI-P2-008` Add a doctor rule failing unclassified nested `*Action` enums or action variants with explicit ownership overrides.

## Phase 3 - Target Split

- [ ] `RTCLI-P3-001` Decide destination for workbench commands: `xtask`, `trust-dev`, or another explicit tool binary.
- [ ] `RTCLI-P3-002` Move `agent` implementation out of product runtime binary or wrap it behind a deprecated forwarding alias.
- [ ] `RTCLI-P3-003` Move `commit` command implementation and `git.rs` helper implementation out of product runtime binary or wrap them behind deprecated forwarding aliases.
- [ ] `RTCLI-P3-004` Move `docs` command implementation and `prompt.rs`, `workflow.rs`, `style.rs`, `ci.rs`, and dev-only `test` module implementation as decided.
- [ ] `RTCLI-P3-005` Keep product runtime commands behavior-compatible.
- [ ] `RTCLI-P3-006` Keep benchmark/conformance commands only if explicitly accepted as runtime-adjacent.

## Phase 4 - Tests

- [ ] `RTCLI-P4-001` CLI help snapshot or assertion covers command list/classes.
- [ ] `RTCLI-P4-002` Product command smoke tests still pass.
- [ ] `RTCLI-P4-003` Moved command compatibility wrappers are tested if retained.
- [ ] `RTCLI-P4-004` Doctor rejects a synthetic unclassified command.
- [ ] `RTCLI-P4-005` Doctor rejects a synthetic unclassified bin module.
- [ ] `RTCLI-P4-006` Doctor rejects product command/module import of workbench module.
- [ ] `RTCLI-P4-007` Doctor rejects a synthetic nested `*Action` enum or action variant with no inherited or explicit class.

## Exit Criteria

- [ ] `RTCLI-EXIT-01` Every command is classified.
- [ ] `RTCLI-EXIT-02` Every bin module is classified.
- [ ] `RTCLI-EXIT-03` Product runtime binary no longer silently grows workbench/dev behavior.
- [ ] `RTCLI-EXIT-04` Workbench/dev commands and modules have an explicit home or explicit retained rationale.
- [ ] `RTCLI-EXIT-05` Runtime-core split is not blocked by CLI workbench dependencies.
- [ ] `RTCLI-EXIT-06` Every nested CLI action enum is classified by inheritance or explicit override.

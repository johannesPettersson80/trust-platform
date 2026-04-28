# Full Software Architecture Map Audit - 2026-04-28

Branch: `architecture/full-software-map-audit`
Worktree: `/home/johannes/projects/trust-platform-architecture-map`

This report covers the whole software architecture. Evidence from earlier focused validation runs is used only as generic architecture evidence when it reveals repo-wide tooling or test-strength gaps.

## Scope

Goal: map the full `truST` software architecture with automated tools, identify design weaknesses and silent-bug vectors, and define improvement work that keeps the codebase aligned with SOLID, KISS, and "0 silent bugs".

No source-code refactor is included in this report branch.

## Evidence Collected

Primary artifact root: `target/gate-artifacts/full-software-map-2026-04-28/`.

| Tool | Scope | Result | Artifact |
| --- | --- | --- | --- |
| `cargo metadata --format-version=1` | full dependency resolve | completed | `cargo-metadata-full.json` |
| `cargo metadata --no-deps --format-version=1` | workspace package/target map | completed | `cargo-metadata-no-deps.json` |
| `cargo tree --workspace --edges normal --depth 2` | dependency tree | completed | `cargo-tree-workspace-depth2.txt` |
| `cargo depgraph --workspace-only --all-deps` | workspace dependency graph | completed | `graphs/cargo-depgraph-workspace-only.dot` |
| `cargo modules structure` | all workspace crates | completed | `modules/*.txt` |
| `cargo modules dependencies` | crate internal dependency graph | partial; several large crates emitted empty graphs or tool-noise only | `module-deps/*.dot` |
| `cargo run -p xtask -- architecture-doctor --all` | repo-specific architecture guardrails | pass | `architecture-doctor.log` |
| `python scripts/generate_full_software_map_puml.py` | source-derived full software PUML map | completed | `docs/diagrams/architecture/full-software-map-generated.puml` |
| `scripts/render_diagrams.sh` | render PlantUML outputs and refresh manifest | pass | `docs/diagrams/generated/full-software-map-generated.svg`, `docs/diagrams/manifest.json` |
| `python scripts/check_diagram_drift.py` | diagram drift | pass with no output | `static/check-diagram-drift.txt` |
| `cargo machete --with-metadata` | unused dependency scan | findings present | `security/cargo-machete.txt` |
| `cargo audit --json` | advisory scan | 9 advisories/warnings | `security/cargo-audit.json` |
| `cargo deny check` | supply-chain policy scan | no config; default license policy rejects many dependencies | `security/cargo-deny.txt` |
| `cargo geiger` | unsafe dependency scan | not reliable on this workspace; package-match errors, empty report | `cargo-geiger-trust-runtime.err` |
| Focused mutation evidence | parser/HIR semantic adequacy | syntax strong; HIR weak | `mutants-*.json`, `mutants-*.log` |
| Runtime communications fuzz smoke | runtime comms parsers/protocols | pass | `runtime-comms-fuzz-summary.md` |

The map intentionally records partial and failed tools. A complete architecture system must make those failures reproducible and actionable instead of hiding them.

## Generated PUML Map

The repo now has a generated full-software PlantUML view:

- Source: `docs/diagrams/architecture/full-software-map-generated.puml`
- Rendered SVG: `docs/diagrams/generated/full-software-map-generated.svg`
- Generator: `scripts/generate_full_software_map_puml.py`
- Inputs:
  - `docs/internal/architecture/generated/software-map.json`
  - `target/gate-artifacts/full-software-map-2026-04-28/static/workspace-direct-deps.tsv`
  - `target/gate-artifacts/full-software-map-2026-04-28/static/rust-file-line-counts.tsv`
  - `target/gate-artifacts/full-software-map-2026-04-28/static/largest-rust-files.txt`
  - `target/gate-artifacts/full-software-map-2026-04-28/static/hotspot-summary.tsv`
  - live source scans for `trust-runtime` command variants, root project/runtime artifacts, and `crate::<top_module>` references.

The generated PUML is not a hand-authored architecture opinion. It is a repeatable view over collected facts. Manual judgement stays in this report as findings and proposed refactor work.

## Architecture Snapshot

The high-level package boundary is clean and still matches the intended layering:

- `trust-syntax`: lexer/parser/syntax node classification.
- `trust-hir`: semantic analysis, symbol tables, diagnostics, type checking.
- `trust-ide`: IDE-facing semantic features.
- `trust-lsp`: LSP transport and wiring.
- `trust-runtime`: runtime, compiler/lowering harness, VM, control surfaces, web/HMI/debug/cloud surfaces.
- `trust-debug`: debug adapter/session model.
- `trust-wasm-analysis`: WASM analysis support.
- `xtask`: repo automation, including architecture doctor.

The existing architecture doctor confirms several important source-derived contracts:

- Parser initializer call sites are constrained.
- HIR does not depend on `trust-runtime`.
- Runtime initializer service does not import `SyntaxNode` or `trust_syntax`.
- Runtime initializer service funnel is enforced.
- Normal runtime and VM local initializer paths are checked for FB initializer symmetry.
- Guarded initializer drop patterns are currently absent.
- Diagram drift check passes.

This is good, but the doctor is still feature-specific. It does not yet map the whole system or continuously guard all cross-crate architecture rules.

## Full Source Map

### Workspace Inventory

Cargo metadata reports 8 workspace packages, 176 workspace targets, 618 external registry/path dependencies in the full resolve, and one local path package that is not a workspace member: `third_party/tiverse-mmap`.

| Package | Targets | Target Kinds | Role |
| --- | ---: | --- | --- |
| `trust-syntax` | 14 | `lib`, `test` | lexer, parser, syntax classification |
| `trust-hir` | 13 | `lib`, `test` | semantic analysis, symbols, diagnostics, type checking |
| `trust-ide` | 4 | `lib`, `test` | IDE semantic features |
| `trust-lsp` | 2 | `bin`, `test` | LSP server transport/wiring |
| `trust-runtime` | 138 | `bin`, `lib`, `test` | compiler/lowering harness, VM, runtime, control/web/cloud/HMI/debug surfaces |
| `trust-debug` | 2 | `bin`, `lib` | debug adapter/session model |
| `trust-wasm-analysis` | 2 | `cdylib`, `rlib`, `test` | WASM analysis bridge |
| `xtask` | 1 | `bin` | repo automation and architecture doctor |

### Workspace Dependency Direction

Direct workspace dependencies from metadata:

| From | To |
| --- | --- |
| `trust-hir` | `trust-syntax` |
| `trust-ide` | `trust-hir`, `trust-syntax` |
| `trust-lsp` | `trust-hir`, `trust-ide`, `trust-runtime`, `trust-syntax` |
| `trust-runtime` | `trust-hir`, `trust-ide`, `trust-syntax`, `trust-wasm-analysis` |
| `trust-debug` | `trust-hir`, `trust-runtime` |
| `trust-wasm-analysis` | `trust-hir`, `trust-ide`, `trust-syntax` |

Current layering is mostly one-way from syntax -> HIR -> IDE/LSP/runtime users, but `trust-runtime` depends on `trust-ide` and `trust-wasm-analysis` depends on IDE/HIR/Syntax. These may be intentional, but they should be made explicit in the architecture doctor because they are not obvious from the ideal layering.

### Crate Size Map

| Crate Path | Rust Files | Rust Lines | Test Files | `#[test]` Count |
| --- | ---: | ---: | ---: | ---: |
| `crates/trust-runtime` | 802 | 158,470 | 225 | 1,065 |
| `crates/trust-hir` | 110 | 27,715 | 22 | 368 |
| `crates/trust-lsp` | 89 | 22,357 | 26 | 141 |
| `crates/trust-ide` | 57 | 14,389 | 9 | 99 |
| `crates/trust-debug` | 49 | 10,991 | 0 | 43 |
| `crates/trust-syntax` | 47 | 8,639 | 13 | 142 |
| `crates/trust-wasm-analysis` | 16 | 2,385 | 9 | 23 |
| `xtask` | 1 | 633 | 0 | 0 |

The runtime crate is the dominant architectural risk by size and by number of integration surfaces. That does not mean the crate must be split immediately, but it means runtime must be mapped by subsystem rather than reviewed as one unit.

### Module Map

`cargo modules structure` generated module trees for every workspace crate:

| Package | Module Map Lines | Artifact |
| --- | ---: | --- |
| `trust-runtime` | 1,543 | `modules/trust-runtime.txt` |
| `trust-lsp` | 345 | `modules/trust-lsp.txt` |
| `trust-hir` | 286 | `modules/trust-hir.txt` |
| `trust-ide` | 158 | `modules/trust-ide.txt` |
| `trust-debug` | 78 | `modules/trust-debug.txt` |
| `trust-syntax` | 33 | `modules/trust-syntax.txt` |
| `xtask` | 28 | `modules/xtask.txt` |
| `trust-wasm-analysis` | 2 | `modules/trust-wasm-analysis.txt` |

Major `trust-runtime` top-level modules:

`bundle`, `bundle_builder`, `bundle_template`, `bytecode`, `config`, `control`, `datetime`, `debug`, `discovery`, `error`, `eval`, `execution_backend`, `harness`, `helper_eval`, `historian`, `hmi`, `instance`, `io`, `linux_rt`, `memory`, `mesh`, `metrics`, `numeric`, `opcua`, `plcopen`, `program_model`, `realtime`, `registry`, `retain`, `runtime`, `runtime_cloud`, `scheduler`, `security`, `settings`, `setup`, `simulation`, `stdlib`, `task`, `ui`, `value`, `watchdog`, `web`.

Major `trust-hir` top-level modules:

`db`, `diagnostics`, `ident`, `project`, `symbols`, `type_check`, `types`.

Major `trust-syntax` top-level modules:

`lexer`, `parser`, `syntax`, `token_kinds`.

### Largest Source Files

The largest Rust files are concentrated in runtime VM, web/config UI, LSP handlers, HIR collector/type-checking tests, and debug expression evaluation:

| Lines | File |
| ---: | --- |
| 3,146 | `crates/trust-runtime/src/runtime/vm/register_ir/tests.rs` |
| 2,716 | `crates/trust-runtime/src/runtime/vm/call.rs` |
| 2,539 | `crates/trust-runtime/src/web/config_ui_routes.rs` |
| 1,511 | `crates/trust-runtime/tests/agent_command.rs` |
| 1,458 | `crates/trust-runtime/src/runtime/vm/register_ir.rs` |
| 1,425 | `crates/trust-runtime/tests/bytecode_vm_core.rs` |
| 1,371 | `crates/trust-runtime/src/bin/trust-runtime/agent.rs` |
| 1,279 | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09.rs` |
| 1,224 | `crates/trust-runtime/src/runtime/vm/register_ir/lower.rs` |
| 1,212 | `crates/trust-runtime/src/memory.rs` |
| 1,079 | `crates/trust-runtime/src/value/types.rs` |
| 1,073 | `crates/trust-runtime/src/runtime/vm/register_ir/tier1.rs` |

These files should be treated as architecture hotspots because review-by-reading is unreliable at that size.

### Test Surface Map

Cargo metadata reports 164 integration test targets. Raw source scanning finds 1,881 `#[test]` occurrences across workspace crates. The test surface is broad, but mutation evidence shows breadth is not the same as semantic strength.

The biggest test investment is in `trust-runtime` (225 test files, 1,065 `#[test]` annotations). The main missing piece is not "more tests everywhere"; it is mutation-backed tests for high-risk semantic transformations and architecture guardrails.

### Diagram And Documentation Map

Diagram inventory:

- PlantUML sources: 16
- Diagram drift check: pass/no output
- Key diagram groups:
  - `docs/diagrams/architecture/`
  - `docs/diagrams/debug/`
  - `docs/diagrams/hir/`
  - `docs/diagrams/lsp/`
  - `docs/diagrams/syntax/`

Specification/documentation inventory:

- `docs/specs`: 27 files
- `docs/internal/testing/checklists`: 19 files

The drift check confirms rendered/generated files are fresh. It does not prove semantic correctness of diagram claims against source facts.

### Hotspot Map

Raw grep-based hotspot counts:

| Hotspot Class | Count | Caveat |
| --- | ---: | --- |
| `unsafe` occurrences | 8 | includes tests/comments unless filtered later |
| `unwrap`/`expect`/`panic`/`todo`/`unimplemented` occurrences | 4,507 | includes tests; still useful as a triage map |
| boundary-sensitive markers (`SyntaxNode`, crate imports, initializer/coercion patterns) | 1,315 | raw map for doctor-rule mining |

These are not all bugs. They are map inputs for future automation: production-only filters, allowlists, and subsystem ownership checks.

## Findings

### Framing Correction: Not Classic Spaghetti, But Over-Broad Responsibility Scope

The map does not show pervasive circular dependencies or tangled control flow across the whole workspace. The main language-tooling chain is still understandable:

`trust-syntax -> trust-hir -> trust-ide -> trust-lsp`

The stronger diagnosis is:

- `trust-runtime` has over-broad responsibility scope.
- The production runtime binary includes developer/workbench workflows.
- UI/HMI/control/cloud surfaces overlap in several places.
- The current architecture doctor is too narrow to keep those boundaries from drifting.
- HIR mutation evidence shows semantic tests are too weak in high-risk code.

This is primarily an SRP/KISS and guardrail problem, not proof that the whole codebase is unstructured spaghetti. The remedy is boundary policy, mutation-backed semantic hardening, and measured extraction behind enforced interfaces.

### F1. Architecture doctor is useful but too narrow

Current doctor checks catch the initializer-class drift well, but they are not a complete architecture map. They do not yet enforce all global boundaries, ownership rules, dependency direction, public API surface drift, module-size trends, or high-risk silent-bug surfaces outside the initializer path.

Evidence:

- `xtask` doctor output passes only the currently encoded rules.
- The generated map exists at `docs/internal/architecture/generated/software-map.json`, but the report surface is still limited.

Risk:

- A future feature can preserve the initializer rules while still introducing runtime/HIR/IDE/LSP coupling elsewhere.
- Diagrams can remain syntactically fresh while missing a new source-level ownership drift.

Improvement:

- Extend `cargo xtask architecture-doctor --all` into the repeatable architecture-doctor command for the full repo.
- Add checks for:
  - allowed crate dependency direction,
  - forbidden imports by layer,
  - public API growth by crate,
  - module/function size budgets,
  - source-count guardrails for duplicated classifiers/helpers,
  - diagram claim checks against the generated software map,
  - known silent-bug patterns beyond initializers.

### F2. HIR semantic tests are weak against mutation in high-risk code

Mutation testing showed that the focused HIR semantic slice still lets many mutants survive. This is not proof of bugs by itself, but it is strong evidence that the tests are not pinning important semantics tightly enough.

Evidence:

- Focused HIR mutation rerun: 48 mutants tested, 46 missed, 2 caught.
- Survivors are concentrated in:
  - cross-project type import and initializer translation: `crates/trust-hir/src/db/symbol_import.rs:292`
  - type-check constant evaluation: `crates/trust-hir/src/type_check/const_eval.rs:7`
  - aggregate initializer validation: `crates/trust-hir/src/db/queries/collector/variables.rs:169`

Risk:

- Silent cross-project semantic drift.
- Constant-expression regressions where divide-by-zero, overflow, parens, name refs, unary/binary operations, or enum constants stop being distinguished.
- Aggregate validation regressions where illegal fields, union variants, array repetition, references, or FB member legality can degrade without a failing test.

Improvement:

- Add HIR mutation gates in focused shards:
  - `symbol_import` cross-project type-shape matrix,
  - `type_check::const_eval` expression operator/error matrix,
  - aggregate initializer semantic matrix.
- Require these shards to have zero missed mutants or documented equivalent-value survivors.
- Add source-level architecture guardrails for import/initializer ID translation and const-eval error classification.

### F3. Parser recovery has fragile hand-rolled scanning logic

The syntax mutation slice was ultimately fully killed, but the initial run exposed that parser recovery relies on hand-written lookahead and depth tracking. The current code has unbounded offset scanning and a separate positional recovery loop.

Evidence:

- Current scanner: `crates/trust-syntax/src/parser/grammar/declarations.rs:409`
- Current positional recovery loop: `crates/trust-syntax/src/parser/grammar/declarations.rs:444`
- Focused syntax mutation final run: 54 mutants, 54 caught.

Risk:

- Parser recovery is easy to regress when new tokens or initializer contexts are added.
- Similar scan/recovery logic may be copied instead of centralized.

Improvement:

- Refactor parser recovery helpers into a small reusable bounded scanner API.
- Add property/fuzz tests for parser recovery around nested parentheses/brackets, declaration boundaries, comments, and malformed initializer cascades.
- Add a doctor rule that verifies positional-initializer diagnostics stay targeted and bounded.

### F4. Dependency hygiene is not clean enough for a zero-silent-bug posture

`cargo machete` and `cargo audit` both found actionable dependency hygiene issues.

Evidence:

- `cargo audit`: vulnerabilities/advisories found = true; count = 9; warning classes include `unmaintained` and `unsound`.
- `cargo machete` possible unused dependencies:
  - `trust-runtime`: `home`, `instability`
  - `trust-hir`: `expect-test`, `indexmap`, `thiserror`
  - `trust-syntax`: `expect-test`, `rustc-hash`, `smol_str`, `thiserror`
  - `trust-ide`: `tracing`
- `cargo machete` also reports `third_party/tiverse-mmap/Cargo.toml` believes it is in the workspace while not being a workspace member/exclude.

Risk:

- Supply-chain risk can silently enter release builds.
- Unused dependencies hide stale architecture decisions and increase compile/test surface.
- The `third_party/tiverse-mmap` workspace metadata mismatch breaks generic tooling and undermines repeatable architecture scans.

Improvement:

- Add dependency hygiene to architecture-doctor:
  - `cargo audit` summary with an allow/deny policy,
  - `cargo machete` with explicit ignored false positives,
  - workspace membership/exclude check for `third_party`.
- Decide whether each unused dependency is false positive, dev-only, or removable.
- Fix `third_party/tiverse-mmap` by adding it to `workspace.members`, `workspace.exclude`, or giving it an empty `[workspace]` depending on intended ownership.

### F5. Runtime is broad enough that module mapping alone is insufficient

`trust-runtime` dominates the module map. It includes VM, compiler/lowering harness, web UI, runtime cloud, HMI, debug/control, IO, benchmarking, deployment/bundle, and command surfaces. The module tree is navigable, but too broad to audit manually with confidence.

Evidence:

- `cargo modules structure -p trust-runtime --lib --no-types`: 1,543 lines.
- `trust-runtime`: 802 Rust files, 158,470 Rust lines, 138 cargo targets.
- Hotspot scan over the main source crates produced 1,315 boundary-sensitive matches for markers such as `SyntaxNode`, crate boundary imports, initializer patterns, CST/runtime boundary markers, and direct coercion calls.

Risk:

- A future runtime feature can accidentally bypass intended services because the runtime crate has many integration surfaces.
- Manual diagram review will miss ownership or execution-flow drift.

Improvement:

- Split architecture-doctor runtime checks by subsystem:
  - compiler/lowering,
  - VM execution,
  - retain/restart,
  - debug/control,
  - HMI/web,
  - runtime-cloud/mesh/TLS,
  - benchmark/performance.
- Generate subsystem maps from source and require diagrams to reference those maps.
- Add a public API report for `trust-runtime` so accidental API growth is visible.

### F6. Generated diagrams are still partly hand-trusted

The diagram drift check passes, but the real requirement is stronger: diagrams should be generated from or checked against source facts.

Evidence:

- `architecture-doctor --all` reports diagram drift pass.
- The generated software map is source-derived, but diagram semantic assertions are still limited.

Risk:

- A `.puml` file can be fresh but semantically wrong.

Improvement:

- Add semantic diagram claim checks:
  - every component in selected `.puml` files must map to a crate/module/subsystem in `software-map.json`,
  - every edge labeled as dependency/control/data flow must be backed by source facts or an explicit documented manual edge,
  - fail if diagrams mention a service/module that no longer exists.

### F7. Workspace dependency graph has intentional-looking but undocumented reverse edges

The direct workspace dependency map is mostly layered, but some edges deserve explicit ownership decisions.

Evidence:

- `trust-runtime -> trust-ide`
- `trust-lsp -> trust-runtime`
- `trust-debug -> trust-runtime`
- `trust-wasm-analysis -> trust-ide`
- local non-member path package: `third_party/tiverse-mmap`

Risk:

- If these edges are intentional, they need to be declared as allowed architecture edges.
- If any are accidental convenience dependencies, they can grow into circular product ownership and make future splits harder.

Improvement:

- Add an `allowed-workspace-edges.toml` or equivalent doctor configuration.
- Fail new workspace edges unless they are explicitly reviewed.
- Classify each current edge as `core-layer`, `adapter-layer`, `tooling`, `runtime-integration`, or `temporary`.

### F8. Large-file hotspots need subsystem refactor plans, not cosmetic splits

The largest files are concentrated around VM register IR, VM call execution, web config UI, LSP tests, runtime agent command, runtime memory, runtime value types, and HIR collector/type tests.

Evidence:

- `crates/trust-runtime/src/runtime/vm/register_ir/tests.rs`: 3,146 lines
- `crates/trust-runtime/src/runtime/vm/call.rs`: 2,716 lines
- `crates/trust-runtime/src/web/config_ui_routes.rs`: 2,539 lines
- `crates/trust-runtime/src/runtime/vm/register_ir.rs`: 1,458 lines
- `crates/trust-runtime/src/memory.rs`: 1,212 lines
- `crates/trust-runtime/src/value/types.rs`: 1,079 lines

Risk:

- These files are hard to review and easy to patch locally in a way that violates subsystem boundaries.
- Cosmetic splitting without ownership boundaries would create more files but not better architecture.

Improvement:

- For each file over 1,000 lines, write a short owner/split note before refactoring.
- Split by responsibility and test boundary:
  - VM call dispatch vs FB/class method call semantics vs error mapping,
  - config UI route parsing vs persistence vs response models,
  - value constructors vs validation vs serialization/test helpers,
  - memory layout vs retain vs runtime access.

### F9. Tooling itself is not yet repeatable enough

Several useful tools ran, but not all ran cleanly.

Evidence:

- `cargo modules dependencies` was partial for larger crates; module trees were useful, internal graph output was not consistently useful.
- `cargo geiger` produced package matching errors and no usable report.
- `cargo deny` had no repo policy config and therefore failed under default license rules rather than project-specific rules.

Risk:

- A future engineer can run "architecture checks" and get different answers depending on tool defaults.
- Failed tools can be mistaken for clean tools if only exit codes are summarized.

Improvement:

- Wrap all mapping tools in `xtask architecture-doctor --full-map`.
- Capture tool status as `pass`, `finding`, `partial`, or `failed`.
- Store exact commands, versions, and failure text in generated artifacts.
- Add project-specific configs for deny/audit/geiger or document why a tool is advisory-only.

### F10. Production runtime binary mixes product runtime and developer/workbench tooling

The generated PUML exposes the command split from `crates/trust-runtime/src/bin/trust-runtime/cli/commands.rs`. The binary contains legitimate product/runtime commands, but also workbench/developer commands.

Evidence:

- Product/runtime-adjacent commands include `Run`, `Play`, `Ctl`, `Validate`, `Build`, `Hmi`, `Plcopen`, `Registry`, `Setup`, `Deploy`, `Rollback`, `Bench`, and `Conformance`.
- UI/product commands include `Ui`, `Ide`, `ConfigUi`, and `Wizard`.
- Workbench/developer commands include `Test`, `Docs`, `Agent`, and `Commit`.
- Source files under `crates/trust-runtime/src/bin/trust-runtime/` include `agent.rs`, `ci.rs`, `commit.rs`, `docs.rs`, `git.rs`, `prompt.rs`, `workflow.rs`, and `style.rs` alongside product runtime entry points.
- `agent.rs` is 1,371 lines and imports `trust_runtime::bundle_builder::collect_project_source_files`.
- `commit.rs` shells through the local git helper and stages/commits project changes.

Risk:

- A field-deployed runtime binary ships developer/workbench behaviors that are not runtime execution responsibilities.
- CLI growth can keep pulling project tooling, source processing, git, docs, and agent workflows into the runtime crate.
- This is the clearest SRP violation in the current map.

Improvement:

- Define a product/workbench split:
  - keep product runtime commands in `trust-runtime`,
  - move developer/workbench commands to `xtask`, `trust-dev`, or another explicit tool binary.
- Add an architecture-doctor rule that classifies `trust-runtime` subcommands and fails new unclassified commands.
- Preserve CLI compatibility with deprecated forwarding aliases only if needed, and document the migration.

### F11. HMI, web, UI, control, and runtime-cloud ownership overlaps

The generated PUML and source scan show related UI/control concerns spread across multiple top-level runtime modules.

Evidence:

- `web`: 51 Rust files, 16,694 lines.
- `hmi`: 46 Rust files, 7,123 lines.
- `control`: 39 Rust files, 6,315 lines.
- `ui`: 15 Rust files, 4,938 lines.
- `runtime_cloud`: 8 Rust files, 2,588 lines.
- HMI behavior appears in `src/hmi/`, `src/control/hmi_handlers*.rs`, and `src/web/hmi_ws.rs`.
- Runtime-cloud behavior appears in `src/runtime_cloud/` and `src/web/runtime_cloud_*` / `src/web/runtime_cloud_routes/*` / `src/web/runtime_cloud_state/*`.
- The generated top-level module scan shows `web -> runtime_cloud` references and `web -> control` references, while `control -> web` also exists through pairing/auth support.

Risk:

- Runtime-facing HMI contracts, HTTP routes, browser rendering/scaffolding, write policy, and UI projection do not have one clear owner.
- The current layout makes it easy for UI transport logic and runtime domain logic to grow together.
- The `control -> web` edge is an inversion smell; control should not need web-specific implementation types.

Improvement:

- Define ownership boundaries before extraction:
  - runtime core owns execution state and value access ports,
  - control owns HTTP-neutral request/response contracts and authorization policy,
  - HMI owns schema/contracts/descriptors,
  - web owns transport/routes/assets/websocket serving,
  - runtime-cloud owns cloud contracts/projection and does not own runtime execution.
- Add doctor rules for forbidden internal edges, starting with `control -> web`.
- Extract only after those rules exist, so the split cannot silently drift back.

### F12. The deferred runtime-core/native-host split is the right runtime extraction blueprint

The old embedded/STM32 plan should not be revived as an embedded product commitment. But its core architectural split is directly useful for the current SOLID/KISS problem: `trust-runtime` currently owns portable execution logic and host/product surfaces in one crate.

Evidence:

- `docs/internal/testing/checklists/runtime-core-native-host-split-checklist.md` is marked deferred and explicitly says the broader native-host / MCU plan was parked in favor of the PREEMPT_RT Linux track.
- The same checklist already defines the useful architecture target:
  - one shared ST compiler + bytecode artifact (`program.stbc`),
  - one shared execution core,
  - one rich Linux host,
  - one minimal embedded host profile,
  - behavior locked before ownership changes.
- It also defines the intended first core contents:
  - VM dispatch modules,
  - bytecode container/decode/format/validation needed at runtime,
  - `helper_eval`,
  - `program_model`,
  - `value`,
  - `numeric`,
  - `memory`,
  - `task`,
  - runtime cycle/core execution,
  - scheduler model and core time abstractions,
  - retain policy/snapshot management,
  - portable runtime error types needed by the core.
- The same checklist already states what stays host-side initially:
  - `web`,
  - `hmi`,
  - `control`,
  - `opcua`,
  - `mesh`,
  - `discovery`,
  - `realtime` / `realtime/*`,
  - `runtime_cloud`,
  - `historian`,
  - `ui`,
  - all current `io/*` drivers,
  - `bundle_builder`,
  - `security`,
  - `simulation`,
  - `debug/*`.
- The active PREEMPT_RT checklist explicitly says the current active track keeps the shipped Linux runtime architecture intact and defers STM32H7, Opta, ESP32, embedded T0, embedded EtherCAT, and broader native-host extraction until a later roadmap decision.

Risk:

- If the runtime crate is split only by visible product surface (`web`, `hmi`, `cloud`, `dev tools`) while portable execution remains tangled with host assumptions, the god-crate problem will return under different module names.
- If the old plan is revived without narrowing, the project risks accidentally reopening a large embedded product roadmap instead of fixing the current architecture.
- If extraction happens before behavior locks, the split can introduce silent runtime behavior drift in bytecode execution, retain/warm restart behavior, scheduler ordering, I/O cycle boundaries, or T0 semantics.

Improvement:

- Reuse the old runtime-core/native-host checklist as the blueprint for a behavior-preserving architecture cleanup.
- Explicitly narrow the revived scope:
  - in scope: portable execution core, Linux host boundary, dependency fence, capability interfaces, behavior locks;
  - out of scope until re-approved: STM32H7 product support, Opta acceptance, ESP32 follow-up, embedded T0, embedded EtherCAT, no_std release promises, and MCU protocol commitments.
- Add the split to the main refactor plan after workspace, CLI, and UI/HMI/cloud ownership rules exist.
- Add doctor rules that prevent host-only dependencies from leaking into `trust-runtime-core`.
- Treat the first implementation branch as a Linux behavior-preserving extraction, not a new target-support feature.

## SOLID/KISS Assessment

### Single Responsibility

Mostly acceptable for the language-tooling chain. `trust-runtime` is not acceptable as a long-term single-responsibility boundary: it owns runtime execution, compiler/lowering harnesses, VM internals, control/web/HMI/UI/cloud surfaces, deployment/setup workflows, and workbench/dev commands. The first fix is not blind crate splitting; first encode ownership rules and silent-bug guardrails, then extract behind those enforced boundaries. The old runtime-core/native-host split is the right extraction shape once those rules are in place.

### Open/Closed

Current central classifier and initializer doctor checks are good examples. The weakness is that similar centralization is not yet enforced everywhere. Add source-count guardrails for duplicated classifiers, type-shape imports, lowering funnels, and runtime service bypasses.

### Liskov

The initializer work improved value construction invariants. More broadly, LSP-style substitutability should be checked through type/value invariant tests and mutation gates, especially in `trust-runtime` value and retain/restart paths.

### Interface Segregation

Public APIs are not yet continuously measured. Add `cargo public-api` snapshots for exported crates and review growth as architecture evidence.

### Dependency Inversion

HIR/runtime dependency direction is guarded for the initializer path and currently passes. Expand the same rule across IDE/LSP/runtime/debug boundaries with forbidden-import checks.

### KISS

The codebase has several local KISS wins, but `trust-runtime` size, product/workbench command mixing, overlapping UI/HMI/control/cloud surfaces, large files, and manual parser recovery logic are risk areas. The improvement path is small enforced boundaries and mutation-backed semantic tests before large extraction work. The runtime-core/host split should be used to reduce conceptual load, not to start another broad platform-support track.

## Refactor Work Plan

### Execution Board Map

The runtime-core/Linux-host split is one execution board, not the whole answer to this audit. The full program now has separate checklists for the main risk classes:

| Audit Area | Execution Board | Purpose |
| --- | --- | --- |
| Program coordination | `docs/internal/testing/checklists/full-architecture-refactor-program-checklist.md` | umbrella order, stop rules, and final claims |
| F1/F6/F7/F8 automation prerequisite | `docs/internal/testing/checklists/architecture-doctor-full-map-execution-checklist.md` | repeatable `architecture-doctor --full-map`, edge policy, API/size/diagram checks |
| F2 silent-bug exposure | `docs/internal/testing/checklists/hir-mutation-hardening-execution-checklist.md` | mutation-backed tests for `symbol_import`, `const_eval`, and aggregate validation |
| F3 parser recovery | `docs/internal/testing/checklists/parser-recovery-hardening-execution-checklist.md` | bounded scanner API, targeted diagnostics, fuzz/property coverage |
| F10 runtime binary SRP | `docs/internal/testing/checklists/runtime-cli-product-workbench-split-checklist.md` | separate product runtime commands from workbench/dev commands |
| F11 host surface overlap | `docs/internal/testing/checklists/runtime-host-surface-ownership-checklist.md` | define/enforce `web`, `hmi`, `ui`, `control`, `runtime_cloud` ownership |
| F5/F7 runtime core extraction | `docs/internal/testing/checklists/runtime-core-host-split-execution-checklist.md` | behavior-preserving portable core plus Linux host split |
| F4 dependency hygiene | `docs/internal/testing/checklists/dependency-hygiene-execution-checklist.md` | audit/machete/deny policy and third-party workspace cleanup |
| F8 KISS large-file risk | `docs/internal/testing/checklists/runtime-large-file-split-execution-checklist.md` | owner/split notes and quantitative large-file gates |
| Runtime VM silent-bug risk | `docs/internal/testing/checklists/runtime-vm-mutation-hardening-execution-checklist.md` | mutation-backed VM execution semantic tests |
| Unsafe/concurrency risk | `docs/internal/testing/checklists/unsafe-concurrency-hardening-execution-checklist.md` | unsafe ownership, panic policy, and focused Miri/sanitizer/Loom/Valgrind evidence |

Do not collapse these into one branch. Each board closes a different failure mode.

### Phase 1 - Report and Map Automation

- Add a stable `cargo xtask architecture-doctor --full-map` command.
- Persist a generated summary report under `docs/internal/architecture/generated/reports/`.
- Include cargo metadata, cargo tree, module maps, dependency hygiene, unsafe/hotspot summary, mutation summary, fuzz summary, and diagram semantic checks.
- Add CI artifact upload for the full map report.
- Classify each tool as `pass`, `finding`, `partial`, or `failed`; never collapse partial/failed tools into success.
- Execution board: `docs/internal/testing/checklists/architecture-doctor-full-map-execution-checklist.md`.

### Phase 2 - HIR Silent-Bug Hardening

- Add mutation-backed tests for:
  - cross-project import of every `Type` shape,
  - initializer ID translation for struct/union/type defaults,
  - type-check const eval operators and error variants,
  - aggregate initializer validation for struct/union/array/reference/FB/class targets.
- Gate the HIR focused mutation shard to zero unexplained survivors.
- Treat this as a near-term bug-prevention gate, not a cosmetic architecture task.
- Execution board: `docs/internal/testing/checklists/hir-mutation-hardening-execution-checklist.md`.

### Phase 3 - Workspace Boundary Hardening

- Add an explicit allowed workspace dependency edge policy.
- Classify current non-obvious edges such as `trust-runtime -> trust-ide`, `trust-lsp -> trust-runtime`, and `trust-wasm-analysis -> trust-ide`.
- Fail new workspace edges in the doctor unless the policy is updated with rationale.
- Execution board: `docs/internal/testing/checklists/architecture-doctor-full-map-execution-checklist.md`.

### Phase 4 - Runtime Product/Workbench Boundary

- Classify every `trust-runtime` subcommand as product runtime, UI product, conformance/benchmark, or workbench/dev.
- Move or wrap workbench/dev commands (`Agent`, `Commit`, `Docs`, and potentially `Test`) behind `xtask` or a separate `trust-dev` binary.
- Add a doctor rule so new runtime-binary commands cannot land without a declared ownership class.
- Execution board: `docs/internal/testing/checklists/runtime-cli-product-workbench-split-checklist.md`.

### Phase 5 - Runtime UI/HMI/Cloud Ownership Boundary

- Decide ownership of `web`, `hmi`, `ui`, `control`, and `runtime_cloud` before moving files.
- Add forbidden-edge rules for obvious inversions such as `control -> web`.
- Create ports/interfaces for runtime value access, HMI write policy, runtime snapshots, and cloud projection.
- Only then begin extraction of web/IDE/config UI or runtime-cloud surfaces.
- Execution board: `docs/internal/testing/checklists/runtime-host-surface-ownership-checklist.md`.

### Phase 6 - Runtime Core/Host Split Design Lock

This phase revives only the architecture cleanup part of the deferred runtime-core/native-host plan. It does not reopen embedded product delivery.

Execution checklist: `docs/internal/testing/checklists/runtime-core-host-split-execution-checklist.md`.

This phase is behavior-preserving. It cannot be used as proof that existing HIR semantic risks or parser recovery risks are fixed.

Scope lock:

- In scope:
  - define `trust-runtime-core` ownership,
  - define Linux host ownership,
  - define the dependency fence,
  - define behavior-lock tests,
  - define capability traits needed to keep core execution host-agnostic,
  - define doctor rules that prevent host-only concerns from leaking back into the core.
- Out of scope until a separate roadmap decision:
  - STM32H7 hardware bring-up,
  - Arduino Opta acceptance,
  - ESP32 host follow-up,
  - embedded `T0`,
  - embedded EtherCAT,
  - no_std product promise,
  - Modbus RTU/TCP or MQTT support on MCU targets,
  - any marketing claim for embedded runtime support.

Core ownership:

- `trust-runtime-core` owns only portable runtime execution concerns:
  - bytecode runtime container/decode/format/validation pieces needed after compile,
  - VM dispatch and execution state that does not depend on host services,
  - `program_model`,
  - `value`,
  - `numeric`,
  - `memory` model needed by execution,
  - `task` and scheduler model,
  - core clock/time abstraction,
  - runtime cycle execution,
  - retain snapshot policy and canonical retain data model,
  - portable runtime error types,
  - helper evaluation that is required by runtime execution and not by host tooling.
- `trust-runtime-core` must not own:
  - CLI parsing,
  - project scaffolding,
  - source bundling,
  - git/docs/agent workflows,
  - web routes/assets/websocket serving,
  - HMI rendering/scaffolding,
  - runtime cloud transport,
  - discovery/mesh clients,
  - OPC UA, MQTT, Modbus TCP, EtherCAT, GPIO driver implementations,
  - Linux PREEMPT_RT setup,
  - debug adapter sessions,
  - security policy stores,
  - simulation authoring/UI surfaces.

Linux host ownership:

- The current `trust-runtime` crate remains the Linux host during the first extraction wave.
- It assembles and owns:
  - CLI product commands,
  - launcher/runtime assembly,
  - web/HMI/control transports,
  - discovery/mesh/runtime-cloud integration,
  - realtime/T0 host implementation,
  - IO driver implementations,
  - debug/control surfaces,
  - security/config/setup/deploy surfaces,
  - PREEMPT_RT deployment posture,
  - benchmark/conformance harnesses that run against the assembled host.
- The Linux host consumes `trust-runtime-core`; it must not duplicate core execution logic.

Dependency fence:

- `trust-runtime-core` must not depend on:
  - `tokio`,
  - `zenoh`,
  - `rumqttc`,
  - `rustls`,
  - `tiny_http`,
  - `tungstenite`,
  - `mdns-sd`,
  - `notify`,
  - `opcua`,
  - `ethercrab`,
  - `ureq`,
  - `ratatui`,
  - `crossterm`,
  - `home`,
  - `trust-ide`,
  - `trust-lsp`,
  - `trust-debug`.
- Add a doctor rule or `cargo deny`-style policy that fails any forbidden dependency in the core crate.
- Add a source import check that fails if core code imports host-only modules through `crate::web`, `crate::hmi`, `crate::control`, `crate::runtime_cloud`, `crate::mesh`, `crate::io`, `crate::debug`, `crate::security`, `crate::setup`, or `crate::simulation`.

Capability traits:

- Keep the trait surface synchronous and capability-shaped.
- Required first-pass core-facing traits:
  - monotonic clock: `now`, and only `sleep_until`/`wake` if the scheduler truly needs them,
  - retain storage backend: load/save snapshot through host-owned persistence,
  - watchdog backend: arm/kick/disarm or the smallest equivalent surface,
  - process-image exchange boundary: caller-owned buffers, no hidden host driver ownership,
  - diagnostic sink or event recorder if needed for fault reporting without tying core to HTTP/logging.
- Explicitly avoid:
  - `async fn`,
  - futures,
  - Tokio types,
  - Embassy/RTIC/RTOS-specific executor types,
  - generic `NetworkStack` in the first-pass core,
  - one giant fieldbus trait that pretends EtherCAT, Modbus TCP, MQTT, and GPIO are the same abstraction.

Behavior-lock tests required before moving code:

- Same compiled bytecode + same initial memory + same input image + same retain snapshot + same manual clock sequence must produce:
  - bit-identical output image,
  - identical retain snapshot,
  - identical fault/runtime status.
- I/O cycle boundary tests:
  - all inputs are latched before user logic,
  - outputs are committed only after task execution,
  - no mid-cycle input refresh,
  - multiple drivers still respect pre-read/post-write ordering.
- Retain/warm-start tests:
  - cold start resets non-retain state,
  - warm start restores retain-backed values before user logic,
  - retained state canonicalization remains unchanged.
- Scheduler tests:
  - task interval ordering,
  - equal-time FIFO ordering,
  - priority ordering,
  - overrun accounting does not reorder execution.
- Watchdog/fault tests:
  - timeout path,
  - halt/warn/degrade/restart policy branches where supported,
  - expected snapshot/error contract after fault.
- ABI compatibility test:
  - preserve at least one pre-split `program.stbc` fixture,
  - load it through the post-split runtime,
  - prove identical output/retain behavior.
- Runtime vertical tests remain in the split gate:
  - `cargo test -p trust-runtime --test api_smoke`,
  - `cargo test -p trust-runtime --test debug_control`,
  - `cargo test -p trust-runtime --test complete_program`,
  - `cargo test -p trust-runtime --test runtime_reliability`,
  - `cargo test -p trust-runtime --test realtime_t0_integration` if realtime/T0 ownership is touched.

Doctor rules:

- `trust-runtime-core` dependency fence passes.
- No host-only module imports in the core.
- No product/workbench CLI code imports the core except through approved runtime assembly APIs.
- No web/HMI/cloud/control module bypasses the approved runtime value/snapshot ports.
- No new top-level module is added under `trust-runtime` without a subsystem decision note.
- Public API snapshot for `trust-runtime-core` is reviewed on every change.

### Phase 7 - Runtime Core/Host Extraction Execution

This phase performs the split in small behavior-preserving slices.

Slice 0 - baseline and freeze:

- Add the behavior-lock suite from Phase 6.
- Capture current runtime public API snapshot.
- Capture current runtime subsystem map.
- Add the dependency/import doctor checks in failing/allowlisted mode before code movement.
- Mark embedded-specific old checklist items as parked in the implementation checklist so they cannot be accidentally claimed complete.

Slice 1 - core crate scaffold:

- Add `crates/trust-runtime-core`.
- Add it as a workspace member.
- Start with `std` enabled if that reduces risk, but keep APIs shaped so `no_std + alloc` remains technically possible later.
- Add a crate-level module map:
  - what belongs in core,
  - what is explicitly host-only,
  - which old `trust-runtime` modules are planned for later movement.
- Re-export through `trust-runtime` temporarily so external callers do not change in the first slice.

Slice 2 - pure data/model movement:

- Move the lowest-risk portable modules first:
  - `numeric`,
  - portable `value` pieces,
  - runtime `program_model`,
  - portable bytecode decode/format/validation pieces needed at runtime.
- Keep constructors/invariants intact.
- Add tests that compare pre/post value serialization, equality, retained-state canonicalization, and bytecode validation behavior.

Slice 3 - execution movement:

- Move VM dispatch/execution pieces that do not require host services.
- Keep host assembly and IO driver invocation in `trust-runtime`.
- Add service ports where the VM needs host-facing callbacks instead of importing host modules.
- Split giant files only by responsibility:
  - call dispatch,
  - FB/class method semantics,
  - error mapping,
  - register IR lowering/profile/tier1 internals,
  - test fixtures.

Slice 4 - scheduler/retain/watchdog movement:

- Move scheduler model and core cycle logic only after cycle-boundary behavior locks are green.
- Move retain policy/snapshot model into core while keeping persistence backend host-owned.
- Introduce watchdog capability only at the smallest needed surface.
- Keep Linux PREEMPT_RT posture, `mlockall`, CPU affinity, scheduler policy, and systemd deployment in the Linux host.

Slice 5 - Linux host rewire:

- Rewire current `trust-runtime` to consume `trust-runtime-core`.
- Keep the public CLI/runtime behavior unchanged.
- Keep product commands in the Linux host.
- Keep workbench/dev command migration on the separate product/workbench branch unless it is already complete.
- Confirm web/HMI/control/cloud surfaces access runtime state through approved ports rather than core internals.

Slice 6 - extraction validation:

- Run focused behavior-lock tests three times to check flakes.
- Run runtime vertical tests.
- Run `cargo xtask architecture-doctor --all`.
- Run `cargo xtask architecture-doctor --full-map` once available.
- Run `cargo public-api` or equivalent API snapshot comparison for `trust-runtime-core` and `trust-runtime`.
- Regenerate source-derived maps and diagrams only after the factual checks pass.

Exit criteria:

- `trust-runtime-core` exists and owns portable execution.
- `trust-runtime` Linux host compiles against the core without behavior regressions.
- Dependency/import doctor rules prevent host-only leaks into the core.
- No embedded product support is claimed.
- The old STM32/Opta/no_std/EtherCAT work remains parked with explicit follow-up IDs rather than mixed into this cleanup.

### Phase 8 - Dependency Hygiene

- Fix or document `cargo machete` findings.
- Add `third_party/tiverse-mmap` to the intended workspace policy.
- Add `cargo audit` policy with explicit exceptions and owners.
- Add `cargo deny` policy if not already enforced in CI.
- Add a repo-owned geiger/unsafe scan policy or mark geiger as advisory-only if it remains incompatible with the workspace resolve.
- Execution boards: `docs/internal/testing/checklists/dependency-hygiene-execution-checklist.md` and `docs/internal/testing/checklists/unsafe-concurrency-hardening-execution-checklist.md`.

### Phase 9 - Parser Recovery Hardening

- Refactor positional/aggregate recovery scanning into bounded reusable helpers.
- Add fuzz/property tests for malformed declarations and nested initializer recovery.
- Add doctor checks for targeted parser diagnostics and bounded cascade behavior.
- Execution board: `docs/internal/testing/checklists/parser-recovery-hardening-execution-checklist.md`.

### Phase 10 - Runtime Subsystem Maps And Extraction

- Generate runtime subsystem maps for VM, compiler/lowering, retain, debug/control, HMI/web, cloud/mesh/TLS, and benchmarks.
- Add bypass checks for each subsystem's intended service/funnel.
- Add API-surface snapshots for runtime public modules.
- Create owner/split notes for every Rust file over 1,000 lines before refactoring.
- Execution boards: `docs/internal/testing/checklists/architecture-doctor-full-map-execution-checklist.md`, `docs/internal/testing/checklists/runtime-host-surface-ownership-checklist.md`, `docs/internal/testing/checklists/runtime-core-host-split-execution-checklist.md`, `docs/internal/testing/checklists/runtime-large-file-split-execution-checklist.md`, and `docs/internal/testing/checklists/runtime-vm-mutation-hardening-execution-checklist.md`.

### Phase 11 - Diagram Semantic Enforcement

- Convert existing diagram drift from freshness-only to source-claim checks.
- Fail when a diagram component or edge cannot be linked to code facts or documented manual facts.

## Immediate Next Branch Tasks

This branch should remain report/planning only until reviewed.

Recommended implementation branches after review:

0. `architecture/refactor-program-index`
   - Lands the umbrella execution board and links all sub-checklists.
1. `architecture/full-map-doctor`
   - Implements `xtask architecture-doctor --full-map`.
2. `architecture/hir-mutation-hardening`
   - Adds HIR semantic tests and mutation gate for the 46/48 missed-mutant hotspot.
3. `architecture/workspace-boundary-policy`
   - Adds allowed-edge policy and doctor enforcement.
4. `architecture/runtime-cli-product-workbench-split`
   - Separates runtime product commands from workbench/dev commands or adds compatibility wrappers with ownership rules.
5. `architecture/runtime-ui-hmi-cloud-boundaries`
   - Defines and enforces ownership for `web`, `hmi`, `ui`, `control`, and `runtime_cloud`.
6. `architecture/runtime-core-host-split-design-lock`
   - Revives the old runtime-core/native-host split as a Linux behavior-preserving architecture cleanup, with STM32/Opta/no_std/EtherCAT product work explicitly parked.
7. `architecture/runtime-core-host-extraction`
   - Adds `trust-runtime-core`, moves portable execution slices behind behavior locks, and rewires the Linux host without user-visible behavior changes.
8. `architecture/dependency-hygiene`
   - Handles `cargo audit`, `cargo machete`, `cargo deny`, and workspace policy.
9. `architecture/parser-recovery-hardening`
   - Refactors parser recovery helpers and adds fuzz/property tests.
10. `architecture/runtime-subsystem-maps`
   - Adds runtime subsystem maps and service-bypass checks.
11. `architecture/runtime-large-file-split`
   - Adds owner/split notes and starts high-risk file splits for runtime files over 1,000 lines.
12. `architecture/runtime-vm-mutation-hardening`
   - Adds mutation-backed VM semantic tests for call dispatch, register IR lowering, and tier/deopt behavior.
13. `architecture/unsafe-concurrency-hardening`
   - Adds unsafe-site ownership, panic/unwrap policy, and focused Miri/sanitizer/Loom/Valgrind evidence.

Completion of branch 7 (`runtime-core-host-extraction`) must not be described as completion of the full architecture program. It closes the portable runtime execution boundary only. The HIR mutation, parser recovery, runtime CLI, host surface, dependency hygiene, KISS hotspot, runtime VM mutation, unsafe/concurrency, and diagram semantic gates remain separate until their boards are complete.

## Current Status

- Branch is report-only.
- No implementation refactor is included.
- Full source mapping pass is recorded in this report and in `target/gate-artifacts/full-software-map-2026-04-28/`.
- Current source-level doctor checks pass.
- The remaining work is to turn this mapping into a repeatable `architecture-doctor --full-map` command and then implement the refactor/hardening branches listed above, including the narrowed runtime-core/host split.

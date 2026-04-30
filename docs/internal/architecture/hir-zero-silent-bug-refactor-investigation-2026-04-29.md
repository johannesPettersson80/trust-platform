# HIR Zero-Silent-Bug Refactor Investigation - 2026-04-29

Status: Investigation complete; implementation not started.
Owner: HIR/runtime compiler team
Scope: all HIR architecture paths that can turn a semantic bug into no error, wrong error, wrong symbol/type, skipped declaration, or hidden runtime mismatch.

## Trigger

The OSCAT aggregate experiment exposed namespace/OOP/runtime-lowering problems, but namespace is only the trigger. The real issue is broader: the earlier HIR hardening work closed a scoped mutation board, but it did not prove that HIR as a whole has a zero-silent-bug architecture.

The target for this board is not "fix namespace". The target is: no silent HIR semantic failures. That includes namespace/OOP resolution, broad fallback lookups, typed sentinels such as `TypeId::UNKNOWN` / `SymbolId::UNKNOWN`, diagnostic suppression, duplicated semantic rules, project/import identity, constant scope identity, HIR/runtime declaration parity, and architecture-doctor coverage for future drift.

The zero-silent-error rule still stands: any implementation work from this investigation must start with focused failing tests before a bug fix or refactor is accepted.

## What Was Already Done

The previous HIR mutation board was real progress. It was created to move toward a "0 silent bugs" posture and it closed the mapped audit F2 risk. The board targeted:

- `crates/trust-hir/src/db/symbol_import.rs`,
- `crates/trust-hir/src/type_check/const_eval.rs`,
- `crates/trust-hir/src/db/queries/collector/variables.rs`,
- related tests under `crates/trust-hir/tests/`.

The board records those target files explicitly (`docs/internal/testing/checklists/hir-mutation-hardening-execution-checklist.md:9-14`) and locks cross-project import, const-eval, and aggregate initializer validation phases (`docs/internal/testing/checklists/hir-mutation-hardening-execution-checklist.md:31-82`). The architecture improvements checklist records the final focused mutation result: 185 mutants tested, 178 caught, 7 unviable, 0 missed, and 0 timeouts (`docs/internal/testing/checklists/architecture-improvements.md:62-65`).

That evidence must not be thrown away. But it also must not be overstated. It proves the scoped mutation slices. It does not prove that all HIR semantic architecture paths are protected from silent errors.

## Relation To Existing Audit

The full software-map audit says the intended layering is still `trust-syntax` for parsing, `trust-hir` for semantic analysis/symbol tables/diagnostics/type checking, and `trust-runtime` for lowering/runtime/control surfaces (`docs/internal/architecture/full-software-map-audit-2026-04-28.md:56-79`). It also says test breadth is not semantic strength and that high-risk semantic transformations need mutation-backed tests and architecture guardrails (`docs/internal/architecture/full-software-map-audit-2026-04-28.md:176-180`).

The same audit warned that the architecture doctor was useful but too narrow and needed checks for silent-bug patterns beyond the initial initializer rules (`docs/internal/architecture/full-software-map-audit-2026-04-28.md:232-256`).

Conclusion: the existing audit and closed HIR mutation board justify a larger HIR zero-silent-bug refactor board. Namespace/OOP is only one high-priority symptom.

## Live Architecture Map

The current HIR project query builds a syntax root, builds a project type catalog, precollects constants across project roots, collects symbols for the target file, merges project symbols, resolves pending types, and then runs diagnostics including class/interface checks (`crates/trust-hir/src/db/queries/salsa_backend.rs:355-429`).

`SymbolCollector` owns several independent stores: `SymbolTable`, diagnostics, pending types, parent stack, constant expressions/values keyed by optional scope/name, program instances, project type provider state, importing guard state, and namespace override (`crates/trust-hir/src/db/queries/collector/mod.rs:12-24`). Its project-aware path precollects constants over every project root before collecting the target root (`crates/trust-hir/src/db/queries/collector/mod.rs:57-72`).

The runtime compiler also owns a second semantic walk. It parses sources, asks HIR for diagnostics, then independently walks syntax to lower type declarations, predeclare FB/class/interface declarations, lower globals, lower interfaces/classes/FBs/functions/programs, and register duplicate names (`crates/trust-runtime/src/harness/build.rs:20-235`). That gives runtime lowering a separate declaration-discovery path from HIR.

## Findings

### 1. The completed HIR board was scoped, not global HIR safety

The closed HIR mutation board is important, but it was never a full HIR architecture proof. Its target files and phases are explicit (`docs/internal/testing/checklists/hir-mutation-hardening-execution-checklist.md:9-82`). A full zero-silent-bug claim needs broader semantic ownership rules, resolver outcomes, diagnostic contracts, runtime/HIR parity, and doctor/mutation coverage for the remaining HIR surfaces.

### 2. HIR still has duplicated name/type resolution rules

The diagnostics context has scoped type helpers that try builtin/qualified/scoped resolution and then fall back to broad table lookup (`crates/trust-hir/src/db/diagnostics/context.rs:221-276`).

The type checker has a separate `resolve_type_by_name` with similar builtin/qualified/scoped logic and a final `lookup_type` fallback (`crates/trust-hir/src/type_check/symbol_resolve.rs:92-119`).

OOP interface checks now use scoped resolution for `IMPLEMENTS`, but inherited interface collection still resolves `extends_name` with a broad `symbols.lookup(base_name)` (`crates/trust-hir/src/db/diagnostics/oop/interfaces.rs:38-43`, `crates/trust-hir/src/db/diagnostics/oop/interfaces.rs:131-141`).

This is not only a namespace problem. It is duplicated semantic ownership. If one resolver path is fixed and another is not, silent drift can survive in any feature using the unfixed path.

### 3. Broad fallback lookups are a general silent-error risk

`SymbolTable::resolve` is scope-chain aware and reports USING ambiguity internally as no result (`crates/trust-hir/src/symbols/table.rs:142-162`). But the same table also exposes global/broad lookup helpers: `lookup_any`, `resolve_by_name`, and `lookup_type`-based patterns (`crates/trust-hir/src/symbols/table.rs:311-340`, `crates/trust-hir/src/symbols/table.rs:347-363`).

These broad lookups are not always wrong, but they are dangerous in any context where the semantic question is scoped, kind-specific, project-specific, namespace-sensitive, or diagnostic-bearing. The architecture needs to make fallback explicit and testable instead of letting callers hide it behind `Option`.

### 4. `None` is overloaded in several semantic flows

Some HIR code legitimately suppresses cascaded diagnostics after an earlier error. That is necessary. The architecture problem is when the same `Option::None` shape can also mean unresolved name, ambiguous name, wrong kind, missing source symbol, unsupported syntax shape, or "already reported elsewhere".

The resolver and validation architecture needs explicit outcomes so each caller can intentionally choose between emitting a diagnostic, suppressing a cascade, or escalating an internal invariant failure. Silent bugs happen when those states collapse too early.

### 5. Constant precollection does not encode full semantic scope identity

Constant precollection tracks scope as `Option<SmolStr>` and updates it to only the POU name when entering a POU (`crates/trust-hir/src/db/queries/collector/precollect.rs:65-80`). The collector stores constants under `(Option<SmolStr>, SmolStr)` (`crates/trust-hir/src/db/queries/collector/mod.rs:18-19`).

That key shape cannot distinguish all project/namespace/POU identities. This is one example of the broader rule: HIR semantic keys must encode the full identity they claim to represent.

### 6. Runtime lowering duplicates HIR semantic discovery

Runtime lowering currently discovers POU nodes by walking syntax descendants directly (`crates/trust-runtime/src/harness/compiler/pou/entry_points.rs:1-82`). It also has its own namespace/USING helper that derives implicit namespace chains and explicit `USING` directives (`crates/trust-runtime/src/harness/util.rs:32-78`).

This means HIR can diagnose one semantic world while runtime lowering lowers another. A zero-silent-bug HIR architecture needs a HIR-owned declaration catalog or semantic index that runtime can consume for declaration discovery and parity checks.

### 7. The architecture doctor does not yet guard HIR silent-error classes

The full-map doctor now guards workspace edges, runtime-core fences, command/module ownership, host-surface edges, dependency hygiene, unsafe summaries, KISS thresholds, and diagram claims (`docs/internal/testing/checklists/architecture-doctor-full-map-execution-checklist.md:38-50`, `docs/internal/testing/checklists/architecture-doctor-full-map-execution-checklist.md:83-133`). It does not yet block HIR-specific silent-error patterns such as raw broad lookups, duplicated resolver logic, unclassified diagnostic suppression, or runtime semantic bypasses.

### 8. Typed sentinels are the second `None` overload

The current source tree contains nearly 300 `TypeId::UNKNOWN` occurrences and multiple `SymbolId::UNKNOWN` occurrences under `crates/trust-hir/src`. These sentinels are sometimes legitimate, but they can also hide the same states that `Option::None` hides: unknown name, wrong kind, unsupported shape, earlier error, or internal invariant break.

High-risk examples include call inference returning `TypeId::UNKNOWN` after failing to resolve a call target (`crates/trust-hir/src/type_check/calls.rs:150-184`), expression inference returning `TypeId::UNKNOWN` in many fallback paths (`crates/trust-hir/src/type_check/expr.rs`), standard-function inference returning `TypeId::UNKNOWN` across `type_check/standard/*`, and helper paths such as `direct_address_type` treating unrecognized address forms as `TypeId::UNKNOWN` (`crates/trust-hir/src/type_check/helpers.rs:148-166`).

Conclusion: the semantic kernel must handle typed sentinels, not only `Option`.

### 9. Const-eval still has lossy `Result` to `Option` bridges

`try_eval_const_int_expr` returns a typed `ConstEvalError`, but `eval_const_int_expr` erases that result with `.ok()` (`crates/trust-hir/src/type_check/const_eval.rs:18-22`). Several callers use the `Option` form in statement/type-check/standard expression paths.

This is a concrete silent-collapse pattern. The previous const-eval mutation board locked important error variants, but any caller that uses the lossy bridge must prove that the error was already diagnosed or that the caller intentionally does not require a diagnostic.

### 10. Unsupported syntax shapes need explicit classification

`collect_var_config_block` is currently an empty collector method (`crates/trust-hir/src/db/queries/collector/variables.rs:585`). That may be an intentional non-goal or an unfinished collector path, but under the zero-silent-bug rule it cannot remain an unclassified no-op.

Unsupported syntax shapes must become one of: supported lowering/collection, exact diagnostic, tested cascade suppression, or documented non-goal with a test proving the behavior.

### 11. Collision-bearing return values are discarded

`symbol_import.rs` discards `define_in_scope` results with `let _ = ...` while importing symbols into global or namespace scopes (`crates/trust-hir/src/db/symbol_import.rs:142-183`). If the return value carries collision information, discarding it can make an import collision silent.

The same class applies to map/table `insert` calls that overwrite semantic facts. Collision-bearing returns must either be handled, diagnosed, or allowlisted with a tested invariant.

### 12. Import cycles and missing imported types can silently become `TypeId::UNKNOWN`

`symbol_import::import_type` returns `TypeId::UNKNOWN` for cyclic import, missing source table, and missing source type (`crates/trust-hir/src/db/symbol_import.rs:301-315`). Downstream type-check and validation code commonly suppresses cascades when either side is `TypeId::UNKNOWN`.

That suppression is only safe if a primary diagnostic exists. The architecture needs tests that prove cyclic or missing project import failures produce a primary diagnostic before later checks suppress cascades.

### 13. Resolver duplication is wider than the first namespace finding

Additional resolver-style functions include `is_type_defined_in_scope_with_table`, which repeats qualified/scoped/`lookup_type` fallback behavior (`crates/trust-hir/src/db/diagnostics/globals.rs:24-50`), and collector type resolution paths such as `resolve_project_type_path`, `resolve_type_path`, and `resolve_type_in_scope` (`crates/trust-hir/src/db/queries/collector/types.rs:146-180`, `crates/trust-hir/src/db/queries/collector/types.rs:679-715`).

The migration must explicitly reach these functions. Migrating only files named `symbol_resolve` is not enough.

### 14. HIR has internal duplicate global/external linking behavior

The collector path builds a local `globals` map and inserts `VAR_GLOBAL` facts without duplicate diagnostics (`crates/trust-hir/src/db/queries/collector/validation.rs:35-63`). The project diagnostics path uses a `GlobalKey` map and emits duplicate global diagnostics on collision (`crates/trust-hir/src/db/diagnostics/globals.rs:65-101`).

This is HIR-internal semantic duplication, not a runtime/HIR split. The zero-silent-bug refactor must make those paths one semantic implementation or prove parity with focused tests.

### 15. Public and raw-string APIs can reintroduce the old pattern

`trust-hir` exports `symbols` publicly (`crates/trust-hir/src/lib.rs:37`), and `SymbolTable` exposes broad lookup helpers. `SymbolTable` also stores `extends` and `implements` as raw strings (`crates/trust-hir/src/symbols/table.rs:32-34`), leaving later consumers to re-resolve inheritance references independently.

After migration, broad lookup APIs need narrower visibility or explicit names/contracts, and inheritance references need catalog/semantic-kernel records rather than raw strings that every consumer binds again.

### 16. The OSCAT trigger must become a permanent fixture

The one-compile OSCAT aggregate attempt was the discovery vector for this investigation. It should not remain only a manual retry. A minimal fixture distilled from the original failure must become a focused HIR/runtime regression before the aggregate is retried.

### 17. Alias resolution is duplicated and silently depth-capped

Alias resolution is implemented both on `SymbolTable` and in type-check compatibility (`crates/trust-hir/src/symbols/table.rs:571-585`, `crates/trust-hir/src/type_check/compatibility.rs:5-19`). Both implementations stop after a fixed depth and return the current type instead of producing an explicit cycle or depth-exceeded outcome.

Alias resolution is a semantic operation, not a helper detail. The semantic kernel must own it, and the depth cap must become a diagnostic-bearing outcome or a tested non-error invariant.

### 18. Enum value lookup is ambiguous without scope

`SymbolTable::enum_value_by_name` scans every enum type and returns the first value-name match (`crates/trust-hir/src/symbols/table.rs:503-516`). Because the table is map-backed, two enum types with the same member name can make bare const-eval depend on iteration order. Enum value resolution must be scoped and ambiguity-aware.

### 19. Context lookup failures can silently become global scope

Diagnostic context construction falls back to `PouContext::global` or `ScopeId::GLOBAL` when it cannot find the owner symbol/scope (`crates/trust-hir/src/db/diagnostics/context.rs:133-161`). If this happens inside a method/function/property, later expression/type checks can resolve the wrong names, `THIS`, `SUPER`, or return type without a primary diagnostic.

Every global fallback in diagnostic-bearing context construction must either emit a diagnostic, return an explicit suppression outcome, or be allowlisted with a focused test.

### 20. Symbol insertion policy differs by API

`SymbolTable::add_symbol` writes `global_names` with last-writer-wins semantics while `add_symbol_raw` writes with first-writer-wins semantics (`crates/trust-hir/src/symbols/table.rs:212-237`). The same lookup map therefore has different collision behavior depending on the insertion path. A zero-silent-bug architecture needs one insertion policy or an earlier duplicate diagnostic before lookup state diverges.

### 21. Program configuration collection uses broad fallback and silent wrong-kind behavior

`collect_program_config` tries `resolve_qualified`, then falls back to `lookup_any` for single-segment type names, and silently leaves the program type as `TypeId::UNKNOWN` if the resolved symbol is not a `PROGRAM` (`crates/trust-hir/src/db/queries/collector/collect.rs:331-351`). That is both broad fallback and wrong-kind silence. It belongs in the semantic-kernel migration.

### 22. `Type::Unknown` substitution can hide missing type records

Type compatibility substitutes `Type::Unknown` when referenced element/target types are missing (`crates/trust-hir/src/type_check/compatibility.rs:131-145`). This is the value-level equivalent of a typed sentinel return and must be handled by the same doctor/allowlist policy.

`type_check::compatibility::is_assignable` is the highest-traffic cascade-suppression site for `TypeId::UNKNOWN` (`crates/trust-hir/src/type_check/compatibility.rs:87-89`). It must have focused tests proving every `UNKNOWN`-is-assignable result has an earlier primary diagnostic and does not mask the failure with a wrong-reason cascade.

### 23. Migration needs guardrails before the final fail gate

If the new doctor rules are only added at the end, the long migration window can reintroduce broad lookups or sentinel returns in already-migrated areas. The doctor rules should exist in warn-only/report mode when Phase 3 starts and flip to failing rules when Phase 6 closes.

## Refactor Target

### HIR Semantic Kernel

Create a small HIR semantic kernel that owns name resolution, type resolution, declaration identity, diagnostic-producing outcomes, and project semantic catalog construction. Existing diagnostics/type-check/collector code should call this kernel instead of duplicating semantic rules.

Required shape:

- typed `QualifiedName`, `ScopePath`, and project/source identity types instead of raw strings where semantic identity matters,
- central `NameResolutionOutcome` and `TypeResolutionOutcome` with at least `Resolved`, `Unknown`, `Ambiguous`, `WrongKind`, and `SuppressedCascade`,
- explicit replacement or allowlisting for diagnostic-bearing `TypeId::UNKNOWN` and `SymbolId::UNKNOWN` sentinel returns,
- no namespace-sensitive, kind-sensitive, or project-sensitive broad fallback unless the call site names the fallback mode and has a test,
- one implementation for builtin, qualified, scoped, USING, project import, type-symbol, alias/subrange, enum-value, interface inheritance, and callable/value distinction,
- diagnostic conversion helpers that preserve exact code/message/location decisions.

The migration must include type-check inference, not only resolver helpers. Expression inference, call inference, standard-function inference, literal inference, common type selection, alias/subrange resolution, and any `TypeId::UNKNOWN` return that affects diagnostics belong to this board.

### HIR Declaration And Semantic Catalog

Add a HIR-owned project declaration catalog / semantic index:

- namespace-qualified and project-qualified POU/type/function/interface/program names,
- owner scope and parent namespace,
- source file and source range,
- declaration kind and callable/value/type role,
- interface `EXTENDS` / class or FB `IMPLEMENTS` references as unresolved references first, then resolver outcomes,
- project import and initializer identity mapping,
- duplicate/ambiguous declaration diagnostics with exact locations.

Runtime lowering should consume this catalog for declaration discovery and parity checks. Runtime may still walk syntax to lower bodies, but it should not independently decide which declarations exist.

The catalog must also replace or own raw `EXTENDS` / `IMPLEMENTS` references. A second catalog that shadows `SymbolTable` raw strings would leave the old silent surface in place.

### Explicit Diagnostic Suppression Contract

Replace ambiguous internal `Option` flows where they represent semantic failure with explicit outcomes. It is acceptable for a caller to suppress cascaded diagnostics, but that suppression must be visible in code as a deliberate state, not indistinguishable from "no problem".

### Full Semantic Identity For Keys

Replace HIR semantic maps that use partial identity with typed full identity where required. Constant precollection is the immediate evidence, but the same review must include diagnostics `GlobalKey`, imports, initializer IDs, `TypeId` translation, `SymbolTable::extends` / `implements`, symbol-owner scopes, program instances, `Project::insert_with_id`, `Project::normalize_path`, and project file identity.

### Runtime/HIR Parity Gate

Add a cheap focused test gate that compares the HIR declaration catalog against runtime lowering registration for `PROGRAM`, `FUNCTION`, `FUNCTION_BLOCK`, `CLASS`, `INTERFACE`, global variables, callable symbols, and project imports. This gate is required before trying a one-compile OSCAT aggregate replacement again.

### Architecture Doctor Rules

Extend `architecture-doctor --full-map` with HIR zero-silent-bug checks:

- forbid new raw `lookup_any`, namespace/kind/project-sensitive `lookup_type`, broad `lookup`, and broad `resolve_by_name` uses outside the semantic kernel allowlist,
- fail if diagnostics/type-check/OOP code implements new resolver logic instead of using the semantic kernel,
- fail if semantic functions return ambiguous `Option` for diagnostic-bearing failure states without an explicit suppression type,
- fail if semantic functions return `TypeId::UNKNOWN` or `SymbolId::UNKNOWN` outside the sentinel allowlist,
- fail if HIR semantic code discards collision-bearing return values with `let _ = ...`,
- fail if HIR semantic code substitutes `Type::Unknown` through `unwrap_or(&Type::Unknown)` or equivalent inline fallbacks outside the allowlist,
- fail if the semantic-kernel allowlist exceeds five one-function or one-file-line entries,
- fail if runtime declaration discovery grows new direct syntax-walk ownership after the HIR catalog is available,
- require a failing fixture or known-bad test for each doctor rule, matching the full-map stop rules (`docs/internal/testing/checklists/architecture-doctor-full-map-execution-checklist.md:9-22`).

The semantic-kernel compatibility allowlist must be small enough to audit. The target is at most five entries, each with owner, rationale, review date, and a focused test.

Doctor enforcement should be staged: warn-only when semantic-kernel migration starts, fail when the Phase 6 gates close.

## Test-First Refactor Order

1. Add red tests for the known trigger:
   - sibling namespace interface conformance resolves the local interface,
   - `INTERFACE LocalChild EXTENDS LocalBase` resolves in the declaring namespace,
   - duplicate bare type names across namespaces do not resolve through a global fallback,
   - `USING` ambiguity reports an exact diagnostic, not a silent unknown,
   - constants in same-named POUs under different namespaces do not collide,
   - diagnostics and type checker resolve the same scoped type reference.

2. Add red tests for broader HIR silent-error classes:
   - wrong-kind symbol resolution produces the intended diagnostic instead of unknown/success,
   - ambiguous project/import resolution cannot silently choose one candidate,
   - unsupported syntax shapes in HIR validation are classified as diagnostic, cascade suppression, or explicit non-goal,
   - TypeId and initializer ID translation cannot silently reuse the wrong identity,
   - symbol-owner scope lookup failure cannot be treated as successful validation,
   - diagnostic suppression paths prove the original diagnostic exists,
   - cross-project symbol-name import collision emits a duplicate diagnostic,
   - cyclic cross-project type import emits a primary diagnostic before `TypeId::UNKNOWN` cascade suppression,
   - `eval_const_int_expr` callers do not erase `ConstEvalError` variants silently,
   - `VAR_ACCESS` undefined target and non-callable `FieldExpr` callee paths emit exact diagnostics,
   - `VAR_CONFIG` collection is either implemented or emits a documented unsupported diagnostic,
   - alias-chain depth or cycles produce explicit outcomes instead of silent capped resolution,
   - enum-value lookup is scoped and ambiguous bare enum values produce exact diagnostics,
   - context lookup failures do not fall back to global scope without a diagnostic or explicit suppression,
   - insertion policy for global symbol lookup state is deterministic and duplicate-aware.

3. Add red runtime/HIR parity tests:
   - declarations visible to HIR are visible to runtime lowering,
   - runtime lowering cannot silently skip namespaced or project-imported declarations,
   - catalog/lowering mismatch is an error with a focused diagnostic.

4. Extract the semantic kernel behind existing APIs, then migrate diagnostics/type-check/OOP/collector/import/type-check-inference/alias-resolution call sites one group at a time.

5. Change constant precollection and other semantic maps to full identity keys, and remove or allowlist collision-bearing `let _ = ...` discards.

6. Add the HIR declaration catalog and switch runtime declaration discovery to catalog-driven parity before changing OSCAT aggregation behavior. Distill the OSCAT aggregate trigger into a permanent focused fixture first.

7. Add doctor rules and mutation/fuzz gates after the new architecture exists, not before.

## Focused Validation Cadence

Do not use `just test-all` as the edit loop for this board. Use focused gates until merge/release readiness:

- focused HIR namespace/OOP tests added by the board,
- focused HIR wrong-kind/ambiguous/suppression tests added by the board,
- focused typed-sentinel and lossy-const-eval bridge tests added by the board,
- existing focused import, const-eval, and aggregate initializer tests,
- `cargo test -p trust-runtime --test e2e_multifile namespaced_ -- --nocapture`,
- `cargo test -p trust-runtime --test oscat_oop_examples -- --nocapture` for the fast structural OSCAT gate only,
- focused mutation shards for the semantic kernel and identity maps once implemented,
- `cargo run -p xtask -- architecture-doctor --full-map` after doctor rules are added.

`just fmt`, `just clippy`, and `just test-all` remain final merge/release/board-completion gates under the staged cadence rules.

## Acceptance Criteria

- All diagnostic-bearing HIR semantic decisions go through the semantic kernel or an explicitly documented allowlisted compatibility path.
- Resolver and validation outcomes preserve unknown/ambiguous/wrong-kind/suppressed-cascade distinctions until diagnostics or explicit cascade suppression.
- HIR semantic keys use full project/scope/source identity where the language semantics require it.
- Runtime declaration discovery is catalog/parity driven and cannot silently miss declarations HIR accepted.
- Full-map doctor blocks new HIR broad lookup, duplicated resolver, ambiguous suppression, typed-sentinel, silent-discard, and runtime semantic-bypass paths.
- Focused mutation tests for the semantic kernel and identity maps have zero unexplained survivors or written equivalent-mutant rationale.
- The OSCAT one-compile aggregate is not reintroduced until the HIR/runtime parity tests pass.
- The semantic-kernel compatibility allowlist has at most five entries, and every sentinel/discard allowlist entry has a test and owner-reviewed rationale.

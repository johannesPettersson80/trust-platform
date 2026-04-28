# Issue #51 Root-Cause Architecture Plan

## Purpose

This plan supersedes a narrow "make struct initializers parse" fix. Issue #51 exposes an architecture gap: initializer syntax, semantic typing, default preservation, and runtime materialization do not have one shared contract. The goal is to repair that contract with small, reviewable changes that follow SOLID and KISS, then implement the language behavior on top of it.

No compiler implementation has been started in this branch. This document is the checkpoint to review before coding.

Work location:

- Worktree: `/home/johannes/projects/trust-platform-issue-51`
- Branch: `fix/issue-51-struct-initializers`
- Base: `origin/main` at `dd8c0f276b8f4ec865633681c725b101fccf4931`

## Evidence Reviewed

Standard and repo specs:

- IEC OCR source: `/home/johannes/Downloads/iec-61131-3.ocr.txt`
- `docs/specs/README.md:35-41` says the repo specs are based on IEC 61131-3:2013.
- `docs/specs/02-data-types.md:180-197` documents struct field defaults and named struct aggregate initialization.
- `docs/specs/03-variables.md:7-16` documents variable initializers.
- `docs/specs/03-variables.md:29-38` documents array initialization, struct initialization, and FB instance initialization.
- `docs/specs/10-runtime-semantics.md:272-292` says struct defaults use field type defaults only. That conflicts with type-specific field defaults documented in `02-data-types`.

IEC OCR evidence:

- `/home/johannes/Downloads/iec-61131-3.ocr.txt:2691-2700` says structure components have data-type defaults, component user-defined values can override them, and variable-level structure assignment lists have higher priority.
- `/home/johannes/Downloads/iec-61131-3.ocr.txt:2702-2731` shows `STRUCT` field defaults and variable initialization with `(RANGE := ..., MIN_SCALE := ...)`.
- `/home/johannes/Downloads/iec-61131-3.ocr.txt:3166-3187` defines default initialization priority for variables and says user-defined values in `TYPE` and `VAR` declarations participate.
- `/home/johannes/Downloads/iec-61131-3.ocr.txt:6928-6956` says FB instance declarations are similar to structured variables and may initialize inputs, outputs, or public variables with a parenthesized list after the assignment operator.
- `/home/johannes/Downloads/iec-61131-3.ocr.txt:14776-14867` shows the grammar names for `Array_Init`, `Struct_Init`, `Struct_Elem_Init`, and `Struct_Spec_Init`; OCR noise is present, so exact grammar punctuation must be checked against the PDF/source before implementation.
- `/home/johannes/Downloads/iec-61131-3.ocr.txt:14983-15006` shows variable declarations include `Struct_Var_Decl_Init`, `FB_Decl_Init`, and an FB declaration initializer using `Struct_Init`.

Architecture diagrams:

- `docs/diagrams/syntax/syntax-pipeline.puml:51-59` shows parser lookahead already exists through `peek_kind_n(n)`.
- `docs/diagrams/syntax/syntax-pipeline.puml:75-85` shows parser recovery is a first-class concern.
- `docs/diagrams/syntax/syntax-pipeline.puml:87-94` puts declarations and expressions in separate grammar modules.
- `docs/diagrams/hir/hir-semantics.puml:60-70` shows `SymbolCollector` owns symbol collection, pending types, and constant maps.
- `docs/diagrams/hir/hir-semantics.puml:73-99` shows collection, type resolution, variable validation, and constants are separate HIR phases.
- `docs/diagrams/hir/hir-semantics.puml:288-314` shows the type model stores `Struct { name, fields }` and `Union { name, variants }`.
- `docs/diagrams/hir/hir-semantics.puml:331-349` shows type checking is owned by `TypeChecker` and sub-checkers such as `CallChecker`.
- `docs/diagrams/architecture/system-architecture.puml:276-305` shows the IDE path and execution path both flow through syntax and HIR.
- `docs/diagrams/architecture/runtime-execution.puml:443-460` shows `StructValue::new` is the runtime validation boundary for struct/union identity, field order, missing fields, extra fields, and wrong field types.
- `docs/diagrams/architecture/runtime-execution.puml:494-510` warns that helper evaluation is residual/test/debug/build-time surface and production execution goes through the VM path.

Code evidence:

- `crates/trust-syntax/src/parser/grammar/declarations.rs:35-38` parses TYPE-level defaults after `parse_type_def()` by calling `parse_expression()`, and `:366-370` parses variable declaration initializers the same way.
- `crates/trust-syntax/src/parser/grammar/expressions.rs:230-239` parses `(` as `ParenExpr`; there is no struct initializer branch.
- `crates/trust-syntax/src/syntax/mod.rs:262-266` defines `InitializerList` and `ArrayInitializer`.
- `crates/trust-hir/src/db/queries/collector/variables.rs:5-28` returns only names, type, and address from `extract_var_decl_info`; all other children are dropped in `_ => {}`.
- `crates/trust-hir/src/db/queries/collector/types.rs:235-247` uses that helper for struct fields.
- `crates/trust-hir/src/db/queries/collector/types.rs:253-266` uses that helper for union variants.
- `crates/trust-hir/src/types/defs.rs:210-230` has no default-initializer slot on `StructField` or `UnionVariant`.
- `crates/trust-hir/src/db/queries/collector/variables.rs:79-81` only checks string initializer length; normal declaration initializers are not type-checked as assignments in HIR.
- `crates/trust-hir/src/db/diagnostics/type_check.rs:78-87` type-checks statement lists, not declaration initializers.
- `crates/trust-hir/src/type_check/calls.rs:89-101` emits "not callable" when `Name(args)` does not resolve to a callable target.
- `crates/trust-hir/src/type_check/calls/resolve.rs:49-73` resolves functions, methods, FBs, and FB instances, but not struct constructors.
- Expression-kind classification is copied in more than four places. Confirmed local definitions include `crates/trust-hir/src/db/diagnostics/expression.rs:68-84`, `crates/trust-hir/src/type_check/mod.rs:153-169`, `crates/trust-hir/src/db/diagnostics/unreachable.rs:90-107`, `crates/trust-runtime/src/harness/util.rs:111-129`, `crates/trust-lsp/src/handlers/features/core_impl/helpers/syntax_utils.rs:175-193`, `crates/trust-ide/src/var_decl.rs:130-147`, and `crates/trust-ide/src/refactor/operations/inline_and_namespace_helpers.rs:153-170`. HIR/IDE variants generally exclude aggregate initializer kinds; runtime/LSP include them.
- Statement-kind classification has the same copy-drift shape: `crates/trust-hir/src/type_check/mod.rs:172`, `crates/trust-hir/src/db/diagnostics/unreachable.rs:109`, `crates/trust-runtime/src/harness/util.rs:132`, and `crates/trust-ide/src/refactor/operations/convert_callsite_updates.rs:212` each define a local `is_statement_kind`.
- `crates/trust-hir/src/db/diagnostics/expression.rs:4-46` already has an `ExpressionIndex`, but `ExpressionIndex::from_root` filters through the local HIR `is_expression_kind`; aggregate initializer nodes will not receive expression IDs until classification is centralized and includes `InitializerList` and `ArrayInitializer`.
- `crates/trust-hir/src/diagnostics.rs:23-143` has existing diagnostic codes for this work: `UndefinedVariable` E101, `UndefinedType` E102, `UndefinedFunction` E103, `DuplicateDeclaration` E104, `TypeMismatch` E201, `InvalidOperation` E202, `IncompatibleAssignment` E203, `InvalidArgumentType` E205, and `OutOfRange` E304. There is no existing dedicated `UndefinedField` or `DuplicateField` code.
- HIR and runtime both already have partial constant-expression infrastructure. `crates/trust-hir/src/db/queries/collector/const_eval.rs:23-86` and `crates/trust-hir/src/type_check/const_eval.rs:7-74` evaluate integer constants with checked arithmetic, but both return `Option<i64>` and collapse divide-by-zero, overflow, unsupported expression, and unresolved names into `None`. `crates/trust-runtime/src/helper_eval/const_expr.rs:32-88` evaluates lowered literals, arrays, unary/binary expressions, `SIZEOF`, and named constants, but not struct aggregate initializers.
- `crates/trust-runtime/src/harness/compiler/vars.rs:76-125` can preserve a normal variable initializer.
- `crates/trust-runtime/src/harness/compiler/types.rs:201-220` explicitly discards struct field `_initializer`.
- `crates/trust-runtime/src/harness/compiler/types.rs:135-151` lowers `UnionDef` through the same `lower_struct_def` path and maps fields into `UnionVariant`, so union variant initializers are discarded by the same runtime-side drop.
- `crates/trust-runtime/src/harness/lower/expr/lowering.rs:147-150` rejects `InitializerList` as unsupported.
- `crates/trust-runtime/src/program_model/expr.rs:10-40` has `ArrayInitializer` but no struct/aggregate initializer expression.
- `crates/trust-runtime/src/value/defaults.rs:89-98` builds struct defaults from field type defaults only.
- `crates/trust-runtime/src/value/defaults.rs:112-120` builds union defaults from variant type defaults only.
- `crates/trust-runtime/src/value/types.rs:242-286` confirms `StructValue::new(registry, type_id, fields)` exists and validates field names, missing fields, extra fields, and types. `default_value_for_type_id` currently bypasses that validation through `StructValue::from_canonical_parts(...)`.
- `crates/trust-runtime/src/instance.rs:303-312` evaluates and coerces declaration initializers at runtime startup.
- `crates/trust-runtime/src/helper_eval/const_expr.rs:32-88` has a runtime-side constant evaluator, but it does not support struct aggregate initializers.
- `crates/trust-hir/src/db/queries/collector/validation.rs:49-51` and `:124-129` already detect scalar `VAR_EXTERNAL` initializers at HIR level, and `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs:596-614` already covers the scalar case. Aggregate external initializers still need coverage after aggregate syntax is emitted and centrally classified.
- `STRUCT OVERLAP` is documented in `docs/specs/02-data-types.md:217-231` and IEC OCR `/home/johannes/Downloads/iec-61131-3.ocr.txt:2747-2794`, but the current parser has no `OVERLAP` token or struct-overlap model. That is a pre-existing spec gap; initializer support must not silently enable explicit initialization of overlapped structs when that gap is closed.
- Reference defaults need a precise rule, not a blanket "only NULL" claim. `docs/specs/02-data-types.md:276` says the default initial value is `NULL`, while IEC OCR `/home/johannes/Downloads/iec-61131-3.ocr.txt:2957-2967` shows reference variables can be initialized with `NULL` or `REF(...)`. For type/member defaults, this plan therefore rejects non-constant reference-producing expressions unless a standard-backed constant/default rule is implemented.
- Function-call argument lists already use `:=` for named arguments in `crates/trust-syntax/src/parser/grammar/expressions.rs:326-355`; aggregate initializer parsing must not be reused from that path. Initializer-position declaration/default parsing currently happens from `parse_type_decl()` after `parse_type_def()` at `crates/trust-syntax/src/parser/grammar/declarations.rs:35-38` and from `parse_var_decl()` after `:=` at `:366-370`.
- Pragmas are trivia: `crates/trust-syntax/src/syntax/mod.rs:279-283` classifies `Whitespace`, `LineComment`, `BlockComment`, and `Pragma` as trivia. Central expression/statement classifiers must return false for trivia kinds.
- Existing `VAR_CONFIG` initialization is a concrete separate path today: `crates/trust-runtime/src/harness/compiler/config/globals_access.rs:114-147` lowers `ConfigInit`, `crates/trust-runtime/src/harness/config/config_inits.rs:1-105` applies it and currently calls `coerce_initializer_value_to_type` at `:71`, and `crates/trust-runtime/src/harness/compiler/config/entry.rs:37-59` wires configuration/resource `VAR_CONFIG` blocks into that path.
- Symbol import and project-type loading have concrete copy paths that must carry initializer handles: `crates/trust-hir/src/db/symbol_import.rs:325-345` copies struct fields and union variants, `crates/trust-hir/src/db/queries/collector/types.rs:130-210` imports project data types, and `crates/trust-hir/src/db/queries/salsa_backend.rs:290-295`/`:525-545` provide the Salsa project type provider.
- Runtime benchmark infrastructure exists under `crates/trust-runtime/src/bin/trust-runtime/bench/`. The command implementation uses `include!("bench/...")` from `bench.rs:28-35`, tests are wired with `#[path = "bench/tests.rs"]` at `bench.rs:37-39`, and the CLI already exposes `trust-runtime bench project` with sample/warmup/watch/output options in `crates/trust-runtime/src/bin/trust-runtime/cli/bench.rs:1-24`.
- Runtime FB/class variable initialization currently rejects any initializer on an FB/class typed variable. `crates/trust-runtime/src/instance.rs:285-295` returns `RuntimeError::TypeMismatch` for FB/class instance vars with initializers, `:335-358` does the same for method static locals, and `crates/trust-runtime/src/runtime/vm/local_init.rs:212-239` does the same for VM local/static initialization.
- Runtime struct values are copy-on-write values: `Value::Struct(Arc<StructValue>)` is defined in `crates/trust-runtime/src/value/types.rs:764-808`. Existing mutation paths use `Arc::make_mut`, including `crates/trust-runtime/src/eval/expr/access.rs:188-220`, `crates/trust-runtime/src/helper_eval/storage_lvalue.rs:154-168`, and nested reference writes in `crates/trust-runtime/src/value/reference.rs:151-164`.

Reproductions already performed:

- Named `STRUCT` variable initializer `(cyl := 2, ext := TRUE)` cascades in the parser.
- Type-name initializer `T_Step(cyl := 2, ext := TRUE)` reaches runtime as `PROGRAM init error: type mismatch`.
- Statement assignment `s := T_Step(cyl := 2, ext := TRUE);` reaches HIR as E103 "not callable".
- `TYPE`-level struct field defaults execute as zero/false instead of the declared default.
- `UNION` variant initializers also execute as zero instead of the declared default in the repo implementation.
- `BOOL := 5` is reported at runtime initialization rather than as a HIR semantic diagnostic.

## Diagnosis

This is an architecture issue, not a simple parser bug.

The immediate parser symptom is local: a declaration initializer that begins with `(` goes through the generic expression parser and becomes `ParenExpr` or a parse cascade. The root cause is wider: the project has no canonical initializer/default contract spanning parser, HIR, runtime lowering, value construction, diagnostics, IDE, and VM execution.

The same source-level concept currently has different meanings in different layers:

- Parser: `InitializerList` exists as a syntax kind, but no parser path emits it.
- HIR collection: field initializers can be present in accepted CST, but `extract_var_decl_info` cannot return them and drops unknown children.
- HIR type checking: declaration initializers are mostly not checked against target type; only string length gets a special check.
- HIR calls: `T(...)` is always a callable lookup, so struct constructor syntax cannot be represented.
- Runtime lowerer: array initializers are supported, initializer lists are rejected, and struct field defaults are discarded independently from HIR.
- Runtime default construction: struct/union defaults know only field/variant type defaults.
- IDE/LSP/runtime/HIR helpers disagree on what counts as an expression node, and HIR itself has multiple local expression-kind helpers.

That architecture allowed one accepted construct, `field : INT := 2` inside `STRUCT`, to become a silent wrong value. The implementation did not reject unsupported semantics, did not preserve the initializer in the type model, and did not have a test that forced accepted syntax to reach runtime semantics.

## What We Must Fix

We need one initializer architecture, not several per-feature patches.

The fixed architecture must provide these guarantees:

1. If the parser accepts an initializer syntax kind, each downstream semantic owner must either handle it or emit a targeted diagnostic.
2. Declaration initializers are type-checked in HIR against the declared target type before runtime startup.
3. Type-level field defaults are representable in HIR/runtime metadata.
4. Runtime value construction starts from the correct default chain:
   - elementary default,
   - type-level default,
   - struct/FB member default,
   - variable-level initializer,
   - VAR_CONFIG or later instance-specific override where applicable.
5. All syntax consumers use one central expression, initializer, and statement classification or have explicit, tested deviations.
6. Struct/FB aggregate initialization has one internal model, regardless of surface form.

## SOLID And KISS Constraints

Single Responsibility:

- Parser recognizes syntax and performs bounded recovery only.
- HIR owns semantic typing, target-type checking, name resolution, and user diagnostics.
- Runtime owns value materialization and storage initialization after HIR/lowering have made the initializer shape explicit.
- IDE/LSP consume syntax/HIR contracts; they do not invent their own language classification.

Open/Closed:

- Adding a syntax kind such as `InitializerList` must force explicit consumer decisions through tests or a central helper.
- Adding future aggregate forms should extend one initializer model instead of duplicating array, struct, FB, and config paths.

Liskov/Substitution:

- A declared struct value built from defaults, aggregate initialization, retained state, or deserialization must satisfy the same `StructValue` validation contract.
- VM and bounded helper paths must produce equivalent values for the same initializer where both paths exist. Production cycle execution is VM-only, so there is no separate production interpreter parity target for Issue #51.

Interface Segregation:

- Avoid one massive "compiler utilities" object. Use small interfaces:
  - syntax classification,
  - declaration-part extraction,
  - initializer type checking,
  - initializer materialization,
  - default-value construction.

Dependency Inversion:

- HIR must not depend on `trust-runtime::Value`.
- HIR type metadata must not store raw `SyntaxNode` values inside `StructField` or `UnionVariant`; those structs derive `Eq` and are imported across symbol tables.
- Runtime should consume a lowered/canonical initializer contract, not re-parse semantic meaning from arbitrary CST shapes.

KISS:

- Do not rewrite the full parser or type checker.
- Do not add a VM `STRUCT_INIT` opcode as the first step.
- Do not introduce a second struct-literal model next to initializer lists.
- Do not bundle unrelated OOP inheritance/action audits into this fix.
- Prefer one small initializer service/funnel over changing every caller by hand.

## Target Architecture

### 1. Central Syntax Classification

Add a central API in `trust-syntax`, for example:

- `SyntaxKind::is_expression_node()`
- `SyntaxKind::is_initializer_expression_node()`
- `SyntaxKind::is_aggregate_initializer_node()`
- `SyntaxKind::is_statement_node()`

Lock the set relationships before implementation:

- `is_expression_node()` is the canonical generic expression classifier and intentionally excludes aggregate-only `InitializerList` and `ArrayInitializer`.
- `is_aggregate_initializer_node()` returns true for exactly `InitializerList` and `ArrayInitializer`.
- `is_initializer_expression_node()` returns true for `is_expression_node() || is_aggregate_initializer_node()`. This includes ordinary initializer RHS syntax such as literals, `NameRef`, `CallExpr`, and `SizeOfExpr` through the generic expression classifier, while semantic checks decide which of those are valid in a given initializer context.
- `is_statement_node()` covers statement syntax nodes only and is not a fallback expression classifier.

Consumers in HIR, runtime harness, IDE, and LSP should call this helper or wrap it with a named reason for divergence.

Acceptance:

- A parity test fails if HIR/runtime/LSP/IDE expression-kind lists drift.
- A parity test fails if HIR/runtime/IDE statement-kind lists drift.
- `InitializerList` and `ArrayInitializer` have one source of truth.
- Unit tests assert the classifier set relationships above, including that aggregate initializer nodes are not generic expression nodes but are initializer expression nodes.
- `ExpressionIndex::from_root` produces IDs for `InitializerList` and `ArrayInitializer` after aggregate parsing exists.
- The central classifiers return false for every trivia kind; add a unit test that asserts no `SyntaxKind` can be both trivia and expression/statement/initializer.
- Consumers that deliberately exclude a kind must name that exclusion in code and tests.

This fixes the structural drift that currently exists between HIR, runtime, LSP, and IDE helpers.

The guardrail must enumerate all local `fn is_expression_kind` and `fn is_statement_kind` definitions with `rg` and either remove them, delegate them to the central helper, or document a tested reason for divergence. It is not enough to cover only one helper per crate.

### 2. Declaration Parts Instead Of Tuple Extraction

Replace tuple-shaped declaration extraction with explicit declaration parts.

HIR side:

```rust
struct VarDeclParts {
    names: Vec<(SmolStr, TextRange)>,
    type_id: TypeId,
    direct_address: Option<SmolStr>,
    initializer: Option<InitializerSyntaxRef>,
}
```

Runtime harness side should get the same structural treatment for `parse_var_decl(...)`, because `crates/trust-runtime/src/harness/compiler/types.rs:210` currently destructures and discards `_initializer`.

The exact names can change, but the architecture must have a slot for initializer syntax or an initializer handle. The return type change should be structural: old tuple destructures must stop compiling.

Rules:

- No silent `_ => {}` for initializer-capable declaration children.
- Unknown children in a declaration either have an explicit ignore reason or produce a diagnostic.
- Struct field collection, union variant collection, normal variable collection, global/external validation, and runtime type lowering all use declaration parts.

This makes it impossible for field defaults to be accepted by the parser and erased before HIR or runtime lowering can decide.

### 3. HIR Initializer Catalog

Do not store raw `SyntaxNode` in `StructField` or `UnionVariant`. Do not store `trust-runtime::Value` in HIR.

Introduce a small HIR-side initializer catalog:

```rust
struct InitializerId(u32);

struct InitializerRecord {
    owner: InitializerOwner,
    target_type: TypeId,
    source_range: TextRange,
    kind: InitializerKind,
}

enum InitializerOwner {
    Variable(SymbolId),
    StructField { type_id: TypeId, field: SmolStr },
    UnionVariant { type_id: TypeId, variant: SmolStr },
    FunctionBlockMember { owner: SymbolId, member: SmolStr },
    VarConfigOverride { instance_path: SmolStr },
}
```

Then add only a lightweight handle to type members:

```rust
pub struct StructField {
    pub name: SmolStr,
    pub type_id: TypeId,
    pub address: Option<SmolStr>,
    pub default_initializer: Option<InitializerId>,
}
```

Same principle for repo-supported `UnionVariant` defaults.

The catalog can initially keep a source locator plus typed initializer kind. If implementation shows a runtime-independent `ConstValue` is needed, add it inside HIR as a small enum, not as runtime `Value`.

Before adding a new parallel store, Phase 3 must inspect and either reuse or intentionally bypass the existing HIR expression/constant infrastructure:

- `docs/diagrams/hir/hir-semantics.puml:60-70` shows `SymbolCollector` already has `const_exprs` and `const_values`.
- `crates/trust-hir/src/db/diagnostics/expression.rs:4-46` provides `ExpressionIndex` with `id_for_range`, `range_key_for_id`, and `id_at_offset`.
- `crates/trust-hir/src/db/diagnostics/type_check.rs:91-92` records expression types after type checking.

Preferred design: make `InitializerId` a thin handle over the existing expression index or another existing HIR expression key when that is sufficient. If a new initializer catalog is still the cleanest option, the plan must state why these existing tables are not enough and must add a guardrail that aggregate initializer nodes receive IDs.

The catalog must be Salsa-compatible:

- Phase 3 should use the simplest tracked design first: store the initializer catalog on `SymbolTable` itself, next to the existing type and constant maps, so it is returned by the existing Salsa-tracked `file_symbols_query`/project symbol table path. Add a sibling tracked output only if implementation proves the `SymbolTable` field is too invasive.
- It must not be an ad hoc mutable side table outside the tracked symbol/analysis result.
- Editing one field default must recompute the owning file's catalog and change that field's record/value; sibling field records must remain semantically unchanged and must not be remapped to the wrong initializer.
- If IDs are derived from source order, reordering fields must not let downstream consumers reuse stale IDs. Prefer an owner/range-backed key or an import-time translation step over raw source-order identity where that matters.

Copy/import owners that must explicitly preserve initializer handles:

- `crates/trust-hir/src/db/symbol_import.rs`, especially struct/union field copying in `import_type(...)`;
- `crates/trust-hir/src/db/queries/collector/types.rs`, especially project data type import through `import_project_data_type(...)` and `register_imported_data_type(...)`;
- `crates/trust-hir/src/db/queries/salsa_backend.rs`, where `SalsaProjectTypeProvider` loads project type declarations for collection.

`InitializerId` is file/project-local. Cross-project import must not copy a raw `InitializerId` from the source table into the target table. Import must either copy the source `InitializerRecord` into the target catalog and emit a fresh target-local ID, or translate through a per-import ID map. The simpler first design is to copy the record into the target catalog with translated type IDs and owner metadata.

Why this shape:

- `StructField` remains cloneable and comparable.
- Symbol import can copy handles and records intentionally.
- HIR remains independent from runtime.
- Runtime gets a clear catalog to consume when materializing defaults.

### 4. HIR Type Checking For Declaration Initializers

Add one HIR entry point:

```rust
check_decl_initializer(target_type, initializer, context) -> Result<TypedInitializer, Diagnostic>
```

It must cover:

- normal variable declarations,
- global declarations,
- struct field defaults,
- union variant defaults if retained,
- FB instance declarations,
- array element initializers,
- nested aggregate initializers,
- statement expressions that use `TypeName(...)`.

Rules:

- `BOOL := 5` and similar scalar mismatches are HIR diagnostics, not runtime startup errors.
- Unknown aggregate fields are diagnostics.
- Duplicate aggregate fields are diagnostics.
- Field type mismatches are diagnostics.
- Missing fields are allowed when defaults can fill them.
- Non-constant defaults in `TYPE` field declarations are diagnostics.
- `VAR_EXTERNAL` initializers remain invalid per the IEC OCR evidence at `/home/johannes/Downloads/iec-61131-3.ocr.txt:3187`.
- `STRUCT OVERLAP` initializers remain invalid per IEC OCR `/home/johannes/Downloads/iec-61131-3.ocr.txt:2791-2794` and repo spec `docs/specs/02-data-types.md:227-231`. Because `OVERLAP` is not currently parsed as a first-class construct, the implementation must either add the missing overlap model or record a separate pre-existing gap; it must not silently initialize overlapped structs.

Constant-expression scope for `TYPE` member defaults:

- Allowed: literals and typed literals, enum literals, `NULL` for reference-like targets where assignment compatibility allows it, named compile-time constants, unary and binary operators over constants, `SIZEOF` type/static operands already supported by the HIR evaluator, and aggregate initializers whose nested values are all constant expressions.
- Disallowed: ordinary variable references, function/method/FB calls other than built-in constant forms such as `SIZEOF`, subscript/field/deref expressions over runtime storage, side-effectful expressions, and aggregate values that depend on runtime storage.
- Boundary failures are diagnostics, not silent fallback values: divide by zero, integer overflow, out-of-range subrange values, implicit narrowing such as `SINT := 200`, and unresolved names in defaults must be reported at HIR level.
- Cyclic constant/default references are diagnostics, not recursion, stack overflow, panic, or silent fallback. Use `CyclicDependency` E305 for a required default that transitively references itself through constants or type/member defaults.
- The review claim that NaN must be a mandatory boundary test is not accepted as-is: add a NaN/Inf test only if the language has a concrete NaN/Inf literal or constant surface. Otherwise record it as out of scope for this initializer fix.

Diagnostic-code matrix:

| Rule | DiagnosticCode |
|------|----------------|
| Scalar declaration initializer type mismatch | `TypeMismatch` E201 |
| Aggregate initializer against non-struct/non-union/non-FB target | `TypeMismatch` E201 |
| Unknown aggregate field/member | Add `UndefinedField` E107 in Phase 0 and use it here |
| Duplicate aggregate field/member | Add `DuplicateField` E108 in Phase 0 and use it here |
| Aggregate field value type mismatch or enum type mismatch | `TypeMismatch` E201 |
| STRING/WSTRING literal exceeds declared length in any variable initializer or type/member default | `OutOfRange` E304 |
| Subrange violation or literal outside target range | `OutOfRange` E304 |
| Non-constant default where constant expression is required | `InvalidOperation` E202 |
| Divide by zero or overflow while evaluating a required constant default | `InvalidOperation` E202, or `OutOfRange` E304 when the failure is specifically target-range overflow |
| Cyclic constant/default reference while evaluating a required default | `CyclicDependency` E305 |
| `VAR_EXTERNAL` initializer, scalar or aggregate | `InvalidOperation` E202 |
| Explicit initializer for `STRUCT OVERLAP` | `InvalidOperation` E202 |
| FB aggregate member targets `VAR_IN_OUT`, private, temporary, external, or otherwise non-initializable member | `InvalidOperation` E202 with stable wording such as `function block aggregate initialization can target only VAR_INPUT, VAR_OUTPUT, or public VAR members` |
| Positional aggregate initializer when unsupported | syntax diagnostic, then HIR must not cascade |
| `T(...)` where `T` is a struct/union/FB initializer target | no `UndefinedFunction` E103 |
| `T(...)` where `T` is a class type and class aggregate initialization is out of issue #51 scope | `InvalidOperation` E202 with stable wording such as `class types do not support aggregate initialization; use NEW T(...) for class instantiation` |
| `T(...)` where `T` is neither callable nor initializer target | existing callable diagnostic, `UndefinedFunction` E103 or current resolver equivalent |

Phase 0 must add `UndefinedField` and `DuplicateField` to `DiagnosticCode` before the diagnostic-code tests are written. Reusing `UndefinedVariable` or `DuplicateDeclaration` would work mechanically, but it would blur IDE/LSP categories and create message drift for users who named a missing or repeated aggregate member rather than a variable or declaration.

Constant-evaluator strategy:

- Do not rely on the existing `Option<i64>` evaluators for required default diagnostics.
- Add a declared-default constant evaluator, or a fallible wrapper around the existing evaluator internals, that returns `Result<ConstValue, ConstEvalError>`.
- The fallible evaluator must distinguish at least: not constant, undefined name, divide by zero, integer overflow, negative exponent/unsupported operator, cyclic constant/default reference, and target range violation.
- Existing `eval_int_expr(...)` and `eval_const_int_expr(...)` may remain `Option<i64>` for legacy array-bound/subrange callers during this issue, but default evaluation must use the fallible path.
- Diagnostic mapping from `ConstEvalError` to `DiagnosticCode` is part of the Phase 4 test contract.

This makes the IDE path and execution path share the same semantic contract.

### 5. One Aggregate Initializer Model

Use one internal aggregate initializer representation for:

- bare expected-type form: `(field := value, ...)`,
- type-name form: `T(field := value, ...)`,
- nested struct values,
- array elements whose element type is a struct,
- FB instance declaration initialization where IEC/repo specs require it.

Surface parsing can differ:

- `(field := value)` is parsed only in initializer/expected-type positions.
- `T(field := value)` may remain a `CallExpr` in CST and be reclassified in HIR when `T` resolves to a struct, union, or FB type.

Internal representation should include:

```rust
AggregateInitializer {
    target_type: TypeId,
    fields: IndexMap<SmolStr, TypedInitializer>,
    source_range: TextRange,
}
```

The runtime can materialize this as:

1. construct default value for the target type using the initializer catalog,
2. evaluate each override,
3. validate the final value with the existing struct/union validation contract.

### 6. Runtime Initializer Materialization Funnel

Introduce a runtime-side initializer service/module rather than spreading new logic across all call sites.

Possible module:

- `crates/trust-runtime/src/harness/initializer.rs`

Possible responsibilities:

```rust
struct InitializerContext<'a> {
    registry: &'a TypeRegistry,
    profile: &'a DateTimeProfile,
    initializer_catalog: &'a InitializerCatalog,
    const_resolver: &'a dyn ConstResolver,
}

fn default_for_type(type_id, ctx) -> Result<Value, InitError>;
fn evaluate_initializer(target_type, initializer, ctx) -> Result<Value, InitError>;
fn apply_aggregate_overrides(base, aggregate, ctx) -> Result<Value, InitError>;
```

Keep `default_value_for_type_id` as the primitive/type-default helper if useful, but do not make it responsible for every source-level initializer rule. It can be called by the initializer service for the elementary baseline.

All initialization call sites should route through the service:

- program globals,
- program locals,
- function block/class vars,
- VM local/static init,
- array element initialization,
- FB instance initialization,
- retained/default value fallback where appropriate.

Concrete current call sites to audit or route:

- `crates/trust-runtime/src/instance.rs:217`, `:285-295`, `:311`, `:335-358`, `:400`
- `crates/trust-runtime/src/harness/build.rs:415`
- `crates/trust-runtime/src/harness/config/globals.rs:88`
- `crates/trust-runtime/src/harness/config/config_inits.rs:71`
- `crates/trust-runtime/src/harness/compiler/config/globals_access.rs:23`
- `crates/trust-runtime/src/harness/compiler/pou/class_vars.rs:28`
- `crates/trust-runtime/src/harness/compiler/pou/program_vars.rs:35`
- `crates/trust-runtime/src/harness/compiler/pou/function_block_vars.rs:34`
- `crates/trust-runtime/src/harness/compiler/pou/function_vars.rs:34`
- `crates/trust-runtime/src/runtime/vm/local_init.rs:191`, `:212-239`, `:270`

The recursive calls inside `crates/trust-runtime/src/harness/coerce.rs:79` and `:84` are representation coercion internals; they may remain inside the coercion helper, but source-level initializer validity must be decided before reaching them.

Runtime type lowering must also stop reparsing and discarding type-member initializers. `crates/trust-runtime/src/harness/compiler/types.rs:201-220` and the `UnionDef` branch at `:135-151` must consume the HIR initializer/default contract or equivalent lowered metadata rather than raw CST tuple destructures.

`StructValue::new(registry, type_id, fields)` exists and matches the runtime diagram. Default construction currently uses `from_canonical_parts(...)`; after this work, either use the validating constructor for aggregate/default materialization or document the invariant that makes the canonical bypass safe for internally assembled values.

Do not add a VM `STRUCT_INIT` opcode in the first implementation slice. For compile-time/default-only initializers, prefer precomputed canonical values. For runtime aggregate expressions whose field values depend on runtime state, lower through a default-plus-overrides sequence or a shared runtime helper with differential tests; do not silently route only the interpreter path.

Acceptance:

- There is one semantic path for "construct this declared value".
- Runtime coercion remains a representation conversion guard, not the first source-level type checker.
- Initialization diagnostics are source-aware before runtime where possible.

### 7. Parser Work

Parser changes should be narrow:

- Add a declaration-initializer parser entry point instead of always calling plain `parse_expression()`.
- Use existing `peek_kind_n(n)` to detect `LParen Name Assign` in initializer position. Named detection and positional rejection are different parser paths: named detection can use lookahead; positional rejection needs the initializer-position paren branch to commit and inspect the first parsed child.
- Emit `InitializerList` for `(field := value, ...)`.
- Keep `TypeName(...)` as `CallExpr` in CST; HIR has type information and resolves whether it is a constructor or callable.
- Add bounded recovery for malformed aggregate initializers. One malformed initializer should not corrupt the rest of the `VAR` block.
- Reject positional `(2, TRUE)` with one targeted diagnostic. IEC OCR `/home/johannes/Downloads/iec-61131-3.ocr.txt:14865-14867` shows `Struct_Init` elements are named `Struct_Elem_Name := ...`; no positional production was found.

KISS rule:

- Do not make the general expression parser context-sensitive except through a small initializer-position entry point.

### 8. FB Instance Initializers

IEC OCR and repo specs both put FB instance initialization in scope:

- IEC OCR: `/home/johannes/Downloads/iec-61131-3.ocr.txt:6931-6942`
- Repo spec: `docs/specs/03-variables.md:37-38`

Therefore the architecture must not build a struct-only initializer subsystem that cannot support:

```st
Timer : TON := (PT := T#1s);
```

Implementation can phase FB initialization after struct field defaults and struct variable initializers, but the data model must already allow aggregate overrides against a target whose members are FB inputs, outputs, or public variables.

Locked rule for the plan:

- FB instance initializer targets are inputs, outputs, or public variables, matching IEC OCR `/home/johannes/Downloads/iec-61131-3.ocr.txt:6936-6938`, unless implementation discovers a repo-specific accessibility decision that must be recorded in `docs/IEC_DECISIONS.md`.

### 9. Union Variant Defaults

IEC OCR search did not show a `UNION` construct in the IEC text. The repo supports `UNION` as a language feature, so this plan locks the repo decision:

- Support `UNION` variant defaults through the same initializer/default architecture used for `STRUCT` fields.
- Add an entry to `docs/IEC_DEVIATIONS.md` documenting `UNION ... END_UNION` as a truST/vendor extension and stating that variant defaults are supported by the initializer catalog.

The current behavior, accepting the syntax and silently using zero/default values, is not acceptable.

Do not claim union defaults are IEC-required unless the source standard text is verified to contain them.

### 10. Positional Struct Initializers

Do not implement positional `(2, TRUE)` as part of this fix.

Reasons:

- The repo specs document named field initialization, not positional struct initialization.
- IEC OCR `/home/johannes/Downloads/iec-61131-3.ocr.txt:14865-14867` shows named `Struct_Elem_Init`; no positional production was found.
- Positional values are brittle under field reordering.
- `(2)` is already a normal parenthesized expression shape.
- Supporting it would add context-sensitive parsing and semantic reinterpretation that is not needed for issue #51.

Required behavior if unsupported:

- Emit one targeted diagnostic:
  `positional struct initializers are not supported; use named field initializers`
- Recover to the closing `)` or declaration terminator.
- Do not cascade through the rest of the `VAR` block.

Rejecting positional form is IEC-conformant based on the reviewed OCR and does not need a deviation entry. Add a deviation entry only if truST later decides to support positional struct initialization as a vendor extension.

## Implementation Phases

### Phase 0 - Spec And Design Lock

Deliverables:

- Verify exact IEC grammar and wording against the source PDF/text or reviewed OCR for:
  - structured data type initialization,
  - variable declaration initialization,
  - FB instance initialization,
  - positional initializer status,
  - `VAR_EXTERNAL` initializer prohibition.
- Lock these decisions before code:
  - named struct initialization is required;
  - positional struct initialization is rejected as unsupported and not IEC-required by the reviewed OCR;
  - FB instance initialization uses the same aggregate initializer model and targets inputs, outputs, or public variables;
  - `UNION` is a repo/vendor extension and variant defaults are supported through the same initializer architecture;
  - `STRUCT OVERLAP` explicit initialization is invalid, but current parser support for `OVERLAP` is a separate pre-existing gap;
  - enum explicit values are out of issue scope because HIR/runtime already preserve them through `extract_enum_value` and `lower_enum_def`;
  - TYPE-level defaults cover every declaration default parsed by `parse_type_decl()` after `parse_type_def()`, including alias/simple type defaults, struct type defaults with aggregate syntax, scalar array defaults, and array-of-struct defaults;
  - central syntax classifier membership is locked: aggregate initializer nodes are exactly `InitializerList` and `ArrayInitializer`; generic expression nodes exclude those two; initializer-expression nodes are the union of generic expressions plus aggregate initializers;
  - add `DiagnosticCode::UndefinedField` as E107 and `DiagnosticCode::DuplicateField` as E108, then use them for aggregate member diagnostics;
  - class aggregate initialization is out of issue #51 scope, and `T(...)` where `T` is a class type reports `InvalidOperation` E202 with stable message wording rather than falling through to `UndefinedFunction` or silently acting like a struct;
  - cyclic constant/default references produce `CyclicDependency` E305;
  - FB aggregate initialization rejects `VAR_IN_OUT` members with `InvalidOperation` E202 because IEC OCR `6936-6938` lists only inputs, outputs, and public variables as legal initializer targets;
  - multi-name variable declarations share the initializer semantically: `VAR a, b, c : T := (...); END_VAR` materializes independent values for every declared name. Applying the initializer to only the first name is a silent error;
  - multi-name materialization evaluates and type-checks the source initializer once per declaration, then materializes independent storage for each declared name. Scalar, aggregate, and `REF(...)` initializer values may be cloned when that preserves the same source semantics; FB/class instance handles must not be cloned as shared runtime identities, and each declared instance must receive independent storage with the same validated initializer contract;
  - array repetition over aggregate elements is supported for issue #51, for example `[3((a := 1))]` repeats the aggregate element initializer three times. If implementation discovers an existing parser constraint that makes this impossible in the first slice, it must emit a targeted diagnostic and record the narrower support in specs before merging;
  - initializer catalog ownership is locked to the existing Salsa-tracked `SymbolTable` first, not a free-floating side table. A sibling tracked catalog is allowed only with an explicit implementation note explaining why a `SymbolTable` field is worse;
  - the `SymbolTable` catalog shape stays narrow: add a private or `pub(crate)` `initializer_catalog` field plus small accessors such as `initializer(id) -> Option<&InitializerRecord>` and owner/range lookup helpers. Do not expose a broad mutable catalog API;
  - initializer diagnostics use the code matrix in Phase 4, so IDE/LSP/runtime tests assert stable error categories;
  - constant-expression scope for type/member defaults is the Phase 4 subset; anything outside it is rejected with a diagnostic instead of evaluated opportunistically;
  - default/aggregate expansion has bounded recursion. Cycle detection handles semantic cycles; a separate depth cap, initially aligned with the parser's existing recursion-limit principle, prevents pathological nested defaults from blowing up without a diagnostic;
  - default constant evaluation uses a fallible `Result<_, ConstEvalError>` path, not the existing `Option<i64>` evaluators that collapse overflow/divide-by-zero into `None`;
  - STRING/WSTRING length validation applies to type/member defaults with the same `OutOfRange` E304 rule as normal VAR initializers;
  - initialization performance is part of the acceptance surface because runtime startup/default materialization changes. Record the init benchmark scope and regression budget before implementation: compare pre/post runs on the same machine and treat an unexplained initialization regression above roughly 10% as blocking unless documented with a recovery path;
  - the benchmark CLI shape is locked: add `BenchAction::Init { project, samples, warmup_cycles, output }` as `trust-runtime bench init`, implemented in a new `crates/trust-runtime/src/bin/trust-runtime/bench/init.rs` included from `bench.rs`. `warmup_cycles` defaults to 0 because warmup hides startup cost. Output includes init-only median/p95/p99, first-cycle latency, and steady-state cycle summary;
  - baseline command is reproducible: use `cargo run -p trust-runtime -- bench init --project crates/trust-runtime/tests/fixtures/init_bench --samples 1000 --output json` unless implementation documents a better tracked fixture path before the baseline is captured. The fixture must be checked in;
  - benchmark interpretation is best-effort but explicit: use repeated samples and median/p95/p99 to reduce timer noise; only sustained regressions over the budget on the same machine are blocking, unless a smaller regression clearly points to a real algorithmic issue;
  - coverage mapping updates target Tables 11, 12, 13, 14, and 41 when the REF_TO guard tests are added. Touch Table 62 only if implementation changes configuration/resource semantics beyond using existing `VAR_CONFIG` machinery.
- Add or update entries in:
  - `docs/IEC_DECISIONS.md`
  - `docs/IEC_DEVIATIONS.md`
  - `docs/specs/02-data-types.md`
  - `docs/specs/03-variables.md`
  - `docs/specs/09-semantic-rules.md`
  - `docs/specs/10-runtime-semantics.md`
  - `docs/specs/coverage/iec-table-test-map.toml`

Exit criteria:

- We know what is required, what is repo-specific, and what is intentionally unsupported.
- The runtime spec no longer says "Struct: each field initialized to type default" without also accounting for declared field defaults.
- `docs/IEC_DEVIATIONS.md` has a `UNION` extension entry.

### Phase 1 - Red Tests And Architecture Guardrails

Add tests before behavior changes:

Parser tests:

- named struct init emits `InitializerList`;
- single-field aggregate;
- nested aggregate;
- partial aggregate;
- array of struct aggregate;
- `VAR_GLOBAL` aggregate;
- FB instance init parse;
- `TypeName(field := value)` remains `CallExpr`;
- function call named arguments such as `f(a := 1)` remain `CallExpr` with `Arg` children, not `InitializerList`;
- initializer-position parsing is invoked only from explicit initializer declaration/default sites after `:=`: `parse_var_decl()` and every TYPE-level declaration default parsed by `parse_type_decl()` after `parse_type_def()`. It is not invoked from `parse_arg_list()`, `parse_primary_expr()`, enum value assignments, string length expressions, subrange expressions, statement parsing, or any general expression context;
- TYPE-level defaults use initializer-position parsing for all supported shapes:
  - alias/simple type default: `TYPE Limited : INT := 100; END_TYPE`;
  - struct aggregate default: `TYPE Cfg : MyStruct := (a := 1, b := 2); END_TYPE`;
  - scalar array default: `TYPE Samples : ARRAY[1..3] OF INT := [1, 2, 3]; END_TYPE`;
  - array-of-struct default: `TYPE Channels : ARRAY[1..8] OF Channel := [8((range := 1))]; END_TYPE`;
- positional form rejected with one diagnostic;
- empty aggregate `()` is rejected with a precise diagnostic, not an "unknown expression" fallback;
- empty struct definitions parse if currently accepted, but empty aggregate initialization `s : Empty := ();` still produces the empty-aggregate diagnostic rather than silently constructing a value;
- malformed aggregate recovery is bounded to one diagnostic per bad declaration;
- three malformed initializers in one `VAR` block produce three targeted diagnostics, not a cascade;
- a nested aggregate error such as `(inner := (a := ))` does not corrupt recovery for the outer declaration;
- array repetition with a struct element, for example `[3((a := 1))]`, parses as supported issue-51 syntax unless Phase 0 deliberately narrows the first implementation slice and adds a targeted diagnostic/spec note;
- trivia inside aggregates, for example `(a := 1 (* comment *), b := 2)`, does not change the CST shape relative to the no-comment form.

HIR tests:

- struct field default is represented in the initializer catalog;
- union variant default is represented in the initializer catalog;
- non-constant struct field default errors;
- non-constant union variant default errors, if the union extension accepts defaults;
- normal declaration initializer type mismatch errors in HIR;
- unknown aggregate field errors;
- duplicate aggregate field errors;
- field type mismatch errors;
- partial aggregate without error;
- `T(field := value)` for a struct type does not emit E103;
- `T(field := value)` for an FB type follows the FB instance initializer path where the target context is an FB instance declaration;
- `T(field := value)` for a class type reports `InvalidOperation` E202 with the Phase 0 message, because class aggregate initialization is out of issue #51 scope;
- a struct type name that collides with a function name has deterministic resolution rules and tests for both callable and initializer contexts;
- real function calls still work;
- unknown callable name still emits the existing diagnostic;
- `VAR_EXTERNAL G : INT := 1` remains a HIR diagnostic. This scalar case already exists; add aggregate/array variants after `InitializerList` and central classification exist so HIR cannot miss them.
- `STRUCT OVERLAP` aggregate initialization is rejected when overlap syntax is modeled. If `OVERLAP` remains unsupported in this issue, record a separate pre-existing parser/spec gap and add the test at the point the overlap model lands.
- directly derived type defaults such as `TYPE MyAnalog : AnalogChannel := (MinScale := 0); END_TYPE` are represented and type-checked.
- VAR_CONFIG override priority is represented through `InitializerOwner::VarConfigOverride` and routed through the initializer service.
- aggregate initializer against a non-aggregate target, for example `x : INT := (a := 1);`, is a HIR diagnostic;
- case-insensitive aggregate field matching works at HIR level, for example `(A := 1)` for field `a`;
- nested aggregate type mismatch is reported at the nested field range where possible;
- reference/pointer defaults have an explicit rule: `NULL` is valid where assignment-compatible, and any non-constant reference-producing expression is rejected in type/member defaults unless a standard-backed exception is implemented;
- implicit narrowing and target range violations are diagnostics, for example `SINT := 200` and subrange default `0..10 := 20`;
- enum value mismatch across enum types is a diagnostic;
- constant-expression scope is tested: named compile-time `CONSTANT` accepted, ordinary variable reference rejected, undeclared name reported as undefined, divide by zero reported, and integer overflow reported;
- the fallible default constant evaluator distinguishes not-constant, undefined-name, divide-by-zero, overflow, unsupported operator, cyclic reference, and target range violation so each maps to the intended diagnostic code;
- cyclic constants, for example `VAR_GLOBAL CONSTANT a : INT := b; b : INT := a; END_VAR`, and a default that depends on such a cycle produce `CyclicDependency` E305 instead of panic, stack overflow, or silent `None`;
- cycle coverage must exercise both the collector/precollection constant path and the type-check/default constant path. Passing only the collector path is insufficient because `crates/trust-hir/src/type_check/const_eval.rs` currently has no cycle guard;
- deeply nested but acyclic aggregate defaults beyond the chosen expansion limit produce a diagnostic instead of panic or resource exhaustion;
- CONST forward-reference behavior is locked by test: `VAR_GLOBAL CONSTANT a : INT := b; b : INT := 5; END_VAR` either resolves deterministically to 5 or is rejected with a documented diagnostic. Do not leave it dependent on collection order by accident;
- field default `INT := 1 / 0` produces `InvalidOperation` E202 rather than silent `None`;
- field default `INT := 99999 * 99999` or another overflowing expression produces a diagnostic through the fallible evaluator;
- operator precedence in default expressions is locked with a test such as `field : INT := -2 ** 4`, matching the parser's documented precedence rather than relying on implementer interpretation;
- field default `STRING[3] := 'hello'` and the WSTRING equivalent produce `OutOfRange` E304 using the same rule as normal variable initializers;
- `SIZEOF` in a constant field default, for example `field : DINT := SIZEOF(T_OtherType)`, is accepted when the operand is in the supported constant subset;
- TIME and DATE typed literals in field defaults, for example `T#100ms` and `D#2024-01-01`, are accepted as constant literals if the existing literal model supports them;
- `VAR_GLOBAL CONSTANT cfg : Config := (a := 1, b := 2);` works when every aggregate element is constant;
- unknown-field diagnostics range to the field-name token, and duplicate-field diagnostics range to the second occurrence's field-name token;
- aggregate field order is independent of declaration order, for example `(MIN_SCALE := -100, RANGE := BIPOLAR_10V)` produces the same field values as `(RANGE := BIPOLAR_10V, MIN_SCALE := -100)`;
- cross-project constants used by imported field defaults either resolve through the same project-type import/default-evaluation path or produce a diagnostic; they must not silently fall back to type defaults;
- alias chains preserve defaults, for example `TYPE Alias : T_Step := (...)`.

Runtime tests:

- struct field default only;
- struct aggregate named initializer;
- partial aggregate uses field defaults and then type defaults;
- nested struct defaults;
- array of structs with aggregate elements;
- statement assignment `s := T(field := value)`;
- `VAR_GLOBAL` aggregate initializer;
- directly derived struct type default applies at runtime;
- VAR_CONFIG instance-specific override wins over type-level and variable-level defaults;
- FB instance initializer at least for a simple FB input;
- union variant initializer support;
- priority chain coverage: elementary/type default, type-level member default, variable initializer, and `VAR_CONFIG` override each win in the documented order;
- nested struct default where the outer aggregate overrides an inner field;
- nested struct default where the outer aggregate omits an inner field and the inner type default applies;
- array of structs where each element receives type/member defaults plus its element-level aggregate overrides;
- adapted IEC examples from `/home/johannes/Downloads/iec-61131-3.ocr.txt:2702-2731` and `:6953-6956`;
- FB instance initializer coverage for `VAR_INPUT`, `VAR_OUTPUT`, and public members, plus diagnostics for private, temporary, external, or otherwise non-initializable FB members if those categories exist in the current model;
- FB instance initializer targeting a `VAR_IN_OUT` member produces `InvalidOperation` E202 with the same stable diagnostic class as private/temp/external rejection;
- standard FB initializer coverage for `TON` at minimum when stable;
- variable-level `REF(...)` initializer works for a legal non-temporary target, for example `myRef : REF_TO INT := REF(myInt)` stores a non-NULL reference and `myRef^` reads `myInt`;
- `REF(...)` against a temporary or otherwise illegal target is rejected if the current semantic model can express that IEC restriction;
- self-referential type through `REF_TO`, for example `Node.next : REF_TO Node`, constructs without recursive default expansion and defaults the reference to `NULL`;
- aggregate default for a `REF_TO` field is rejected by the constant-expression rule instead of trying to recursively construct through the reference;
- case-insensitive field matching at runtime produces the same value for `(A := 1)` and `(a := 1)`;
- project type import/export preserves initializer records, so library/default metadata is not lost across symbol import paths;
- retained-state restart still canonicalizes struct values through `StructValue::new` and is not weakened by the new default path;
- retained value priority is preserved: a `VAR_GLOBAL RETAIN` struct with a variable initializer or TYPE-level field default reconstitutes the retained value on warm restart instead of reapplying defaults over it;
- multi-name aggregate declarations materialize independent values for every name: mutating `a.x` after `VAR a, b, c : T := (x := 1); END_VAR` does not mutate `b.x` or `c.x`;
- multi-name aggregate declarations evaluate/type-check the source initializer once per declaration, then materialize independent storage for each declared name. The test must catch accidental per-name re-evaluation and accidental shared FB/class instance identity;
- multi-name struct aggregate mutation proves copy-on-write: after `VAR a, b : T := (field := 1); END_VAR`, writing `a.field := 2` leaves `b.field = 1` through every supported mutation path;
- multi-name FB/class declarations with aggregate initializers create distinct runtime instances per name, not cloned `Value::Instance` handles;
- direct address plus aggregate initialization is covered, for example `VAR_GLOBAL G AT %MW10 : T := (a := 1); END_VAR`;
- VM/helper parity for every initializer runtime behavior where both paths exist, or an explicit documented reason why a behavior is not available under one backend. Production interpreter parity is released for Issue #51 because `docs/specs/10-runtime-semantics.md` defines production execution as bytecode-VM only and rejects the old `interpreter` backend in runtime selection.

Benchmark/performance guardrails:

- add or extend an initialization-focused benchmark under `crates/trust-runtime/src/bin/trust-runtime/bench/` using the existing `trust-runtime bench` command structure: implementation files are included from `bench.rs`, and bench tests live under `bench/tests.rs`;
- expose that benchmark as `trust-runtime bench init`, backed by `BenchAction::Init` and a checked-in fixture under `crates/trust-runtime/tests/fixtures/init_bench`;
- benchmark representative startup/default workloads: a program with around 100 globals mixing scalar and aggregate initializers, struct field defaults, multi-name aggregate declarations, and arrays of structs;
- benchmark retain/restart materialization for retained structs that must canonicalize through `StructValue::new`;
- benchmark `StructValue::new` versus `from_canonical_parts` for a representative 50-field struct in a repeated loop, while preserving the rule that bypasses are allowed only when invariants are already proven;
- benchmark first-mutation cost after multi-name aggregate construction so copy-on-write costs are visible instead of hidden behind cheap shared-Arc construction;
- run helper and VM init workloads where both paths exist, and record parity plus timing. Do not require production interpreter parity because the shipped runtime is VM-only;
- record a pre-implementation baseline and a post-change run on the same machine. Initialization throughput/startup regressions above roughly 10% must either be fixed or documented in `docs/internal/testing/checklists/architecture-improvements.md` with cause, user impact, and recovery plan.

Architecture guardrails:

- expression-kind parity test for syntax/HIR/runtime/LSP/IDE that fails on current drift before Phase 2 lands;
- statement-kind parity test for HIR/runtime/IDE that fails on current drift before Phase 2 lands;
- source-count guardrail: exactly one canonical expression/statement classifier lives in `trust-syntax`; local helper names either disappear or delegate with a documented reason;
- after Phase 2, `rg -n "fn is_expression_kind|fn is_statement_kind" crates/trust-hir/src crates/trust-runtime/src crates/trust-ide/src crates/trust-lsp/src --glob '*.rs'` returns no local implementations except deliberate delegating wrappers with documented tests;
- central classifier trivia guard: no trivia kind, including `Pragma`, is classified as expression, statement, initializer expression, or aggregate initializer;
- `ExpressionIndex::from_root` produces IDs for `InitializerList` and `ArrayInitializer` once those nodes are emitted;
- Salsa/tracked-output guardrail: editing a struct field default recomputes the owning file's initializer catalog entry and does not remap sibling fields to the edited initializer;
- cross-project import guardrail: imported struct/union defaults get target-local `InitializerId` values or an explicit ID translation map, never raw copied source IDs;
- declaration-part extraction test that proves initializer children are not silently dropped;
- structural compile-time enforcement: declaration helper return types must change from tuples to named structs so old `_initializer` tuple destructures no longer compile;
- optional belt-and-braces grep/check for `_initializer` discards in declaration/type lowering;
- dependency-boundary guardrail: the new runtime initializer materialization module must not parse raw CST or import `trust_syntax::SyntaxNode` for semantic initializer decisions; any syntax-to-contract lowering remains in a separate lowering owner;
- dependency-boundary guardrail: HIR type definitions and initializer catalog records must not import `trust_runtime::Value`;
- diagnostic-code matrix tests assert the declared `DiagnosticCode` for each initializer rule;
- `DiagnosticCode::UndefinedField` and `DiagnosticCode::DuplicateField` exist before aggregate diagnostic tests are written;
- VAR_CONFIG initializer tests prove the existing `ConfigInit` path routes through the new initializer service instead of the legacy direct `coerce_initializer_value_to_type` shortcut;
- regression reproducer for GitHub issue #51.

Exit criteria:

- Tests fail for the known reasons before implementation.
- The failure mode is not a vague parser cascade where a direct semantic diagnostic is expected.

### Phase 2 - Central Syntax Classification

Implement the central `SyntaxKind` classification helper in `trust-syntax`.

Update consumers:

- HIR diagnostics and collectors,
- runtime harness utility,
- LSP syntax helpers,
- IDE var-decl/refactor helpers.

Exit criteria:

- `InitializerList` and `ArrayInitializer` are classified consistently.
- Statement nodes are classified consistently through the same central API, not through four copied local helpers.
- Trivia kinds, including `Pragma`, are excluded by every central classifier.
- `ExpressionIndex::from_root` includes `InitializerList` and `ArrayInitializer` after the parser can emit them.
- A source-count guardrail confirms the central helper is the only implementation owner; other crates delegate or document a tested divergence.
- Any consumer-specific divergence is documented and covered.

### Phase 3 - Declaration Parts And HIR Initializer Catalog

Implement `VarDeclParts` and remove tuple-only declaration helpers.

Add initializer handles:

- `InitializerId`,
- `InitializerRecord`,
- `InitializerOwner`,
- `default_initializer: Option<InitializerId>` or equivalent on relevant type members.

Update:

- struct field collection,
- union variant collection,
- normal variable collection,
- global/external validation,
- symbol import/export,
- `crates/trust-hir/src/db/symbol_import.rs`,
- project type provider copy paths in `crates/trust-hir/src/db/queries/collector/types.rs` and `crates/trust-hir/src/db/queries/salsa_backend.rs`,
- runtime harness `parse_var_decl` callers, especially `crates/trust-runtime/src/harness/compiler/types.rs:201-220` and the union branch at `:135-151`.
- VAR_CONFIG initializer representation as `InitializerOwner::VarConfigOverride` and routing through the runtime initializer service.
- Own the initializer catalog on the collected `SymbolTable` in the first design, so it travels through the existing Salsa-tracked symbol query result. Use a sibling tracked output only if the implementation documents why a `SymbolTable` field is too invasive. Do not use an untracked side table.
- Keep the `SymbolTable` surface small: the catalog field is private or `pub(crate)`, and consumers use narrow accessors for ID lookup, owner/range lookup, and iteration needed by diagnostics/import. Avoid exposing a mutable catalog handle that turns `SymbolTable` into a general initializer service.
- Treat `InitializerId` as file/project-local. Cross-project symbol import must copy initializer records into the target catalog or translate IDs through an import map.

Exit criteria:

- Struct field defaults and repo-supported union defaults can no longer be silently dropped.
- `VAR_EXTERNAL` initializer detection remains intact.
- Old tuple destructures of declaration info no longer compile.
- HIR stores enough information to type-check and later materialize defaults without raw `SyntaxNode` in `StructField`.
- If `ExpressionIndex` is reused, aggregate initializer records are keyed by those expression IDs. If it is not reused, the implementation documents why and adds equivalent ID/range lookup tests.
- Editing a field default in a Salsa-backed project invalidates/recomputes the catalog result that owns that field and the next query/runtime materialization sees the edited value.
- Imported struct/union defaults materialize through target-local initializer records, preventing cross-project ID collisions.

### Phase 4 - HIR Initializer Type Checking

Add expected-type initializer checking.

Tasks:

- Type-check every declaration initializer against target type.
- Type-check field defaults as constant initializers.
- Add aggregate field-name/type validation.
- Add constructor recognition for `T(...)` when `T` resolves to a struct/union/FB target.
- Reject `T(...)` for class types with the Phase 0 `InvalidOperation` E202 decision unless class aggregate initialization is deliberately added in a later issue.
- Preserve function/method/FB call behavior for actual call targets.
- If the declared target type is unresolved or `Type::Unknown`, suppress initializer-specific diagnostics and let the existing undefined-type diagnostic own the error. Do not cascade into misleading aggregate/type-mismatch diagnostics for a target type that is not known.
- Implement the diagnostic-code matrix from the target architecture section and add tests that assert codes, not only message text.
- Enforce the constant-expression subset for type/member defaults, including explicit diagnostics for divide by zero, overflow, non-constant references, and target range violations.
- Implement the fallible default constant-evaluation path chosen in Phase 0. Do not use the existing `Option<i64>` HIR const evaluators directly for required defaults.
- Include cycle detection in the fallible default constant evaluator and map cyclic constant/default references to `CyclicDependency` E305.
- The fallible default constant evaluator carries its own cycle guard. If it reuses or wraps `crates/trust-hir/src/type_check/const_eval.rs:7-74`, that evaluator must gain a guard equivalent to the collector evaluator's `FxHashSet` pattern at `crates/trust-hir/src/db/queries/collector/const_eval.rs:23-27`; otherwise a collector-path cycle test can pass while the type-check/default path still stack-overflows.
- Add a bounded recursion-depth diagnostic for aggregate/default expansion so deeply nested but acyclic defaults cannot recurse until panic or resource exhaustion.
- Invoke string-length validation for STRING/WSTRING defaults on STRUCT fields and UNION variants, using the same rule as `check_string_initializer(...)` for normal variable declarations.
- Range unknown-field diagnostics to the field-name token, and duplicate-field diagnostics to the second occurrence's field-name token.

Exit criteria:

- Scalar declaration mismatches become HIR diagnostics.
- `T_Step(...)` as a struct initializer no longer emits E103.
- Unknown target types emit the existing undefined-type diagnostic without an initializer diagnostic cascade.
- Unknown/duplicate/wrong fields are HIR diagnostics.
- Field name matching follows the repo's case-insensitive identifier rule.
- Non-constant or out-of-range type/member defaults never silently fall back to the target type default.
- STRING/WSTRING type/member defaults that exceed their declared length are HIR `OutOfRange` diagnostics, not runtime truncation.
- Divide-by-zero and overflow in default constant expressions produce specific diagnostics instead of indistinguishable `None`.
- Cyclic constant/default references are diagnostics, not compiler crashes or `None`.
- Cyclic-dependency tests exercise both the collector/precollection path and the type-check/default-check path so one guarded evaluator cannot mask an unguarded second evaluator.
- Deep aggregate/default expansion past the chosen recursion limit is a diagnostic, not a panic or silent fallback.
- Diagnostic-code tests pass for every rule in the matrix.
- No runtime startup path is responsible for first detection of source-level initializer type errors.

### Phase 5 - Parser Aggregate Initializers And Recovery

Implement initializer-position parsing:

- `parse_var_initializer()` or equivalent in declarations.
- Gate: call `parse_var_initializer()` only from explicit initializer-position declaration/default sites after `:=`, including `parse_var_decl()` and every TYPE-level default parsed by `parse_type_decl()` after `parse_type_def()`. Do not call it from `parse_arg_list()`, `parse_primary_expr()`, enum value assignments, string length expressions, subrange expressions, statement parsing, or any general expression context.
- `LParen Name Assign` emits `InitializerList`.
- array element parsing recurses into initializer-aware parsing where an expected aggregate element may appear.
- malformed aggregate recovery is bounded.

Exit criteria:

- Named struct/FB aggregate syntax parses cleanly.
- Positional unsupported syntax emits one targeted diagnostic.
- Existing parenthesized expressions remain parenthesized expressions outside initializer context.
- `f(a := 1)` continues to parse as a function-call argument list, and `s : T := T(a := 1);` remains `CallExpr` in CST for HIR to reclassify by target type.
- TYPE-level defaults for alias/simple types, struct aggregate defaults, scalar array defaults, and array-of-struct defaults use initializer-position parsing, while enum explicit values remain ordinary constant expressions.
- Array repetition over aggregate elements, for example `[3((a := 1))]`, is either supported and tested or deliberately rejected with the Phase 0 diagnostic/spec decision. It must not parse into an unrelated call/paren shape that downstream silently ignores.

### Phase 6 - Runtime Materialization Funnel

Implement the runtime initializer service.

Tasks:

- Construct defaults with the initializer catalog.
- Apply aggregate overrides.
- Support nested aggregates.
- Support arrays of aggregates.
- Route normal var init, global init, FB/class local init, and VM local/static init through the same service.
- Route the existing `VAR_CONFIG` application path through the same `evaluate_initializer(...)` service. Do not keep the current direct `eval_storage_expr(...)` plus `coerce_initializer_value_to_type(...)` shortcut as a separate semantic path.
- Replace runtime FB/class initializer rejection sites with initializer-service calls where the initializer is legal: `crates/trust-runtime/src/instance.rs:285-295`, `crates/trust-runtime/src/instance.rs:335-358`, and `crates/trust-runtime/src/runtime/vm/local_init.rs:212-239` currently return `RuntimeError::TypeMismatch` for any FB/class initializer.
- Keep `StructValue::new` or equivalent validation as the final boundary.
- Remove the runtime-side type-member initializer discard in `crates/trust-runtime/src/harness/compiler/types.rs:201-220`; the `UnionDef` path at `:135-151` must preserve the same default metadata.
- Reconcile `StructValue::new` versus `from_canonical_parts`: validation is available through `StructValue::new`, while canonical construction is only acceptable when the service has already assembled declaration-order, type-checked fields.
- Use precomputed values where all initializer expressions are compile-time constants; use default-plus-overrides evaluation for runtime-dependent aggregate expressions.
- For multi-name declarations, lower/type-check/evaluate the source initializer once per declaration, then materialize independent storage for each declared name. Cloning is acceptable for immutable scalar/aggregate/`REF(...)` values when it preserves source semantics; runtime instance identities for FB/class values must be created independently, not cloned into shared storage.
- Preserve and test the existing copy-on-write contract for shared struct backing: any path that mutates `Value::Struct(Arc<StructValue>)` must go through `Arc::make_mut` or an equivalent deep-copy boundary. The initializer service may share immutable aggregate backing only when first mutation remains isolated across variables.
- Keep the initializer service as orchestration only. It may call syntax lowering, HIR metadata, constant evaluation, default construction, coercion, and storage writers, but it must not absorb those responsibilities into one god module.
- Keep the initializer service split if it grows: no single initializer-service function should exceed roughly 60 lines, and no single initializer-service module should exceed roughly 400 lines before it is split by responsibility such as lowering, validation, default construction, constant evaluation, and storage application.
- Treat array and enum `from_canonical_parts(...)` bypasses as a follow-up audit, not part of issue #51, unless implementation touches them. The same validating-constructor rule should eventually be reconciled for arrays and enums, but struct/union correctness is the issue-51 scope.
- Add the initialization benchmark workload as `trust-runtime bench init` under the existing command family. Implement it as `BenchAction::Init` plus `bench/init.rs`, included from `bench.rs`; do not make it `#[cfg(test)]`-only.

Exit criteria:

- Field defaults work for locals, globals, nested structs, and arrays.
- Multi-name declarations apply the initializer independently to every name and do not re-evaluate the source initializer per name.
- Multi-name struct aggregate mutation proves copy-on-write isolation, and multi-name FB/class initialization proves distinct instance IDs.
- Missing aggregate fields get the correct default chain.
- VAR_CONFIG overrides participate in the same initializer materialization path and priority-chain tests.
- Retained values are applied after default construction on warm restart/load and are never overwritten by newly materialized field defaults.
- Runtime coercion is no longer the primary detector for source type errors.
- Helper and VM behavior are equivalent for covered cases; no production interpreter parity is required because the shipped runtime is VM-only.
- The new runtime initializer materialization owner does not parse raw CST for semantic decisions.
- Initialization benchmark results are recorded for pre/post runs on the same machine. Regressions above roughly 10% are fixed or documented with rationale and a follow-up recovery path.
- Benchmark output comes from the `bench init` CLI path, includes construction and first-mutation measurements, and is reproducible with the locked fixture/sample command from Phase 0.

### Phase 7 - FB Instance Initialization

Implement or complete FB instance initialization using the same aggregate initializer contract.

Tasks:

- Validate legal initializer targets for FB instances from IEC/repo specs.
- Lift the concrete runtime FB/class initializer rejection. `crates/trust-runtime/src/instance.rs:285-295` and `crates/trust-runtime/src/instance.rs:335-358` currently return `RuntimeError::TypeMismatch` whenever an FB/class variable has an initializer; Phase 7 replaces those branches with initializer-service materialization of legal aggregate member overrides over the created instance's default state.
- Audit the symmetric VM path at `crates/trust-runtime/src/runtime/vm/local_init.rs:212-239` so VM local/static FB/class initializers do not keep the old rejection after the normal runtime path is fixed.
- Reject `VAR_IN_OUT`, private, temporary, external, and otherwise non-initializable FB members with `InvalidOperation` E202 and stable message wording.
- Apply omitted FB member defaults.
- Preserve existing FB call semantics.
- Add runtime tests using a small custom FB and standard `TON` if stable.

Exit criteria:

- `Timer : TON := (PT := T#1s);` behaves according to repo spec or is explicitly documented if a narrower support profile is chosen.
- The previous runtime failure mode for FB/class initializers (`RuntimeError::TypeMismatch` from `instance.rs`/VM local-init rejection branches) has a regression test that now passes for legal FB initializer syntax and still rejects illegal member targets.
- No separate FB-only initializer model exists.

### Phase 8 - Docs, Diagrams, And Release Hygiene

Docs/specs:

- Update struct default wording in `docs/specs/10-runtime-semantics.md`.
- Update variable and data-type specs with exact supported forms.
- Update semantic rules for initializer type checking.
- Update IEC decisions/deviations.
- Update `docs/specs/coverage/iec-table-test-map.toml` for Tables 11, 12, 13, 14, and 41 coverage touched by initializer/default behavior. Add or change Table 62 coverage only if this work changes configuration/resource semantics beyond exercising the existing `VAR_CONFIG` path.
- Add the `UNION` extension/default decision to `docs/IEC_DEVIATIONS.md`.
- Record the initialization benchmark baseline and post-change result summary in `docs/internal/testing/checklists/architecture-improvements.md` or the PR/release evidence artifact used for the implementation. Include the exact command, sample count, fixture path, machine identifier, and whether the run was pinned/isolated or best-effort.

Diagrams:

- `docs/diagrams/syntax/syntax-pipeline.puml`: add initializer-position parsing and bounded aggregate recovery.
- `docs/diagrams/hir/hir-semantics.puml`: add initializer catalog, declaration initializer checking, and type-member default handles.
- `docs/diagrams/architecture/system-architecture.puml`: update HIR/runtime contract if the initializer catalog crosses the execution path.
- `docs/diagrams/architecture/runtime-execution.puml`: add initializer materialization funnel and default construction priority.
- `docs/diagrams/architecture/runtime-bytecode-vm-execution.puml`: show VM/local init consuming the initializer service or shared lowered model.

Required commands:

- `scripts/render_diagrams.sh`
- `python scripts/check_diagram_drift.py`
- update `docs/internal/testing/checklists/architecture-improvements.md`

Release hygiene when implementation lands:

- Update `CHANGELOG.md`.
- Bump workspace version unless explicitly told not to.
- Sync VS Code package versions if workspace version changes.
- Run focused tests during implementation.
- Run the issue-51 initialization benchmark before and after the runtime materialization changes on the same machine, using the Phase 0 baseline command unless intentionally updated before baseline capture, and include the summary in the PR description and issue closeout.
- Before declaring completion: `just fmt`, `just clippy`, `just test-all`.
- Runtime-impacting implementation also runs:
  - `cargo test -p trust-runtime --test api_smoke`
  - `cargo test -p trust-runtime --test debug_control`
  - `cargo test -p trust-runtime --test complete_program`
  - `cargo test -p trust-runtime --test runtime_reliability`
- If diagnostics surface through LSP/VS Code, add extension/LSP coverage and run the relevant extension tests.
- If version is bumped and merged, complete tag/release verification and close GitHub issue #51 with a release-linked comment.

## Acceptance Definition

The architecture fix is not complete until these are true:

- There is one named initializer/default contract across parser, HIR, runtime, IDE/LSP, and VM.
- Accepted initializer syntax cannot be silently dropped in HIR collection or runtime type lowering.
- Unsupported initializer syntax or unsupported initializer semantics produce diagnostics. Silent fallback to zero/type default is a failure.
- Struct field defaults are preserved and materialized.
- Union variant defaults are either preserved and materialized as the locked repo extension, or the decision is changed before implementation and the syntax is rejected with diagnostics. Current silent defaults are not acceptable.
- FB instance initialization is either implemented according to IEC/repo specs or documented as a deliberate unsupported subset with diagnostics.
- FB instance aggregate initialization rejects `VAR_IN_OUT` targets.
- Legal FB/class initializer support does not leave the old runtime `TypeMismatch` rejection branches active in `instance.rs` or VM local initialization.
- Class `T(...)` aggregate initialization remains an explicit unsupported diagnostic for issue #51 scope, not a fallback to callable lookup or struct semantics.
- STRING/WSTRING field defaults are length-checked at HIR level.
- The initializer catalog is Salsa-tracked and cross-project initializer handles are target-local or explicitly translated; raw source `InitializerId` collisions are a failure.
- Retained values win over field/type/variable defaults on warm restart and retain-store load.
- Multi-name aggregate declarations initialize every declared name independently without per-name re-evaluation or shared runtime instance identity.
- Multi-name aggregate cloning preserves struct copy-on-write isolation across first mutation.
- Initialization performance is not silently regressed: benchmark evidence is recorded, and any >10% regression is either fixed or documented with a recovery path.
- Initialization benchmark evidence is produced through the reproducible `trust-runtime bench init` CLI path, not a test-only helper.
- Runtime and IDE diagnostics agree on declaration initializer errors.
- Parser recovery for bad aggregate initializers is local and non-cascading.
- Central expression and statement classifiers have executable drift guards.
- HIR/runtime dependency-boundary tests prevent raw CST and runtime `Value` types from leaking into the wrong owners.
- Specs and PlantUML diagrams match the implemented architecture.
- Regression tests cover the GitHub issue repros plus the silent field-default and union-default cases.

## Risks And Mitigations

Risk: Raw syntax handles in type metadata create equality/import/lifetime problems.

Mitigation: use lightweight initializer IDs on type members and store records in an initializer catalog.

Risk: A runtime initializer service becomes a new god object.

Mitigation: keep it as orchestration only. Syntax classification, type checking, const evaluation, and storage writes remain separate owners. Split the service when functions or modules exceed the stated small-size bound.

Risk: HIR constant evaluation grows too large.

Mitigation: implement only the constant-expression subset needed for declared defaults first, with tests. Do not rewrite all expression evaluation.

Risk: VM and helper/interpreter paths diverge.

Mitigation: add differential tests for every runtime initializer form and route VM local/static initialization through the same materialization contract or shared lowered model.

Risk: Correct initializer materialization regresses runtime startup or retain-load performance.

Mitigation: add an init-focused benchmark under the existing `trust-runtime bench` command family, record pre/post results, and treat unexplained regressions above the Phase 0 budget as blocking unless documented with a recovery path.

Risk: Multi-name aggregate initialization appears independent but shares mutable backing.

Mitigation: preserve the existing `Value::Struct(Arc<StructValue>)` copy-on-write mutation contract, test first-write isolation across multi-name declarations, and measure first-mutation cost in the init benchmark.

Risk: Parser context sensitivity spreads.

Mitigation: add one initializer-position parser entry point and leave normal expression parsing unchanged.

Risk: Union semantics are not IEC-standard.

Mitigation: make a repo decision. Support consistently or reject explicitly; do not silently default.

## What Not To Bundle

Do not bundle these with issue #51:

- OOP `EXTENDS`/`IMPLEMENTS`/`Action` audits. Existing code does reference those constructs; they are not proven silent-drop bugs in this investigation.
- A full parser rewrite.
- A new VM opcode unless tests prove the existing fallback/shared lowering cannot support the feature.
- Broad refactors of unrelated HIR collectors.
- OSCAT example/test work from the dirty main checkout.

## Claude Review Questions

Before implementation, ask Claude to review the companion prompt:

- `docs/internal/references/issue-51-struct-initializers/claude-architecture-review-prompt.md`

The review must verify this plan against live code, repo specs, diagrams, and the IEC OCR text. It must not treat any previous assistant claim as authoritative.

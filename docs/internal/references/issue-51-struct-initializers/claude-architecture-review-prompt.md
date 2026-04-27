# Claude Architecture Review Prompt - Issue #51

You are reviewing Codex's root-cause architecture plan for trust-platform issue #51. Do not implement code. Your job is to verify or reject the plan before implementation starts.

Read first:

- `docs/internal/references/issue-51-struct-initializers/root-cause-architecture-plan.md`
- `docs/internal/references/issue-51-struct-initializers/report.md`

Use this IEC OCR source:

- `/home/johannes/Downloads/iec-61131-3.ocr.txt`

Important instruction:

- Do not assume Codex or any prior assistant is correct.
- Verify every concrete claim against the live checkout, repo specs, PlantUML diagrams, and the IEC OCR text.
- Cite exact `file:line` references.
- Do not edit files.
- Do not run broad test suites. Small `rg`, `sed`, or targeted repro commands are fine if they support review.

Required files to inspect:

- `/home/johannes/Downloads/iec-61131-3.ocr.txt`
- `docs/specs/README.md`
- `docs/specs/02-data-types.md`
- `docs/specs/03-variables.md`
- `docs/specs/09-semantic-rules.md`
- `docs/specs/10-runtime-semantics.md`
- `docs/specs/coverage/iec-table-test-map.toml`
- `docs/IEC_DECISIONS.md`
- `docs/IEC_DEVIATIONS.md`
- `docs/diagrams/syntax/syntax-pipeline.puml`
- `docs/diagrams/hir/hir-semantics.puml`
- `docs/diagrams/architecture/system-architecture.puml`
- `docs/diagrams/architecture/runtime-execution.puml`
- `docs/diagrams/architecture/runtime-bytecode-vm-execution.puml`
- `docs/internal/testing/checklists/architecture-improvements.md`
- `crates/trust-syntax/src/parser/grammar/declarations.rs`
- `crates/trust-syntax/src/parser/grammar/expressions.rs`
- `crates/trust-syntax/src/parser/source.rs`
- `crates/trust-syntax/src/syntax/mod.rs`
- `crates/trust-hir/src/db/queries/collector/variables.rs`
- `crates/trust-hir/src/db/queries/collector/types.rs`
- `crates/trust-hir/src/db/queries/collector/validation.rs`
- `crates/trust-hir/src/types/defs.rs`
- `crates/trust-hir/src/diagnostics.rs`
- `crates/trust-hir/src/db/diagnostics/expression.rs`
- `crates/trust-hir/src/db/diagnostics/unreachable.rs`
- `crates/trust-hir/src/db/diagnostics/type_check.rs`
- `crates/trust-hir/src/type_check/mod.rs`
- `crates/trust-hir/src/type_check/const_eval.rs`
- `crates/trust-hir/src/db/queries/collector/const_eval.rs`
- `crates/trust-hir/src/type_check/calls.rs`
- `crates/trust-hir/src/type_check/calls/resolve.rs`
- `crates/trust-hir/src/symbols/table.rs`
- `crates/trust-hir/src/db/symbol_import.rs`
- `crates/trust-hir/src/db/queries/salsa_backend.rs`
- `crates/trust-ide/src/var_decl.rs`
- `crates/trust-ide/src/refactor/operations/inline_and_namespace_helpers.rs`
- `crates/trust-ide/src/refactor/operations/convert_callsite_updates.rs`
- `crates/trust-lsp/src/handlers/features/core_impl/helpers/syntax_utils.rs`
- `crates/trust-runtime/src/harness/compiler/vars.rs`
- `crates/trust-runtime/src/harness/compiler/types.rs`
- `crates/trust-runtime/src/harness/compiler/config/globals_access.rs`
- `crates/trust-runtime/src/harness/compiler/config/entry.rs`
- `crates/trust-runtime/src/harness/config/config_inits.rs`
- `crates/trust-runtime/src/harness/config/bindings.rs`
- `crates/trust-runtime/src/harness/lower/expr/lowering.rs`
- `crates/trust-runtime/src/harness/util.rs`
- `crates/trust-runtime/src/program_model/expr.rs`
- `crates/trust-runtime/src/value/defaults.rs`
- `crates/trust-runtime/src/value/types.rs`
- `crates/trust-runtime/src/helper_eval/const_expr.rs`
- `crates/trust-runtime/src/helper_eval/storage_lvalue.rs`
- `crates/trust-runtime/src/eval/expr/call/reference.rs`
- `crates/trust-runtime/src/eval/expr/access.rs`
- `crates/trust-runtime/src/instance.rs`
- `crates/trust-runtime/src/runtime/vm/local_init.rs`
- `crates/trust-runtime/src/value/reference.rs`
- `crates/trust-runtime/src/runtime/restart.rs`
- `crates/trust-runtime/src/runtime/retain_store.rs`
- `crates/trust-runtime/tests/runtime_restart.rs`
- `crates/trust-runtime/src/bin/trust-runtime/bench.rs`
- `crates/trust-runtime/src/bin/trust-runtime/cli/bench.rs`
- `crates/trust-runtime/src/bin/trust-runtime/bench/command.rs`
- `crates/trust-runtime/src/bin/trust-runtime/bench/models.rs`
- `crates/trust-runtime/src/bin/trust-runtime/bench/project.rs`
- `crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs`

Questions to answer:

1. Is Codex correct that this is an initializer/default architecture issue rather than a simple parser bug?
2. Is the root cause stated correctly: no single contract spans parser, HIR, runtime lowering, runtime default construction, IDE/LSP, and VM?
3. Did Codex correctly use the IEC OCR evidence for:
   - structured data type initialization,
   - variable initialization priority,
   - FB instance initialization,
   - `VAR_EXTERNAL` initializer prohibition?
4. Are named struct aggregate initializers required by the repo specs and the IEC OCR evidence?
5. Is the plan correct to reject positional `(2, TRUE)` with one targeted diagnostic because the reviewed IEC OCR shows named `Struct_Elem_Init` and no positional production?
6. Is the plan correct that FB instance initialization must be designed into the same initializer architecture, even if implemented after struct defaults?
7. Is the proposed central syntax classification helper in `trust-syntax` the right root-cause fix for expression-kind and statement-kind drift, or should each layer keep a local wrapper with stricter tests? Search all local `fn is_expression_kind` and `fn is_statement_kind` definitions, not just one per crate.
8. Is `VarDeclParts` or an equivalent explicit declaration-parts object the right replacement for tuple extraction and `_ => {}` silent drops?
9. Is an HIR initializer catalog with lightweight `InitializerId` handles a sound design?
10. Should HIR store a runtime-independent constant/default value, a source locator plus typed initializer kind, or something else? Explain tradeoffs.
11. Does the plan avoid bad dependencies, especially raw `SyntaxNode` in `StructField` and `trust-runtime::Value` in HIR?
12. Does HIR currently have a suitable expression index or typed-expression model we should reuse instead of adding a new initializer catalog? Verify whether `ExpressionIndex::from_root` will produce IDs for `InitializerList` and `ArrayInitializer` after centralization.
13. Is the runtime initializer materialization funnel correctly scoped, or does it risk becoming a god object?
14. Which existing runtime init call sites must be routed through the funnel to avoid future drift?
15. Is the updated plan correct to lock `UNION ... END_UNION` as a repo/vendor extension and support union variant defaults through the same initializer catalog?
16. Is the updated plan correct that scalar `VAR_EXTERNAL := 1` already has HIR coverage, but aggregate/array external initializers still need coverage after central expression classification?
17. Is the updated plan correct to treat `STRUCT OVERLAP` initialization as invalid while also recording that `OVERLAP` parsing/modeling is a pre-existing gap?
18. Are directly derived struct type defaults and VAR_CONFIG override priority now covered in the right phases?
19. Is the plan correct that `StructValue::new(registry, type_id, fields)` exists, but default construction currently uses `from_canonical_parts` and must reconcile the validation boundary?
20. Are the proposed tests complete enough to prevent this class of silent bug from recurring, including empty aggregate, recovery cascade, non-aggregate target, narrowing/range errors, const-expression boundary errors, case-insensitive field matching, import/export, retain, and VM/interpreter parity?
21. Which tests should be written first, and which should be delayed until the data model is in place?
22. Which PlantUML diagrams must change if the implementation follows this plan?
23. Which repo specs and IEC decision/deviation docs must change?
24. Is the diagnostic-code matrix correct now that it explicitly requires new `UndefinedField` E107 and `DuplicateField` E108 codes before aggregate diagnostics?
25. Is the constant-expression scope for type/member defaults correct and small enough for KISS?
26. Does the plan correctly require a fallible `Result<_, ConstEvalError>` evaluator for default constant evaluation instead of using the existing `Option<i64>` evaluators that collapse overflow/divide-by-zero into `None`?
27. Is the parser scope gate correct: initializer-position parsing may only be called from explicit declaration/default initializer sites after `:=` (`parse_var_decl()` and every TYPE-level default parsed by `parse_type_decl()` after `parse_type_def()`), while `f(a := 1)` remains a `CallExpr` argument list and enum explicit values/string lengths/subranges remain ordinary expressions?
28. Is the plan correct to route the existing `VAR_CONFIG` path through the new initializer service rather than keeping it as a separate coercion shortcut?
29. Is the central classifier trivia invariant correct, especially for `Pragma`?
30. Are the REF_TO tests correctly scoped: positive VAR-level `REF(...)`, rejection for illegal temporary/reference-producing defaults where applicable, and cyclic `REF_TO` struct default termination?
31. Are the dependency-boundary guardrails precise enough to prevent spaghetti code without forbidding the existing runtime harness from using CST where it legitimately still lowers source syntax?
32. Did the plan correctly reject the NaN boundary-test claim as mandatory unless the language has a concrete NaN/Inf literal or constant surface?
33. What is the smallest implementation slice that fixes the architecture root cause without overengineering?
34. Are the central classifier set memberships fully locked: aggregate initializer nodes are exactly `InitializerList` and `ArrayInitializer`, generic expression nodes exclude those two, and initializer-expression nodes are the union?
35. Is the class-type `T(...)` decision locked to `InvalidOperation` E202 for issue #51 scope, rather than falling through to E103 or being treated as struct aggregate initialization?
36. Does the plan correctly require STRING/WSTRING length validation for STRUCT/UNION field defaults with the same `OutOfRange` E304 rule as normal VAR initializers?
37. Is the initializer catalog Salsa-compatible with the locked first design: stored on `SymbolTable` in the existing tracked symbol query result unless implementation documents why a sibling tracked output is better, with tests proving edits recompute the catalog and do not remap sibling defaults incorrectly?
38. Is `InitializerId` correctly treated as file/project-local, with cross-project symbol import copying records or translating IDs instead of raw-copying source IDs?
39. Does the retain/restart test plan prove retained struct values win over variable/type/member defaults after warm restart or retain-store load?
40. Does the plan cover every TYPE-level default form parsed by `parse_type_decl()` after `parse_type_def()`, including alias/simple type defaults, struct aggregate defaults, scalar array defaults, and array-of-struct defaults?
41. Does the fallible default constant evaluator require cycle detection and map cyclic constant/default references to `CyclicDependency` E305?
42. Does FB aggregate initialization explicitly reject `VAR_IN_OUT` members, not only private/temp/external members?
43. Is multi-name aggregate declaration behavior locked so `VAR a, b, c : T := (...); END_VAR` independently initializes every declared name?
44. Is the initialization benchmark requirement scoped correctly for runtime-impacting initializer changes, and does it use the existing `trust-runtime bench` command structure rather than inventing an unrelated harness?
45. Is the multi-name materialization strategy precise enough: evaluate/type-check the source initializer once per declaration, then materialize independent storage per name, without cloning FB/class runtime instance handles into shared identity?
46. Does the plan close the cycle-guard gap in `crates/trust-hir/src/type_check/const_eval.rs` if the fallible default evaluator reuses or wraps that path?
47. Is the Phase 0 array-repetition decision correct for aggregate elements such as `[3((a := 1))]`, or should the first implementation slice deliberately reject that form with a diagnostic and spec note?
48. Is the initializer catalog ownership decision correct to start as a `SymbolTable` field in the existing Salsa-tracked output, with a sibling tracked output allowed only if implementation documents why the field is worse?
49. Does the plan include enough benchmark recording/release evidence to prevent silent performance regressions in startup/default/retain-load initialization paths?
50. Is the benchmark CLI shape locked enough: `trust-runtime bench init` backed by `BenchAction::Init`, `bench/init.rs`, a checked-in `crates/trust-runtime/tests/fixtures/init_bench` project, and reproducible baseline command?
51. Does Phase 6/7 name every current FB/class initializer rejection site that must be lifted or routed, including `instance.rs:285-295`, `instance.rs:335-358`, and `runtime/vm/local_init.rs:212-239`?
52. Does the multi-name aggregate plan preserve the existing `Value::Struct(Arc<StructValue>)` copy-on-write contract across all mutation paths, and does the benchmark include first-mutation cost?
53. Does `check_decl_initializer` correctly suppress initializer-specific diagnostics when the target type is unresolved/`Type::Unknown`, leaving the existing undefined-type diagnostic as the owner?
54. Is the benchmark regression budget reproducible enough: exact command, fixture path, sample count, machine context, median/p95/p99 output, and timing-noise policy?
55. Is the `SymbolTable` catalog field/API shape narrow enough to avoid turning `SymbolTable` into a mutable initializer-service god object?

Output format:

- Start with `Verdict: ready to implement`, `Verdict: revise plan`, or `Verdict: not ready`.
- List blocking corrections first.
- List non-blocking improvements second.
- Then answer all 55 questions.
- For every correction, cite exact `file:line` evidence.
- Call out any claim in Codex's plan that is unsupported, stale, or too broad.
- End with a short recommended implementation order.

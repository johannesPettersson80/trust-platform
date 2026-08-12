# Phase 18 bytecode and authority-selection evidence

Date: 2026-07-30

Scope: focused `VERIF-P18-003`, `VERIF-P18-005`, `VERIF-P18-006`,
`VERIF-P18-004`, and reachability-only `VERIF-P18-007` progress. This evidence
does not close those rows repository-wide and does not claim mutation,
adequacy, environment, or broad-candidate proof.

## Specification and executable proof

The complete `crates/trust-runtime/src/bytecode/` denominator contains 237 live
facts. Six focused product-contract sections were added to
`docs/specs/12-bytecode.md`:

- `4-6-1-exact-boundaries-and-counter-arithmetic`
- `7-4-2-numeric-and-partial-access-domains`
- `7-5-1-1-recursive-lowering-classification`
- `7-5-1-2-return-ref-and-partial-access-lowering`
- `7-5-1-3-literal-index-projection`
- `7-5-1-4-public-constructor-debug-projection`

Six exact invariants bind those sections to code and executable proof:

- `RT_BYTECODE_LOWERING_CLASSIFICATION_001`
- `RT_BYTECODE_RETURN_REF_PARTIAL_001`
- `RT_BYTECODE_LITERAL_INDEX_PROJECTION_001`
- `RT_BYTECODE_PUBLIC_CONSTRUCTOR_DEBUG_001`
- `VM_BYTECODE_NUMERIC_PARTIAL_DOMAIN_001`
- `RT_BYTECODE_RESOURCE_COUNTER_BOUNDARY_001`

Eight new behavior-lock tests cover recursive classification, RETURN/REF and
partial-access lowering, source/path-aware constructor debug projection,
literal-index width and overflow behavior, numeric and partial-access domains,
instruction-counter overflow, exact partial-access encoding, and declared
resource-limit boundaries:

- `lowering_support_predicates_cover_nested_call_sizeof_and_lvalue_shapes`
- `return_and_ref_builtin_lowering_preserve_exact_fail_closed_boundaries`
- `public_module_constructors_preserve_source_and_path_projection`
- `literal_indices_preserve_integer_widths_and_reject_nonliteral_or_overflow`
- `numeric_primitive_and_partial_access_domains_are_closed`
- `instruction_counter_overflow_is_fail_closed_and_preserves_total`
- `partial_access_lowering_emits_exact_opcode_and_typed_operand`
- `declared_resource_limits_accept_empty_tables_and_reject_first_excess`

All eight scanner identities are cataloged and denominator-mapped. The reviewed
authority transactions accepted 70 focused new-test fact selections and 163
residual selections backed by already-cataloged tests. The final bytecode
partition is 233 `specified` executable facts and four retained structural
`spec_gap` facts: the public `BytecodeModule` type and the `ClassLike::name`,
`ClassLike::base_name`, and `ClassLike::methods` trait signatures. Those four
were not assigned fake executable behavior.

## Verification-tool defect closed in the same batch

The reviewed authority-selection implementation substituted a derived
`TEST_AUTO` identity when a denominator row already named a curated catalog
test. The focused regression
`test_catalog_mapped_curated_test_identity_is_reused` established the
assertion failure before the correction and passes afterward.

The normative metadata contract now requires exact `catalog_test_id` reuse,
reciprocal invariant backlinks, fail-closed missing bindings, and recoverable
application. `VERIFICATION_CODE_SPEC_REFRESH_QUARANTINE_001` owns that rule.
The complete 11-fact
`scripts/verification/code_spec_authority_selection.py` module is now
`specified`; one singular authority group was accepted and all 31 unrelated
candidate groups were explicitly withheld.

## Focused remote-builder proof

All execution and coverage proof ran on `scena-rust-builder` in the detached
validation checkout
`/home/johannes/projects/trust-platform-p18-hir-b2ff0fbd`.

- The final focused bytecode selection passed 10/10.
- The complete bytecode module trace observed 118/118 selected identities.
- The exact authority trace observed 51/51 identities.
- The exact new-test trace observed 8/8 identities.
- Live scanner staleness validation passed for all eight new catalog records
  against 4,465 scanner facts.
- A focused bidirectional audit passed for 237 bytecode facts, eight new
  catalog rows, six invariants, six specification anchors, and the exact four
  retained structural facts.
- A second focused audit passed for all 11 authority-selection tool facts and
  their exact specification, invariant, and metadata-gate binding.

Retained builder artifacts include:

- `target/gate-artifacts/verification/p18-bytecode-module-postcatalog-130b1ec3-trace.json`
- `target/gate-artifacts/verification/p18-bytecode-authority-130b1ec3-trace.json`
- `target/gate-artifacts/verification/p18-bytecode-new-tests-130b1ec3-trace.json`
- `target/gate-artifacts/verification/p18-bytecode-module-mapping/mapping-receipt-130b1ec3.json`
- `target/gate-artifacts/verification/p18-bytecode-complete-final-report/code-spec-coverage.json`

The repository-wide metadata validator and catalog-staleness wrapper still
exit nonzero on pre-existing global metadata debt outside this batch. The
batch-local schema findings were corrected, the six new invariants produce no
remaining filtered validator finding, and the focused live-scanner staleness
join is clean. This evidence does not conceal or reinterpret the global
failures as passing proof.

## Post-batch accounting

The clean final report is bound to remote revision
`2411ff36a83a9a2a3474325ad0a390a6a51e5016` and reports:

- 17,233 live code facts;
- 3,340 `specified`;
- 13,858 remaining `spec_gap`;
- zero report findings;
- 233 specified bytecode facts plus four retained structural bytecode gaps;
- 11 specified authority-selection transaction facts.

Relative to the preceding startup report, this batch adds 244 specified facts.
The `spec_gap` count falls by 243 because the authority-tool correction also
introduced and then specified one previously absent live code fact.

No broad `just fmt`, `just clippy`, `just test`, or `just test-all` gate ran.
Those gates remain reserved for the exact frozen `VERIF-P18-016` candidate.

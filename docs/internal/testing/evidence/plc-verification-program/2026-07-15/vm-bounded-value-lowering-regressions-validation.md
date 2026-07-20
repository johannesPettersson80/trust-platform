# VM Bounded-Value Lowering Regression Validation

- Date: 2026-07-15
- Final Rust and metadata checkpoint: `6710d4cc3a96a543c5a6c68d354453240cb5807d`
- Report source checkpoint: `3c63c89f2f9a435046c4b7791d9c3246bcc5ffcc`
- Evidence posture: product regression and test-adequacy evidence; this is not
  IEC conformance proof or release proof.

## Product Defects Found

Real tests exposed five type-preservation defects in the runtime harness
lowerer after the bounded-value contract became stricter:

- binary expressions could lose the HIR-resolved numeric result type between
  the register and stack lowering paths;
- contextual initializer literals could bypass the declared storage type;
- a function return assignment could lose the function's declared result type
  when the expression target type was unknown;
- untyped `FOR` start, end, and implicit-step literals could lose the control
  variable type; and
- named and positional function arguments and named function-block inputs
  could lose the formal parameter type.

The last three defects reproduced in the real OSCAT core dependency fixture.
Before the fix, 5 of its 10 embedded Structured Text cases failed, including
`DIR_TO_DEG` and REAL-valued function/function-block call paths. After rebuilding
`trust-dev`, all 10 cases passed.

These are implementation defects against the committed truST value-semantics
contract. No IEC deviation was added.

## Tests First and Fixes

The focused regressions were exercised red before their owning product edits
and are now cataloged against `VM_SEAM_DECLARED_TYPE_001`:

- `TEST_VM_OSCAT_BINARY_TYPE_PRESERVATION_001` covers matching register and
  stack behavior for an OSCAT-style REAL expression;
- `TEST_VM_FUNCTION_RETURN_TYPE_MATERIALIZATION_001` covers an untyped
  arithmetic expression assigned to an `INT` function result;
- `TEST_VM_FOR_CONTROL_TYPE_MATERIALIZATION_001` covers `SINT` loop bounds and
  the implicit step;
- `TEST_VM_CALL_ARGUMENT_TYPE_MATERIALIZATION_001` covers named and positional
  REAL function arguments plus named function-block input/output bindings; and
- `TEST_VM_OSCAT_OOP_CORE_REGRESSION_001` runs all ten real OSCAT core ST tests
  through the rebuilt `trust-dev` command path.

The product changes are narrow to lowering context propagation. Formal
parameter lookup and enclosing-storage lookup live in separate small modules;
the main lowering module remains below the repository's 1,000-line split
threshold.

## Focused Validation

On `trust-builder`, using the isolated validation checkout and
`CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate`:

- `cargo test -p trust-runtime --test bounded_value_semantics`: 6/6 passed;
- `cargo test -p trust-runtime --test initializer_architecture`: 13/13 passed;
- `cargo test -p trust-runtime --test bytecode_vm_differential`: 8/8 passed;
- `cargo test -p trust-runtime runtime::vm::type_policy::tests --lib`: 4/4
  passed;
- the rebuilt `oscat_oop_core_st_unit_tests_pass`: 10/10 embedded ST cases
  passed; and
- all four case-table generator checks passed.

The metadata graph at the Rust checkpoint contained 501 records. Catalog
staleness validated 156 committed records against 3,942 discovered facts, with
151 mapped and 3,791 still explicitly unmapped.

## Final Remote Checkpoint

The final heavy gate ran once from the clean isolated checkout
`$HOME/projects/trust-platform-bounded-values-final-validation` at
`6710d4cc3a96a543c5a6c68d354453240cb5807d`.

Disk preflight found only 33 GiB free because the generated shared target used
52 GiB. With no active compiler processes, only that generated target was
removed, restoring 85 GiB before the cold run.

- `just fmt`: passed;
- `just clippy`: passed;
- `just test-all`: passed, including the OSCAT core regression;
- `cargo test -p trust-runtime --test api_smoke`: 3/3 passed;
- `cargo test -p trust-runtime --test debug_control`: 20/20 passed;
- `cargo test -p trust-runtime --test complete_program`: 1/1 passed; and
- `cargo test -p trust-runtime --test runtime_reliability`: 4/4 passed.

The later verification-only checkpoints `f4529bf6` and `3c63c89f` refresh the
existing specification-source audit rule and the reviewed report-test
baselines. Their three owning modules pass 47/47; no second broad Rust run was
performed.

After the report refresh and evidence indexing, the canonical focused
verification suite passed 768/768 in 825.881 seconds. Both metadata entry
points validated 502 records, the Phase 16 report-only product fence passed,
and catalog staleness validated 156 committed records against 3,942 scanner
facts.

## Generated Report Refresh

All 15 installed report pairs were regenerated one at a time from a pristine
detached worktree at `3c63c89f` with timestamp
`2026-07-15T13:48:56+02:00`. Every pair passed its production at-rest
validator before its bytes were staged.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `88b9facdf6477c058c0ef2c47efbad1b37d9ac59bcb1ce91f85f50c81da60021` |
| Coverage-matrix gaps | `3a8286897153f09455302f315c5ef08e416f50d66cd026a7b9c8c4bf84cd8013` |
| Malformed-input coverage | `6eb0c2d393c802b8ba9ca0648cd14ef356b0257573b66beeaaa9adbbd3ed84fa` |
| Unmapped-test debt | `498d2c9b97dbdee351ed25c66e5900e0b7fa59e74f4743b02eea41c062b24649` |
| Test-refactor assessment | `c8c702368f414ffe93c57d139b21b285bc5caf573009059e6ccd452bf5595fa9` |
| Ignored-test inventory | `53ef9b9e51c77be0e603ca14e7188592a49c39c1e4165be9a80cc4e83891007c` |
| Phase 5 suite audit | `0bfa631c5403f189fa8aa25dfacb777222761ad33aebf47e144bf4a77127c5b6` |
| Invariant-seed audit | `bc737f617cf4b32db7946b8e8977dbc3ed4d2361c6661074aec291d748f373c9` |
| Specification completeness | `44817795c7901ea0722949b34e3afe7b675ba66e9f6e37ae7e7fbf1252ec44b0` |
| Requirement/oracle audit | `1af2a7555f921829329a3073fb8724d6494eeee7cd73f0bcbb9b90d6e74d2174` |
| Conformance alignment | `bfa9ed1bcc4ff6c1f591fd455cabfdd3ed19d7ea2cc158070c1f7308ea386f7c` |
| Runtime-anomaly audit | `a4fc00b85fa102b4e8a36866d0e2fc4b57c38ceb2c73a3eb18b7a60ada6cbdb7` |
| Fuzz-program audit | `76c4c07911a32bd60021c66e4e4da3741afbc8199ba5010c848312ecc6f724ee` |
| Mutation program | `35854dd07f991f5159d3afa0c827f78b803c2768aea964e124db724e1799206d` |
| Specification-source audit | `f69f20b406bc0d9e21e74f253abe866eb3e4e08dfb41357f59771ba15a38c263` |

The mutation report still contains two caught validator mutants and zero
survivors; no mutation shard was re-executed because this batch changed neither
its runner nor measured selectors.

## Honest Remaining Posture

- Test-class completeness is 7/32 required class slots.
- Specification completeness reports 24/53 invariants unspecified, three
  expected-result tests unbound, and 31 specification-gap cells.
- Requirement/oracle mapping reports 25/53 eligible and 28 missing.
- Conformance alignment remains 0/21 explicitly linked.
- The fuzz audit still reports six surface gaps.
- No suite, workflow, approved proof producer, CI enforcement state, or proof
  level changed in this closeout.

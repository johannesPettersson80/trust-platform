# HIR Mutation Hardening Evidence - 2026-04-28

Branch: `architecture/hir-mutation-hardening`

Scope:

- `crates/trust-hir/src/db/symbol_import.rs`
- `crates/trust-hir/src/type_check/const_eval.rs`
- `crates/trust-hir/src/db/queries/collector/variables.rs`
- focused tests in `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs`

## Baseline Commands

`symbol_import.rs` baseline:

```sh
cargo mutants -p trust-hir --file crates/trust-hir/src/db/symbol_import.rs --output target/gate-artifacts/hir-mutants-baseline/symbol_import --jobs 4 --timeout 300 --minimum-test-timeout 60 --caught
```

This first run used copy mode and failed after writing results because `/tmp` ran out of space while copying the workspace. The recorded mutation outcomes were still usable. Follow-up runs used `--in-place` and each run was checked for `/* ~ changed by cargo-mutants ~ */` residue.

`type_check/const_eval.rs` baseline:

```sh
cargo mutants -p trust-hir --file crates/trust-hir/src/type_check/const_eval.rs --output target/gate-artifacts/hir-mutants-baseline/type_check_const_eval --in-place --timeout 300 --minimum-test-timeout 60 --caught
```

`collector/variables.rs` baseline:

```sh
cargo mutants -p trust-hir --file crates/trust-hir/src/db/queries/collector/variables.rs --output target/gate-artifacts/hir-mutants-baseline/collector_variables --in-place --timeout 300 --minimum-test-timeout 60 --caught
```

## Baseline Results

| Target | Total | Caught | Missed | Unviable | Timeout |
| --- | ---: | ---: | ---: | ---: | ---: |
| `symbol_import.rs` | 30 | 6 | 22 | 2 | 0 |
| `type_check/const_eval.rs` | 34 | 16 | 18 | 0 | 0 |
| `collector/variables.rs` | 121 | 77 | 39 | 5 | 0 |

The survivor classes were:

- cross-file namespace and type-shape imports, callable symbol-kind type remapping, source/target `TypeId` collisions, and initializer ID translation;
- const-eval literal/name/scope/paren/unary/binary operator and error-specific paths;
- aggregate initializer validation, required nested defaults, array repetition, reference `NULL`, integer bounds, FB initializer member legality, and configuration/global scope handling.

## Fixes And Tests Added

Added `hir_mutation_hardening.rs` under `semantic_type_checking` with 22 focused tests covering:

- cross-file import of scalar aliases, arrays, structs, unions, enums, pointers, references, subranges, strings, wstrings, FB/class/interface types, namespace scopes, callable return/property type IDs, and initializer ID translation;
- const-eval CASE labels, scope-chain constants, type-size expressions, array-index expressions, arithmetic operators, divide/modulo/exponent error paths, and cyclic/default diagnostic specificity;
- aggregate initializer diagnostics with exact codes/messages/locations where meaningful;
- nested struct/union required defaults, array initializer/repetition paths, reference `NULL` legality, function-block member legality, and program/global scope collection.

Production HIR fixes found by mutation:

- direct array repetition defaults such as `ARRAY[1..2] OF SINT := 2(200)` now validate the repeated element against the array element type;
- non-repeat call expressions used as array defaults now produce a `TypeMismatch` instead of being silently accepted;
- the no-op `LINT` branch was removed from default range bounds because evaluated defaults are already `i64`; overflow beyond `LINT` is reported by const evaluation before range checking.

## Final Mutation Commands

`symbol_import.rs`:

```sh
cargo mutants -p trust-hir --file crates/trust-hir/src/db/symbol_import.rs --output target/gate-artifacts/hir-mutants-after-tests/symbol_import --in-place --timeout 300 --minimum-test-timeout 60 --caught
```

`type_check/const_eval.rs`:

```sh
cargo mutants -p trust-hir --file crates/trust-hir/src/type_check/const_eval.rs --output target/gate-artifacts/hir-mutants-after-tests/type_check_const_eval --in-place --timeout 300 --minimum-test-timeout 60 --caught
```

`collector/variables.rs`:

```sh
cargo mutants -p trust-hir --file crates/trust-hir/src/db/queries/collector/variables.rs --output target/gate-artifacts/hir-mutants-after-tests/collector_variables --in-place --timeout 300 --minimum-test-timeout 60 --caught
```

## Final Results

| Target | Total | Caught | Missed | Unviable | Timeout |
| --- | ---: | ---: | ---: | ---: | ---: |
| `symbol_import.rs` | 30 | 28 | 0 | 2 | 0 |
| `type_check/const_eval.rs` | 33 | 33 | 0 | 0 | 0 |
| `collector/variables.rs` | 122 | 117 | 0 | 5 | 0 |

There are zero missed mutants and zero timeout mutants in the final focused shard. No equivalent mutants were accepted as survivors.

## Focused Validation

```sh
cargo test -p trust-hir --test semantic_type_checking hir_mutation_hardening -- --nocapture
```

Result: 22 passed.

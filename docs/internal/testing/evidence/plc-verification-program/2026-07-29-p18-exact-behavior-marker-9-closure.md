# Phase 18 exact-behavior-marker closure

Date: 2026-07-29

## Result

This coherent metadata-and-focused-execution batch closes all nine current
exact-behavior markers. The nine invariants already had complete reviewed
behavior partitions and accepted catalog mappings, but still retained the
stale `cataloged_behavior_tests` marker and eighteen coverage cells that said
catalog association was pending.

The exact catalog join proves:

```text
invariants                 9
declared behaviors        51
catalog test identities   75
mapped behavior refs      51
missing behaviors          0
extra behavior refs        0
missing catalog tests      0
```

The frozen gap report moves:

```text
exact-behavior debt   20 -> 11
gap_open cells       191 -> 173
```

All eleven remaining exact-behavior-debt records are the historical
`P17_HISTORICAL_CODE_*` placeholders. They are not retired by this batch
because live code-spec facts still terminate in those gaps.

The test denominator remains:

```text
4,400 live tests
4,400 catalog_mapped
    0 reviewed_nonmapping
    0 confirmed missing tests
```

This is `VERIF-P18-003` through `VERIF-P18-006` progress, not row closure.

## Checkout and bootstrap

- Canonical source: `/home/johannes/projects/trust-platform`
- Branch: `plc-verification-program`
- Baseline commit:
  `0d31ff122d27a97f588008a2d304ae0b4a4c5f0b`
- Accepted metadata commit:
  `e2c20f68a440bb656652216bee656fcb75e96a11`
- Builder host: `scena-rust-builder`
- Accepted destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-marker-0d31ff12`
- Baseline comparison destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-marker-baseline-0d31ff12`

Both destinations were detached at their stated commit. `AGENTS.md`, all 25
files under `.codex/skills/**`, and the ignored private pre-tool hook were
manually copied from the canonical source and verified byte-exact before
validation. The accepted destination remained clean apart from ignored
bootstrap files.

The protected local runtime diff remained byte-identical:

```text
01458d43c50cd7dfe5cc6eb4649ac1a5a7b8595e61e26d2ce3e76c0b52de4fb4
```

No production code changed. This metadata correction therefore used green
behavior-lock execution rather than a behavioral expected-red.

## Focused remote proof

The exact accepted SHA passed:

| Command boundary | Result |
| --- | ---: |
| `cargo test -p trust-hir --test semantic_type_checking` | 310/310 |
| `cargo test -p trust-syntax --lib` | 41/41 |
| syntax `lexer_pragmas`, `lexer_literals`, `lexer_common`, and `parser_expressions` integration binaries | 29/29 |
| `cargo test -p trust-runtime --test iec_precedence_trace_cases expression_precedence_trace_cases` | 1/1 |
| `python3 -m unittest scripts.verification.frozen_catalog_gaps_tests` | 2/2 |

The frozen report digest is:

```text
dfdd72600b25f792e3fba53d865447670dde8d0ef5374146772096bdcc3dca56
```

No workspace-wide `just fmt`, `just clippy`, `just test`, or `just test-all`
ran.

## Global-debt isolation

`python3 scripts/validate_verification_metadata.py` remains red on downstream
Phase 18 debt. With the private hook present, the clean baseline and accepted
commit each produced exactly 1,610 findings and byte-identical logs:

```text
cdb6cbc9d85f22f8cf593d99ea8ed47cd7e69147f684064ba4953068bb6c1cbb
new findings     0
removed findings 0
```

The required complete changed-path planner used `--intent test-refactor` and
all nine invariant paths. It retains the same repository metadata failure
class at exit 6:

```text
6f4264b4a7880aa847d76c965ee55813629854e798f0967da7d5bc42e95c2992
```

Catalog staleness remains red on the same downstream metadata population:

```text
9481d5ff5aaa1f7c3760584ad4a8b208e5f8b81d706477d4f9cc2bfbff3cba6f
```

Those global failures do not override the exact baseline/post isolation and
do not authorize closing `VERIF-P18-003`, reachability, mutation, adequacy,
hardware/UX, platform, candidate, release, or independent-audit rows.

The machine-readable review lock is
`verification/p18/review-locks/exact-behavior-marker-9-closure-lock.json`.

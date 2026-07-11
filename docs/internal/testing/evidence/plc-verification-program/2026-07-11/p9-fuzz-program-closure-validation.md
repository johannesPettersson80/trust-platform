# Phase 9 Fuzz Program Closure Validation

Date: 2026-07-11
Implementation commit: `c25c62f87b6fe4d768c4ce47a416d1d464cff157`
Report and board checkpoint: `c0ff87ebc34411ea2138d8408e6b4b665638489b`

## Scope

This closure covers `VERIF-P9-001`, `VERIF-P9-002`, `VERIF-P9-003`,
`VERIF-P9-004`, and `VERIF-P9-006`.

`VERIF-P9-005` remains open. The program states that every minimized crash must
become a deterministic regression, but no exhaustive machine registry joins
every raw or minimized crash artifact to a tracked regression, reviewed test
identity, and deterministic runnable command. Ignore rules, an empty artifact
directory, or policy prose do not satisfy that universal handoff.

This slice inventories and reports existing executable mechanisms. It runs no
fuzz campaign, adds no runtime or product behavior, changes no CI or suite
wiring, closes no specification gap, and creates no proof or invariant
coverage.

## Tests First And Review Hardening

Contract, discovery, source-binding, report, hostile-shape, and at-rest
corruption fixtures were added before the final implementation. Adversarial
review then found and closed these fail-open boundaries before the source
checkpoint:

- the Rust candidate vocabulary now recognizes the reviewed fuzz/property,
  constrained randomized/arbitrary smoke, proptest, and QuickCheck forms while
  unmodeled framework markers fail visibly;
- all target identities, surface rows, association rationales, tiers, execution
  bases, and corpus rules are exact-value pinned rather than inferred from
  names or prose;
- execution claims bind raw script and complete workflow bytes, tracked script
  modes, unique trigger and reviewed job blocks, exact active command sets, and
  effective Cargo default-workspace membership. Dead control flow, uncalled
  functions, comments, echoes, CRLF normalization, top-level workflow defaults,
  duplicate blocks, wrong packages, and extra commands fail;
- every bounded Rust smoke must resolve to a live `not_ignored` scanner fact;
  ignored or conditional facts cannot retain a runnable tier claim;
- both JSON schemas are closed and semantic-digest pinned, so live-compatible
  weakening of refs, consts, enums, cardinalities, or string constraints fails;
- nested cargo-fuzz manifests, orphan targets, traversal, symlinks, malformed
  TOML/list shapes, ineffective ignore rules, tracked generated paths, unsafe
  outputs, non-canonical JSON, Markdown tampering, and consistent semantic
  report forgery all fail closed; and
- `VERIF-P9-005` plus every standing stop row remains guarded open at generation
  and at rest.

The residual Rust census limitation is explicit: ordinary `cfg` evaluation and
parent-module reachability are not compiler-proven by the lexical scanner.
`wired` means a reviewed required command path, not observed execution or
passing evidence.

The final Phase 9 contract/report/source test surface passed 46/46 tests after
hardening. A separate adversarial re-review returned clear with no remaining
high, medium, or low finding.

## Measured Inventory

| Measure | Result |
| --- | ---: |
| Tracked cargo-fuzz manifests | 2 |
| Cargo-fuzz targets | 5 |
| Bounded Rust fuzz/property smokes | 6 |
| Rust smokes joined as `not_ignored` | 6 |
| Total executable inventory facts | 11 |
| Required surfaces | 8 |
| Explicit associations | 12 |
| Direct / partial associations | 9 / 3 |
| Cargo-target / smoke-only / partial-only / unmapped surfaces | 2 / 1 / 2 / 3 |
| Surface gaps | 6 |
| Primary PR-smoke / nightly / manual-extended targets | 7 / 1 / 3 |
| Additional nightly targets | 2 |
| Wired / planned / manual-only targets | 7 / 1 / 3 |

The six gaps are HIR/lowering input and LSP incremental edits as partial-only,
bytecode container/instructions as smoke-only, and PLCopen XML, config files,
and HMI schema payloads as unmapped. They are report debt, not new
specification gaps and not claims that no repository test exists.

## Report Reproduction

The Phase 9 report and twelve historically bound report pairs were generated
from clean source commit `c25c62f87b6fe4d768c4ce47a416d1d464cff157`
with timestamp `2026-07-11T20:43:20+02:00`, one pristine detached worktree per
report. Every generator and matching at-rest validator exited zero, every
staged copy was byte-identical to its validated output, and all temporary
worktrees were removed and pruned.

| Report JSON | SHA-256 |
| --- | --- |
| `test-class-completeness.json` | `e09758d15a4c6dd776f7b8085956f6bbaff19bc96bee95286753db86129dbdd4` |
| `coverage-matrix-gaps.json` | `39d10fd3e2d88e2754fe719e5db2ff23f9e89f0cfd0c4a64f69f5fbacc85e909` |
| `malformed-input-coverage.json` | `6ba925a57ef2442d27126007bf60f8023d53405d6c0e3b75acf9f130f3ff0e11` |
| `unmapped-test-debt.json` | `6552e59cd8f187994ecf0e989261620220ffec7afcbb5b983da32a0f60876b1d` |
| `test-refactor-assessment.json` | `0d7f28902305b0ddd575f64c35b93e41ba7768b5921fefefeae871e20c9ca68c` |
| `ignored-test-inventory.json` | `0660bf114414abb9dfcbbfd7cf05d31f92729481d692a6df9cf0804c472c4a2a` |
| `invariant-seed-audit.json` | `2e630c3cf7867c610895389c04fdd65965724927fe9b2314bb34dcc3cfdbffc9` |
| `spec-completeness.json` | `6b6131b895e11d1852d40d80600ce77450912c9d57bb6d9b0aa402141d6d5b09` |
| `phase5-suite-audit.json` | `c87e6f96a77268cb5f3fe5b64fa34e7e7c69df41afbae0596fe612b56afa3c55` |
| `requirement-oracle-audit.json` | `ff00e8caf3ff5e91d6f91b868020814ce51c46a369e023863716eb51e918c25c` |
| `conformance-alignment.json` | `7a720500527ef2d19878ae33d548dcc24d5be7a04376eac9f9afdfa867c37ec4` |
| `runtime-anomaly-audit.json` | `0835b62e1d3bac71671a27995e7a0b62dac16d7715c53a210ab99de78bdae44a` |
| `fuzz-program-audit.json` | `5bde7a684d85e4f660d8445436fa5e5151646210e8d10fc290fe57c9d7211de2` |

The Phase 9 input SHA-256 is
`sha256:d1b851222c327bf24110471c4f78a34b81f32bc516770cf5eb9688f8792091a9`;
its durable Markdown SHA-256 is
`9194a217334d61b719d54e7415559eeac92e087d67a07c71ca77e2aec6d46851`.

The existing-test catalog, tooling-selftest report, four generated case tables,
and bytecode-validator mutation report were not regenerated. The mutation
report remains byte-identical at SHA-256
`fd8b7a7ab1f73b639b198678782072dee19dd5c2f0f9b19fb945dedf22069d4a`.

## Local Validation

The canonical focused runner discovered every verification `*_tests.py` module
under its documented collision exclusion and passed 549/549 tests in 685.704
seconds at the clean source checkpoint. At the report checkpoint, both metadata
entry points validated 334 records before this closure row was indexed, and all
thirteen report pairs passed at-rest validation.

Additional local results:

- verification-tooling self-tests: 27/27;
- ignored-test join: 88 discovered, 88 registered, 63 unknown, 0 catalog-mapped;
- catalog staleness: 6 records against 3,816 scanner facts;
- VS Code registration: 456 facts, 38 files, 38 registrations;
- refactor proposals: 1 proposal, 0 redirects, 6 catalog records, 3,816 facts;
- all four `gen_cases.py --check` invocations passed;
- `py_compile`, both metadata entry points, all report validators,
  `git diff --check`, and the clean-worktree assertion passed.

## Remote Validation

The retained isolated builder clone
`$HOME/projects/trust-platform-p9-validation-c25c62f87` ran broad gates at the
clean source checkpoint with the warmed
`$HOME/.cache/codex-targets/trust-platform-gate` target directory. The host was
`x86_64-unknown-linux-gnu` with `rustc 1.95.0 (59807616e 2026-04-14)` and
`cargo 1.95.0 (f2d3ce0bd 2026-03-21)`.

```text
just fmt                       2.21 seconds, exit 0
just clippy                   49.22 seconds, exit 0
just verification-veryquick 359.91 seconds, exit 0, 549/549 Python tests
just test-all                549.64 seconds, exit 0
```

The clone was then advanced to report checkpoint
`c0ff87ebc34411ea2138d8408e6b4b665638489b`. The focused runner passed 549/549
in 240.21 seconds, both metadata entry points reported 334 records, all thirteen
staged report pairs passed at rest, and the clone remained clean.

## Preserved Boundaries

- The board is 155/231; the five flips are exactly `VERIF-P9-001`,
  `VERIF-P9-002`, `VERIF-P9-003`, `VERIF-P9-004`, and `VERIF-P9-006`.
- `VERIF-P9-005` remains open, as do all standing stop rows guarded by the
  Phase 9 generator and at-rest validator.
- All 34 specification gaps retain `resolution_status = "open"`.
- All 52 invariants remain unvalidated at proof level `S0`: 44 `spec_gap` and
  8 `gap_open`.
- The Phase 9 report evidence uses `proof_kind = "none"` with empty linked
  invariants, tests, and specification gaps.
- CI remains unchanged and report-only; no workflow invokes the new audit.
- Product/runtime Rust, product tests, editors, xtask, Cargo manifests and
  lockfile, `justfile`, changelog, version, skills, agent instructions, public
  docs, conformance inputs, cases, mutation inputs, invariants, spec sources,
  spec gaps, suites, gate inventory, and matrix are unchanged. The only
  crate-tree file is the ADS fuzz workspace's generated-output `.gitignore`.
- The Phase 2 scanner remains unchanged; Phase 9 owns its separate nested-fuzz
  inventory denominator.

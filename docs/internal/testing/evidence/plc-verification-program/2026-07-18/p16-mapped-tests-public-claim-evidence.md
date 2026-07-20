# Phase 16 Mapped Tests And Public-Claim Execution Evidence

Date: 2026-07-18

## Complete Mapped-Test Denominator

The durable mapped-test execution at source commit
`b7e3ab47172ff2e857c22a4173d78a639a6616dd` ran every one of the 242
`status = "mapped"` catalog commands on `trust-builder`. All 242 passed; none
failed or timed out. The canonical result is
`p16-mapped-test-execution.json`, SHA-256
`c70479523fca5640947d6446f497e3b5bdc67d4d78cb0dc7654a59834d8baee2`.

The three public-claim contract tests relevant to this closeout passed in that
exhaustive run:

| Test ID | Exit | Duration | Retained log SHA-256 |
| --- | ---: | ---: | --- |
| `TEST_RUNTIME_BEHAVIOR_LOCK_TRACE_001` | 0 | 242 ms | `cf3cde4953724099b914c8e27badc18e1a42a372f023ef577406520230367bf9` |
| `TEST_DEBUG_BEHAVIOR_LOCK_TRACE_001` | 0 | 285 ms | `65b287ca2ef4f9fafb0718b91e1664b29c3aba35caa175a98b8b9e5d4110ae1e` |
| `TEST_RELEASE_SOURCE_BUILD_TRACE_001` | 0 | 272 ms | `5dc034d35520e68c4262da2245c7db4271d5a9ab623d8fa61dfd41682abbcb52` |

The runtime and debugger behavior-lock tests also retain their existing
producer-authentic lock-baseline/lock-compare proof pairs. Together with the
normative contract in `docs/specs/24-release-evidence.md`, the passing complete
mapped denominator establishes the explicit test, suite, and durable-evidence
traceability required by the narrowly worded behavior-lock claim. It does not
turn that claim into repository-wide validation.

## Isolated Source Build

The documented release build was executed on `trust-builder` from clean commit
`1be81590c89114f1a0abb76d04a5f4f7fe41fb91` in
`~/.cache/trust-platform-source-build-isolated-1be81590/trust-platform`.
The checkout's dedicated parent contained no `open-ot-ref` sibling before,
during, or after the run.

```text
command: cargo build --release -p trust-lsp -p trust-runtime -p trust-debug
started: 2026-07-17T23:36:11+00:00
finished: 2026-07-17T23:46:53+00:00
duration: 642 seconds
exit status: 0
rustc: 1.97.0 (2d8144b78 2026-07-07)
cargo: 1.97.0 (c980f4866 2026-06-30)
post-run source status lines: 0
```

The four expected shipped binaries existed and were executable after the
build. `trust-runtime --version` reported `trust-runtime 0.24.53`, and
`trust-runtime --help` exited zero.

| Binary | SHA-256 |
| --- | --- |
| `trust-lsp` | `452e1779dc75f743bfc5ad823f80b86b857a47e253002c701cd261ef09b07b64` |
| `trust-runtime` | `7e44670e8a3030e0ac0dd41676d9bd9c1b6d0d46ac2d50473887c83374ad8af3` |
| `trust-debug` | `df7f76546d2ee653d6adae0379abb2bbb2e0f2d181f0557dc6c57a0094530d7b` |
| `trust-harness` | `bffc3ab58c65044a6f7f3d77d310cc89842f7506f47277361fd169bf83c04d7d` |

`cargo metadata --locked` resolved all five OpenOT packages from the public
Git source
`https://github.com/johannesPettersson80/open-ot-experiments.git` at full
revision `137f0e765f085c262651f479be35298b836ac891`. Their manifests were in
Cargo's Git checkout cache, not a sibling source tree. The build log also
records `open-ot-carriage`, `open-ot-definition`, and `open-ot-shm` compiling
from that pinned URL.

## Disposition

- `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` remains `spec_updated`. The two
  mapped tests, their lock proof pairs, and the exhaustive 242/242 execution
  establish G1 targeted evidence, but the registered public-claim contract
  requires a validated invariant. Its explicit remaining obligation is an
  approved causal broad-remote gate.
- `SPEC_GAP_SOURCE_BUILD_PUBLIC_CLAIM_001` remains `spec_updated`. Its mapped
  contract test and the successful isolated build establish the source-build
  fact on Linux x86-64, but the invariant likewise remains G1 pending an
  approved causal broad-remote gate.
- `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` and
  `SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001` remain open. A Linux x86-64 source build
  cannot establish native multi-platform or physical-device support.
- This evidence does not close `VERIF-P8-002`, `VERIF-P16-006`, or either stop
  gate, and it does not change CI enforcement. The fixed broad-evidence
  producer was not run: its 60 GiB home and 3 GiB `/tmp` preflight could not be
  met on the shared builder (25 GiB and 2.5 GiB were available after this
  build), and the canonical remote checkout contained unrelated dirty work
  that was preserved.

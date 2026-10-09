# A1 release integration

Candidate branch: `integrate/runtime-portability-a1` in the isolated
`trust-platform-portability-a1-integration` worktree. Base:
`ecbcb08ddd12360d057c8e25b6502b9f9d6772c3` (PR #129 merge). Version: 0.24.71.
The original A1 branch remains at 78d4e641f, A2 at 9f62fd091, and neither original
worktree is reset. Historical A1 run 3 is scope evidence, not evidence for this
rebased release candidate. A3 remains outside this release candidate.

## Integration decisions

- Retain all main security and LSP lifecycle corrections, including macOS/Windows
  identity handling and the concurrently security update.
- Preserve main's Salsa query dereferences; no `returns(copy)` attributes return.
  The lockfile adds only the core libm edge and workspace version changes relative
  to main; security dependencies are already upstream.
- Fold the canonical instruction rewrite already landed by #129; keep the
  canonical release guard and mounted-volume lease tools. Drop the already-landed
  MySQL/LSP Clippy correction when replaying A1.
- Merge changelog entries without dropping the #129 entries. Keep generated
  diagram changes from both branches. Existing run-3 evidence remains byte-exact.
- Replace the obsolete 200-line rulebook ceiling with assertions that the approved
  safety/checkout/release sections remain present; retain skill metadata and routing
  checks. The canonical rulebook intentionally contains the #129 workflow lessons.
- Preserve original worktrees and the bootstrap stash. Never stage Python caches.

## One final validation batch

Read the workflows and release guard before freezing. Independent review covers
this complete diff, source tests/imports, feature gates, manifests, rule parity,
version consistency, and upstream overlap. No validation is launched before review.
The frozen candidate is committed before the exact-SHA guard; source stays frozen
until all independent commands finish. Any actual failure retains its logs and
unrun dependent steps. Corrections require review before a consolidated follow-up;
report failures to the root agent before any further execution unless applicable
integration retry authorization is already explicit. A3-specific approval does not
authorize retries for this release candidate.

| Required proof | Command / owner | Prerequisite and evidence |
|---|---|---|
| Exact candidate, rule parity, clean tree, diff, strict smoke | `release_candidate_guard.py prepare` local stages | Final committed SHA and unchanged base; artifact command ledger |
| VS Code lint/compile/full tests and capture lifecycle | `prepare` remote_vscode and remote_docs_capture_lifecycle | Version files changed; Xvfb and builder-owned TMPDIR; logs |
| Formatting, cross-target warnings, supply chain, architecture safety/full map, all-target/all-feature Clippy | `prepare` remote stages | Mounted target with 80 GiB free, lease, actual toolchain versions recorded |
| Native workspace unit/integration/doc tests including runtime vertical, LSP and debug | `prepare` remote_test_all (`just test-all`) | One run only; no duplicated pre-push full test suite |
| Discovery parity | `prepare` remote_mp001_parity | Reuse native binaries from test-all |
| Isolated portable core and layouts | `cargo +1.95.0 check --locked -p trust-runtime-core --no-default-features --target TARGET`, both thumbv7em-none-eabihf and riscv32imac-unknown-none-elf | Same committed tree, leased target; installed targets; library check only |
| Isolated portable native behavior | `cargo +1.95.0 test --locked -p trust-runtime-core --no-default-features --test portable_foundations` | Runs ordered-map/no_std branch without workspace std feature unification |
| Portable library feature edges | `cargo +1.95.0 tree --locked -p trust-runtime-core --no-default-features --target TARGET -e features,no-dev` | No claim from dev-dependency feature edges |
| Diagram rendering and drift | `scripts/render_diagrams.sh`; `python3 scripts/check_diagram_drift.py` | Exact renderer prerequisites/cache before freeze; compare tracked output, no unrecorded source mutation |
| Windows LSP dependency compile and path hygiene | `scripts/check_test_path_hygiene.sh`; `cargo check -p trust-lsp --tests --target x86_64-pc-windows-gnu` | Supplement only commands not covered by prepare; no duplicate LSP native tests or Clippy |
| Remaining pre-push contracts | `python3 scripts/check_iec_log_paths.py`; `python3 -m unittest scripts.tests.test_diagram_workflow -v`; `scripts/runtime_mesh_tls_stability_gate.sh --iterations 8` | Same frozen candidate; eight stability iterations are one planned suite, not corrective retries |
| Agent routing contract | `python3 -m unittest scripts.verification.skill_routing_tests -v` | Covers the amended rulebook assertion and existing skill routing |
| Native Windows/macOS and remaining CI workflows | Current-head GitHub jobs after push | Cross-compilation does not replace these jobs |

The guard owns its required command sequence and artifact. Supplemental commands
have a separate ledger under the same frozen batch identity. Refresh floating
stable once before the batch, record stable and pinned 1.95 versions, and select
an explicit stable override in the clean builder checkout for CI parity. No
blanket claim of hardware support or cross-platform execution is made.

The guard currently creates its VS Code temporary directory under `/tmp`; inspect
that filesystem and concurrent Xvfb/browser jobs separately from the mounted Cargo
target before launch. Do not infer browser capacity from the target free space.

## Publication order

Do not launch a heavy builder run while A3 holds the coordinated builder slot.
Do not merge a version candidate while #129's 0.24.70 release is incomplete.
After passing guard and supplemental evidence: push once, collect all required
CI failures and review findings before editing, then use guarded merge and
verify-release. A later commit or moving base invalidates the exact-SHA artifact.
A2 is integrated and released only after the A1 release is closed; it retains
its own review and exact-SHA validation. A3 is not published by this task.

## Independent review before the batch

Read-only integration review found three preparation defects, all corrected before
freezing: the obsolete AGENTS line ceiling, omitted pre-push supplemental steps,
and the isolated no-default native core lane. Final review found no further
integration issue. It independently confirmed that upstream-only files match main,
all original A1 Rust files match 78d4e641f, HIR/LSP Rust has no delta from main,
and canonical AGENTS plus 19 skill files match. No test or validator was run by
the reviewer. Review input digests before this record was appended:

- This command map: `15568640f7fa932c48809122b4c32a6e252cd525afc7f1227838e34947dff9b5`.
- `scripts/verification/skill_routing_tests.py`: `c631f5a2d6b9d18b57a3d191bfc876221d159b471bf9270954e63b65948a945f`.

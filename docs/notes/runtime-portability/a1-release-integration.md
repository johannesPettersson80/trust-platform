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
| Windows LSP dependency compile and path hygiene | `scripts/check_test_path_hygiene.sh`; `env -u CC -u CXX cargo check -p trust-lsp --tests --target x86_64-pc-windows-gnu` | Supplement only commands not covered by prepare; no duplicate LSP native tests or Clippy |
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


## First integration batch and prepared corrections

Candidate `9e5a5896ff7de49855cf6db13f2b607aa3af0d00` failed its first integration
batch. The supplemental ledger records 14 passing steps and one Windows LSP
compile setup failure: the orchestration exported native `CC=cc` / `CXX=c++`
into a cross build. Its corrected command explicitly unsets both variables;
the guard already separates its native and cross environments. The same earlier
#129 correction is retained in `pr129-remediation-run5/windows-command-correction.json`.

The exact-SHA guard passed its required stages through Clippy, including 519
VS Code tests, then `just test-all` reported 6,555 native tests passed, one
fixture setup failure and ten ignored tests before stopping. The unsupported
Git-marker fixture bound a Unix socket beneath a long mounted-target TMPDIR,
exceeding the platform socket pathname limit before its product assertion.
Later workspace suites, discovery parity and the guard's final clean check
were unrun. The separate supplemental clean check had passed. No automatic
rerun occurred, and this candidate is not push-ready.

The prepared native correction uses an actual FIFO special file in a deliberately
long project directory. Specification 22 rejects every unsupported filesystem
object through one generic branch; the existing rejection assertion remains.
The test verifies creation status and FIFO type before that assertion, and removes
its task-owned fixture. It is Unix-only and uses the POSIX `mkfifo` utility,
listed in the [Open Group utilities specification](https://pubs.opengroup.org/onlinepubs/9699919799/utilities/contents.html).
The Linux builder provides `/usr/bin/mkfifo`; native macOS remains a CI gate.
This introduces no dependency, unsafe block, global environment/CWD mutation or
shortened assertion. All other temporary/build outputs keep the selected storage.

Raw first-batch guard artifact/logs and the supplement ledger are preserved under
`/home/johannes/projects/.artifacts/runtime-portability-a1-integration/run-1/`.
After independent correction review and local commit, the proposed follow-up is
one new exact-SHA `prepare`, the corrected Windows LSP supplemental command,
and the skill-creator `quick_validate.py` check for the amended release skill.
Unchanged portable-core and diagram source evidence remains the first-batch proof;
full native tests and guard stages rerun because the test source changes the SHA.
This follow-up is prepared only and requires explicit integration authorization.
After batch completion, the canonical release skill and this candidate both clarify
native-only compiler overrides, explicit unsetting for cross checks, and reading
prior failure-ledger command corrections before freezing. This is included in the
complete correction before the next review/commit; no running candidate was changed.


## Integration batch 2 and PR #130 corrections

The separately authorized batch on `c1e0f68f9101a7c1fe33cce52d701143ab18a33c`
passed the exact-SHA guard, corrected Windows LSP compile and skill validation.
The workspace run reported 8,321 passing tests, zero failures and 24 ignored tests;
the VS Code run reported 519 passing tests. The unsupported-marker FIFO assertion
ran successfully. Raw proof is retained under
`/home/johannes/projects/.artifacts/runtime-portability-a1-integration/run-2/`.
PR #130 was then pushed. PR #129's v0.24.70 release is independently closed.

All initial PR #130 jobs finished before corrections began. Strict MkDocs rejected
five links to the missing public projection of specification 34. The projection,
navigation, reference index and search expectation are now prepared; source-only
research/checklist links retain their repository destinations. Link warnings and
strictness are unchanged.

Windows and macOS native suites passed on their first attempts (8,182 and 8,313
tests respectively). Linux CI's green status concealed a first-attempt failure in
`managed_runtime_lifecycle_runs_through_the_shipped_cli`: fleet stop reported
`failed to read control response`. The workflow's second attempt passed. Both
attempts and its retry-rescued marker are retained under `pr130-native-linux/`;
this is a blocker requiring a root-cause correction, not clean first-attempt proof.
The complete failure ledger is retained as `pr130-failure-ledger.json` alongside
these artifacts; `pr130-retry-rescue-findings.json` records the hidden Linux
failure with its original log digest. Security review found no issues; general automatic review did
not run because of its usage limit. Independent review remains separate evidence.

The completed second batch also showed unconditional target deletion before
workspace tests despite ample free space. The prepared guard correction keeps
compiled artifacts when the existing 80 GiB floor is met. Below that floor it
uses the same validated idle-removal helper, prepares the target again and
rechecks space. Six authored orchestration regressions exercise the real path
and lease helpers with controlled free-space readings: warm retention, successful
reclamation, insufficient reclaimed space, a busy lease, invalid readings and a
failed measurement. No lease, ownership, exact-SHA or capacity rule is relaxed.

The fleet correction belongs to the confirmation phase after one valid shutdown
acknowledgement. A listener can accept a subsequent status connection while its
process is exiting; previously any response-read failure aborted stop. A typed
transport-interruption error now marks only socket write/read failures. During
post-acknowledgement confirmation these consume the existing 31-probe / 30-sleep
budget, without resending shutdown. Only a later failed connection confirms
`stopped`; exhausting the budget reports `stopping` and retains the advisory PID.
Initial status, authentication, malformed/oversized envelopes, wrong IDs and
configuration errors remain strict. The CI log does not preserve the underlying
I/O kind, so it cannot distinguish reset from timeout; the new real-TCP fixture
forces accepted EOF before refusal to pin the demonstrated lifecycle gap.

Four real-TCP tests cover EOF then refusal, EOF then healthy/protocol failure,
confirmation exhaustion and strict ordinary/initial status. A fifth unit test
checks transport error classification without misclassifying configuration I/O.
Static review caught the initial test setup reserving ports through the
scaffolding availability check and an unavailable direct `thiserror` dependency;
both are corrected before any execution. The original shipped-CLI test remains
unchanged and runs in the guard's workspace suite. These are authored assertions,
not test results.

### Proposed consolidated follow-up, not yet authorized

Finish all corrections and independent review before committing a new candidate.
Freeze its SHA and run the release-guard Python suite and skill validator as the
modified guard's prerequisites. Run the Docs Captures publication sequence on the
builder in a separate clean checkout of the same SHA with manually verified
canonical rules, so generated assets cannot dirty the frozen guard checkout:
media inventory generation, public IA/link and example-link checks,
`mkdocs build --strict`, assets and search checks. This is the actual failed stage;
no duplicate browser capture is needed for unchanged rendered assets.
Then run exact-SHA `prepare` once; its native workspace suite owns the fleet
regressions and shipped-CLI integration test, with no additional test-all. Retain
unchanged isolated portable-core and diagram evidence from integration batch 1.
Collect independent failures, leave dependent stages unrun, and never retry
automatically. These commands have not run against the prepared corrections.

### Independent correction review

A separate read-only agent reviewed the complete docs/guard correction and the
fleet correction. The fixture's held-port scaffolding conflict and missing direct
error-derive dependency were corrected and re-reviewed; no actionable findings
remain by inspection. Review did not execute tests, a formatter or validators.
Final fleet source SHA-256 values:

- `lifecycle.rs`: `c9f5fd01a321bca8134e00cef923c033273bf0090fc94f3ae4beed098e7c5126`.
- `lifecycle/stop_confirmation_tests.rs`: `37d9b34bed1c8ab7fe03589476fd6e141edbfd5168f04dbb3905521f0cf8a459`.

The reviewed canonical release skill SHA-256 is
`1d8d47f8bf343f7c24400f208e26de360e24b2ca041a3960d78488973fbd9df3`;
prepare script `3d56278762aa62bd0051373dd5922becde9e13aa04e0c396e2f5bf71b805668b`;
guard tests `7552db9c89d6859466200cda6ba4b44ddf68c971b787ac180b210dfb673d42d3`.
This record's final reconciliation and review paragraph were appended after that
source review. The proposed consolidated follow-up remains unrun and requires
explicit integration validation authorization.

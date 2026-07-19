# Eighteenth-review final validation

Date: 2026-07-18

Validation ran in the isolated `trust-builder` checkout
`~/projects/trust-platform-p18-validation`. The final focused and metadata pass
used clean commit `79e8024ea036888c5bbf8d374accf33528db8bb6`.

## Builder preflight

The builder initially had 1.8 GiB free because the generated shared truST
target occupied 60 GiB. No truST compiler process was active. Only that
generated target and this slice's temporary report staging directory were
removed; an unrelated active `cardine-cli` process and all source worktrees were
left untouched. The final validation ended with 100 GiB free under
`/home/johannes` and 5.4 GiB free under `/tmp`.

## Product and workspace gates

| Gate | Result | Wall time |
| --- | --- | ---: |
| `just fmt` | pass | 2.79 s |
| `just clippy` | pass | 184.62 s |
| `cargo test -p trust-runtime --test api_smoke` | 3 passed | 103.40 s cold build |
| `cargo test -p trust-runtime --test debug_control` | 20 passed | 3.58 s |
| `cargo test -p trust-runtime --test complete_program` | 1 passed | 2.64 s |
| `cargo test -p trust-runtime --test runtime_reliability` | 4 passed | 2.61 s |
| `just test-all` | pass, zero failing test targets | 716.29 s |

The VS Code extension validation was performed earlier in the same accepted
implementation chain: `npm ci`, lint, compile, and 461 extension tests passed,
followed by a real Extension Development Host DOM assertion and screenshot of
the visible peer-topology degradation state.

## Verification gates

The first focused run correctly failed four reviewed census tripwires after the
product tests had passed:

- coverage cells changed from 5 gap-open / 64 covered to 6 / 63;
- scanner facts changed from 4,023 to 4,035;
- Rust facts changed from 3,220 to 3,229; and
- VS Code facts changed from 458 to 461.

Those values already matched the regenerated live reports. Only the four
explicit test expectations were refreshed, and the four owning tests passed
together before the final report rebind. The final clean builder run then
reported:

- focused verification: 856/856 passed in 1,085.56 s;
- metadata gate: 781 records, Phase 16 product fence report-only, exit zero in
  285.48 s;
- verification tooling self-tests: 33/33 passed in 128.29 s; and
- `git diff --check` and the tracked/untracked worktree check: clean.

One self-test replay command was first issued without changing into the remote
checkout. Python refused before reading repository files; the corrected command
above passed. This was an invocation error, not a test or product failure.

## Generated reports

All fifteen report pairs were regenerated and at-rest validated from clean
`f514021eef1395b1d6aed0f8a8f77eb67cd7b40a`, one detached worktree per pair.
Their canonical tracked copies are byte-identical to the validated builder
outputs. The final digest table is in
`eighteenth-review-final-report-rebind.md`.

## Boundaries

No workflow, suite definition, approved proof producer, skill, agent rule,
board row, or enforcement flag changed. `VERIF-P16-007` and `VERIF-P16-008`
remain open for independent review. The final report and validation evidence
rows use `proof_kind = "none"` and do not promote product proof.

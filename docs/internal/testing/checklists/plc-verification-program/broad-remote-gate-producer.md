# Broad Remote Gate Evidence Producer

Status: contract implemented; no broad evidence has been produced.

`scripts/record_broad_remote_evidence.py` is the only reviewed producer for a
committed-file `broad_remote_gate` record under the `pr` suite. It has no
command, host, suite, platform, or output-path override. The reviewed command
is exactly:

```text
ssh trust-builder 'cd "$HOME/projects/trust-platform" && mkdir -p "$HOME/.cache/codex-targets/trust-platform-gate" "$HOME/.cache/codex-targets/trust-platform-gate-tmp" && export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-gate" TMPDIR="$HOME/.cache/codex-targets/trust-platform-gate-tmp" && just fmt && just clippy && just test-all'
```

The producer accepts one or more `--invariant <ID>` arguments. Selected
invariants must share one area and each must already name at least one catalog
test. Every linked test must be a mapped `pr`-tier Rust unit or integration
fact, absent from the ignored-test register, and backed by a committed case
file. The broad command alone is not positive test-execution identity because
Cargo can exit zero when a feature-gated test is compiled out. After the broad
gate, the producer therefore reruns every selected catalog command with a fresh
`TRUST_VERIFY_*` stamp, validates the complete same-run case artifact, and
requires every committed case to pass. The evidence links the canonical sorted
invariant set, exactly the union of their committed test IDs, and a closed
per-test execution record containing command, discovery identity, run ID,
case-file/artifact digests, result summary, and exit status. Callers cannot
supply a weaker test list or execution record.

Before execution, the producer requires:

- a clean local worktree at a full 40-hex `HEAD`;
- a clean `trust-builder:$HOME/projects/trust-platform` worktree at the same
  full commit;
- the reviewed `trust-builder-linux-x86_64` platform; and
- the required remote disk audit, at least 60 GiB available under
  `/home/johannes`, and at least 3 GiB under `/tmp`; and
- `broad-remote-gate.py v1` in the named `pr` suite's
  `approved_proof_producers` list.

After all commands, it deletes transient remote case artifacts and rechecks
both worktrees and revisions. A failed broad or selected command, missing,
stale, incomplete, blocked, or failing artifact, cleanup failure, dirty tree,
revision change, unapproved producer, unknown link, low disk, or platform drift
writes no evidence. A successful run records the exact broad command, disk
preflight, exit status, start and finish timestamps, duration, platform,
local/remote commit equality, derived links, and positive per-test execution
records. It appends atomically only to the tracked, non-ignored canonical
`verification/evidence-index.toml` through the shared proof-output writer.

The record uses `proof_kind = "none"` and `proof_scope =
"broad_remote_gate"`. It is cumulative promotion evidence only after a linked
invariant has valid targeted proof at the same commit or an ancestor and every
current invariant test still matches a bound positive execution record, is
absent from the current ignored-test register, and names the same committed
case-file digest and exhaustive case-ID set recorded as passing. Duplicate,
missing, or invented case IDs disqualify the record from current promotion.
For a multi-invariant record, the complete recorded test union must still match
current catalog and case contracts, while each invariant being promoted must
own a non-empty subset of that reviewed union.
Historical evidence remains valid when the invariant later gains tests, but it
ceases to qualify for current promotion until those tests receive new broad
evidence. This record is not targeted behavior proof, a spec-gap closeout, or a
release result. Adding the producer to `approved_proof_producers` changes no
suite command, include semantics, CI workflow, or enforcement state.

No broad command is run and no evidence row is created merely by installing or
validating this producer contract.

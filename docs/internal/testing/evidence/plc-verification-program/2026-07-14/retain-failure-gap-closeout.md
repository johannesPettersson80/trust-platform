# Runtime Retain Failure Gap Closeout

Date: 2026-07-14

## Scope

This record closes `SPEC_GAP_RUNTIME_RETAIN_FAILURE_001`, whose blocking
question asked which retain read, decode, compatibility, and write failures
must block lifecycle progress and how those failures are surfaced. The owning
contract is now written in `docs/specs/11-runtime-engine.md` section 6.7.

The closeout does not claim exhaustive persistence fault injection or durable
power-loss proof. Those obligations remain visible as
`RT_SAFE_RETAIN_001.missing = ["retain_failure_matrix_depth",
"broad_remote_gate"]`.

## Product Finding And Proof

The focused two-value retain trace found a real transaction bug: when an
earlier retained value was compatible and a later value failed declared-type
migration, the earlier value was applied before the runtime returned the
error.

- Red revision: `ec1d23b7be9c36e46599ea1154e2568e7d9a65cc`
- Red evidence: `EVID_TEST_RUNTIME_RETAIN_FAILURE_ATOMICITY_001_RED`
- Observed mismatch: `accepted_first expected 70, observed 111`
- Green revision: `8ba872b86c363e30517b7f1b33cb702788b0dd2f`
- Green evidence: `EVID_TEST_RUNTIME_RETAIN_FAILURE_ATOMICITY_001_GREEN`

The fix stages every canonicalized retained value and migration/orphan event,
then commits them only after the complete snapshot validates. The same case
file and execution-contract digests bind both proof rows.

## Honest Posture

`RT_SAFE_RETAIN_001` advances only to targeted `G1`. The case proves atomic
rejection for one late incompatible retained value. It does not independently
prove every filesystem, shutdown, cadence, migration-shape, or power-loss
failure. Broad builder validation and deeper fault-matrix work remain separate
obligations.

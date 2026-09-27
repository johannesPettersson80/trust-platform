# Recorded-run requirements and comparison

Companion to Specification 35 section 17.3. This truST workflow adds no IEC
language behavior. The evaluator consumes generic numeric channels and never
assumes PID signals, a course project or a named scenario.

Each run retains its evidence class (`model-predicted`, `ST-executed`, or
`physical-recorded`), exact program/model/parameter-schema revisions, initial
state, event sequence, seed, duration and requested sample times. Actual
parameter values are retained per run and may differ between candidates;
parameter identity/schema and all experiment conditions must match. Evaluation
returns an owned deeply frozen snapshot, so later source edits cannot change a comparison.

Requirements declare a unique ID, channel, time window and numerical limits.
Supported measurements are maximum/mean absolute error and RMS error against a constant or
recorded reference; directional overshoot beyond a declared step target;
settling/recovery into a target band by a deadline with a declared hold time;
signal or state bounds; maximum absolute adjacent-sample rate; total saturation
duration; and switching count with a declared minimum change threshold.
Overshoot uses max(0, direction*(PV-target)); step amplitude must be nonzero.
Mean error is the arithmetic mean over the declared sample grid. Error statistic
`rms` is sqrt(mean(error squared)), with the same unweighted sample grid and
quality checks; it is calculated in Control Studio, not required in ST. Saturation
duration uses a left-held signal on each adjacent sample interval. Switching
counts adjacent changes strictly exceeding the declared threshold. Settling
means all observed samples from the selected arrival time through the requested
window end remain in band, with at least the declared hold duration. The
window must extend through deadline plus hold duration; otherwise the
requirement is invalid because late settling cannot be fully observed.

Each requirement window must start/end on the declared sample grid. Complete
coverage requires every expected sample exactly once, in order, with contiguous
scan sequence, exact timestamp agreement (no implicit jitter tolerance), and good finite values for the requirement's channels. Missing,
forced, bad, duplicated, gapped or truncated coverage makes that requirement
Inconclusive; it never creates a passing measurement. A fully observed violation
is Fail. Other independently complete requirements may still Pass or Fail.
The overall result is Fail if any independently observed requirement fails,
otherwise Inconclusive if any requirement is inconclusive, otherwise Pass.
No requirements is Inconclusive. Invalid specifications reject the evaluation.
Passing establishes only the declared observed conditions, never general stability.

Current/candidate comparison requires identical conditions, channel identities,
units, requirement definitions and evidence class. Mismatched conditions produce
Inconclusive with no ranking. Failed or inconclusive candidates cannot rank.
Among qualifying runs, an explicitly chosen measured requirement is minimized;
no hidden composite score or silent default ranking is permitted.

## Imported-run evaluation contract

Evaluation accepts current and optional candidate recorded-run contracts,
declared requirements and an optional ranking objective. An import does not
independently qualify acquisition or prove hardware observation. Ordinary
recordings without declared initial state, events, seed and parameter identity
cannot be upgraded into matched experiments. Missing information is a specific
evaluation failure. These are data/service inputs, not JSON form fields or
additional recording-management controls on Validate.

Imports are limited to 2 MiB each and 64 requirements. Evaluation runs in a
worker with a ten-second budget and cancellation. Replacement imports clear
previous evaluation. Editing requirements or the ranking objective also invalidates
the retained result on the host before it can be saved again. Invalid files, missing current/candidate evidence and
unmatched conditions produce explicit messages; no candidate is applied.
Save retains exact imported contracts, requirements, optional ranking objective
and calculated results in v4. Restoring shows the saved evidence class and
requirement values, not a new observation. The original contracts remain
available for export through the existing result service.

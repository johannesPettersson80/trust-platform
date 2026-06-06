# S4b-4b ARM Unfenced Evidence

Date: 2026-06-06
Host: aarch64 Cortex-A76, 4 cores, taskset pinning available

## Summary

No unfenced hazard fired on this ARM host within the bounded ladder below. This is a documented non-reproduction, NOT proof of safety. The fenced gates stayed clean, and unfenced runs produced clean retention loss only: stale=0, rejected=0, poll_errors=0.

## Live Harness

Top unfenced envelope:

`cargo run -p open-ot-live-harness -- run --mode litmus --unfenced --append-mode encoded --cap 96 --per-source 2000000 --poll-sleep-us 0 --timeout-ms 120000`

Result:

`summary: mode=litmus fence=unfenced append_mode=encoded cap=96 head_abs=192000068 lost_count=2000000 delivered=150293 lost=1849708 lapped=111268 retries=47226 rejected=0 poll_errors=0 stale=0`

Fenced contrast, same envelope:

`summary: mode=litmus fence=fenced append_mode=encoded cap=96 head_abs=192000068 lost_count=2000000 delivered=162009 lost=1837992 lapped=123435 retries=44992 rejected=0 poll_errors=0 stale=0`

Additional unfenced ladder attempts:

- cap=512, per_source=200000, write-record, default poll sleep: stale=0, rejected=0, retries=13526
- cap=128, per_source=500000, write-record, poll_sleep=0: stale=0, rejected=0, retries=372867
- cap=128, per_source=1000000, encoded, poll_sleep=0: stale=0, rejected=0, retries=980071
- cap=128, sources=4, per_source=200000, encoded, poll_sleep=0: stale=0, rejected=0, retries=790687

## truST Capstone

Escalated contrast:

`OPENOT_CAPSTONE_RUN_UNFENCED=1 OPENOT_CAPSTONE_PER_SOURCE=32 OPENOT_CAPSTONE_CAPACITY=256 OPENOT_CAPSTONE_TIMEOUT_SECS=180 cargo test -p trust-runtime --test openot_capstone openot_capstone_unfenced_contrast -- --ignored --nocapture`

Fenced reference:

`summary: mode=capstone fence=fenced append_mode=st-fb cap=256 head_abs=3440 lost_count=63 delivered=67 lost=0 lapped=0 retries=0 rejected=0 poll_errors=0 stale=0`

Unfenced experiment:

`summary: mode=capstone fence=unfenced append_mode=st-fb cap=256 head_abs=3440 lost_count=63 delivered=67 lost=0 lapped=0 retries=0 rejected=0 poll_errors=0 stale=0`

`unfenced_evidence: outcome=non-reproduction fence=unfenced stale=0 rejected=0 poll_errors=0 retries=0 lapped=0 delivered=67 lost=0 lost_count=63`

Source reconciliation:

- source 10: expected_total=33 delivered=33 lost=0 reconciled=33
- source 30: expected_total=33 delivered=33 lost=0 reconciled=33
- system source 0: expected_total=1 delivered=1 lost=0 reconciled=1

## Interpretation

The S4b-4b experiment path is implemented and records stale, rejected, and poll-error evidence without making unfenced firing a CI gate. On this Cortex-A76 run, both live-harness and truST capstone stayed clean under the bounded ladder. This is a non-reproduction, not evidence that the unfenced transport is safe.

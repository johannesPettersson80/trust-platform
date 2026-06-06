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

The experiment is sound, not vacuous: `FenceMode::Unfenced` genuinely drops the section 4.3 release/acquire fences. In `open-ot-ref/crates/open-ot-shm/src/lib.rs:348-358`, `release_before_clobber` and `acquire_before_recheck` are gated on `FenceMode::Fenced`, so the unfenced runs above executed with those fences removed.

A live ARM leak was never the load-bearing proof that the fences matter, and a non-reproduction does not weaken that proof. The fences are established as load-bearing in `open-ot-ref/crates/carriage/src/concurrent.rs` by:

- the **fenced** loom model (`loom_rejects_mid_write_overwrite_or_reads_old_complete_record`), which proves the consumer never accepts torn or pre-publish bytes when the section 4.3 fences are present; and
- the production-store **fence-hook usage** test (`owned_store_protocol_uses_fence_hooks`), which proves the real publisher/consumer invoke those fence hooks.

The same file documents that the **unfenced** hole is, by nature, not reliably detectable by tooling: `loom_control_unfenced_model_does_not_expose_weak_memory_hole` (concurrent.rs:1103-1139) is an intentionally passing documentation test whose comment states correctness "rests on the section 4.3 release/acquire fences, not on loom detecting this relaxed-reordering gap." A hardware stress that fails to surface the hole on a given core/envelope is therefore the *expected* result, fully consistent with the model — not a sign the fences are unnecessary.

Conclusion: the section 4.3 fences are proven load-bearing (fenced loom model + fence-hook usage). This ARM run is a complementary best-effort attempt to also exhibit the hardware-level hole directly; it did not fire within the bounded ladder on this Cortex-A76, which is the documented-expected outcome — recorded as a non-reproduction, not proof of safety.

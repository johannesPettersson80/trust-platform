# Independent Scope B source review

Reviewer: `/root/b_independent_review`. Read-only review of the Scope B worktree based on `df427259cc387a7a79fb81132be23e48ea1493d4`. Canonical root AGENTS and complete skills parity verified against `/home/johannes/projects/trust-platform`; relevant architecture and native test skills read. No source edits, builds, tests, formatting commands or probe access performed. This record is external evidence only.

## Findings and dispositions

1. UART record lifecycle ordering was incomplete: NUM/MEM/PREP/GPIO/STACK could follow the watchdog reset identity. Corrected with an explicit complete record sequence before field parsing. Reinspection follows each firmware emission through main, numeric, GPIO and reset phases; the sequence agrees. The native synthetic fixture retains duplicate order and a swapped-memory-record mutation now asserts rejection. These tests remain unexecuted by this reviewer.
2. STACK_PEAK preceded the final GPIO safe-output/fault probes. Corrected: it now emits after gpio-dropped in main::run; the verifier requires it to cover every MEM and STACK measurement.
3. ELF entry validation did not verify the reset vector actually used by hardware. Corrected: the Reset vector must carry the Thumb bit and equal the ELF entry, whose address is already constrained to the code partition.
4. Board harness could accept queued pre-reset serial data into the capture. Corrected: while halted, a bounded one-second drain archives preexisting bytes separately before the commanded run reset.
5. OpenOCD operations were unbounded. Corrected: install and start invocations each have a 90-second timeout. The UART acquisition itself remains a bounded 25-second operation, with timeout interpreted as acquisition completion, not as a hardware pass.

## Additional source reconciliation

- GPIO native test executes both image input levels at all four actual POU depths and checks committed output, result, nominal deadlines, zero overruns and fault state. It does not claim physical button or pad coverage.
- Firmware scanner now includes the excluded firmware source directory and excludes target output. Its authored test supplies an unregistered unsafe function and atomic import in firmware, alongside generated target code that must be omitted.
- Policy JSON semantic comparison against HEAD shows only fourteen unsafe-site records, six concurrency records, four forbidden edges and two allowed edges added. Existing entries are unchanged despite a large formatting diff. All fourteen new unsafe line bindings match the formatted sources.
- The architecture diagram accurately separates the source-free engine, board adapter, independent application sector and instrumentation. It explicitly declines hardware/fit claims from source or diagrams.
- HAL/PAC methods, RTC register accessors, TIM2 prescaler/ARR logic, GPIO initial-low conversion, watchdog API and Cortex-M PRIMASK semantics were checked against published crate archives: stm32f4xx-hal 0.23.0, stm32f4 0.16.0, cortex-m 0.7.7 and cortex-m-rt 0.7.5. PRIMASK is_active means interrupts were enabled, so restore behavior is correct. Cortex-m-rt paints unused RAM and linker reservations separate the application sector and MSP.

## Remaining evidence boundaries

No additional definite implementation blocker found by source inspection. Compilation, linker fit, native suites, actual UART framing/timing, GPIO levels, watchdog reset and stack/heap measurements remain validation claims owned by the batch, not this review.

The root cargo metadata graph excludes the standalone firmware package. The added firmware allowed/forbidden edge records therefore document intent but do not make the root architecture graph enforce firmware dependencies. The source manifest currently respects those boundaries. Safety-source scanning does cover firmware. Record this limitation rather than presenting root graph success as standalone dependency-edge enforcement.

Physical package/PCB markings, manual button transitions and optical LED inspection remain explicitly unverified; no user-presence dependency was introduced.

Source identities are pinned in `independent-reviewed-source.json`; external reviewed batch scripts are pinned separately in `independent-reviewed-scripts.json`. No validation result or execution authorization is inferred from these hashes.

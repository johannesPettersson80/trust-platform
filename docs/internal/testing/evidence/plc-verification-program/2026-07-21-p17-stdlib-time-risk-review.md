# P17 stdlib time risk review

Status: implementation-side owner review; independent `VERIF-P17-006` acceptance remains pending.

Review date: 2026-07-21
Acknowledgement expiry: 2026-10-21
Owning role: `language`
Area: `compiler_iec`

This review covers only parser-derived risk signals on the exact body digests in
`verification/code-spec-map.toml` for
`crates/trust-runtime/src/stdlib/time.rs`. A body digest change, risk-signal
change, or expiry invalidates the acknowledgement. This review does not remove
or suppress any risk signal.

## stdlib-time-registration

The 30 `StandardLibrary::register` facts carry `state_mutation`. Each fact is a
distinct mutation of the standard-library dispatch table during library
construction. The mappings inherit the corresponding reviewed time-function
contract; the acknowledgement covers only the registration mutation boundary,
not the inherited function semantics.

## runtime-clock-dispatch

`runtime_clock_value` carries `host_time` because its `CURRENT_DT` branch reaches
the host clock. Its `TIME` branch remains elapsed-runtime input. The mapping is
bound to the exact dispatcher digest and the existing clock-boundary gap.

## current-dt-clock-boundary

`current_dt` carries `host_time` and `fault_mapping`: it reads
`SystemTime::now()` and maps host-time/conversion failures to
`RuntimeError::Overflow`. The expected clock, epoch, resolution, and backward
clock behavior remain explicitly unresolved in
`SPEC_GAP_CURRENT_DT_CLOCK_BOUNDARY_001`; this acknowledgement is not a gap
closure.

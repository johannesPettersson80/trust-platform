# Read-only B-R4 call-entry simplification proposal

Source remains frozen. No edit, compilation, test, formatter, link or board operation. The reported linked shortfall is 320 bytes below the unchanged 16 KiB upper margin; no new saving has been measured.

The ordinary root dispatcher and deferred-call start now contain duplicated entry-input materialization:

- initial-local payload length check and exact existing BytecodeDecode message;
- per-value before_value_clone call and indexed Value clone;
- supplied-presence length check and exact existing invalid-bytecode message;
- Vec<bool> copy.

Introduce one small `copy_call_inputs(runtime, frame, initial_locals: Option<&[Value]>, present: Option<&[bool]>) -> Result<(), RuntimeError>` helper in the dispatcher module, with a targeted `#[inline(never)]` boundary to retain one portable-context machine-code body. Call it from both existing sites. Keep both callers' frame lookup/error ordering; invoke runtime.initialize_frame only at its current site after the helper. Initializer ownership/reservation logic is unchanged. Initializer calls have no supplied initial locals or presence; they must still skip ordinary frame initialization.

This removes duplicate logic introduced by the continuation refactor, rather than a PLC function or guard. It preserves clone/accounting order, failure messages, supplied-NULL handling and allocation behavior (`Vec<bool>::clone` and slice `to_vec` both copy the same supplied slice). It also lets input-copy temporaries return before initializer execution. Source duplication is established; whether machine-code sharing recovers 320 bytes is unverified and requires a newly authorized link.

Separately, the Clippy manual_clamp correction is `.max(1).min(VM_MAX_CALL_DEPTH)` -> `.clamp(1, VM_MAX_CALL_DEPTH)` after checked_mul. VM_MAX_CALL_DEPTH is the fixed positive constant1024, so the range is valid and the checked-overflow order is unchanged. No limiter changes or lint suppression are needed.

# Read-only run-4 dispatcher owner proposal

No source edits or product execution. Root's run-4 batch is still active.

Independent read of the retained builder run-4 map confirms execute_with_buffers at 0x080294c4 has 0x3e2c (15,916) bytes of text; copy_call_inputs and finish_call have separate symbols, but start_call has none. The source calls start_call from opcode 0x09, so the call owner is inlined into the common dispatch loop.

The narrow candidate is `#[inline(never)]` on existing dispatch::continuation::start_call. It already owns user binding, an owned PreparedCall, optional optimized result, continuation reservation, callee setup and failure restoration. Those temporaries belong to the call operation; ordinary DEFAULT/APPLY initializer dispatch cannot invoke a user POU and should not retain their stack slots. This is an ownership boundary already present in source, not a new execution mode or function subset.

The unchanged common dispatcher frame is 2,336 bytes by the independent ELF review; it occurs twice on the measured main-construction path. Retaining start_call as a real call boundary may shrink both copies on that path without another heap object. No saving is guaranteed. When start_call itself invokes frame initialization, its own frame remains live, so the same saving cannot be claimed for every call-entry path. Required preparation, instantiation, cycles and all four depth measurements must still pass.

No change to arguments, guards, output/edge restoration, cleanup, limits, metadata allocation charging, operand floors or shared fuel is proposed. Keep the existing native assertions. Do not weaken the stack/flash margin or enlarge the memory partitions.

The larger state-ownership proposal is deferred. In particular Box::new(deep_build(...)) would keep deep construction on the stack and does not solve it. Any future final-location allocation must be shallow, charged/fallible and shared by initial construction and transactional restart; it is unnecessary scope while this measured call-owner boundary remains unresolved.

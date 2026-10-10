# B-R4 run1 call correction independent source review

Accepted by `/root/br1_static_registry`, source only. Four-file identity `1c8d1c0a6155ac90bbb5fbc95708b748e2bb1642ba1ec08db8f01c5a6573c66e`. All before-source hashes match run1's frozen manifest; the reviewed after hashes match the held worktree. No source edit, formatter, compiler, test, link or board command was performed.

copy_call_inputs extracts identical root/deferred local-copy and parameter-presence logic into one non-inlined concrete monomorphized helper. It preserves the local bounds error, per-value admission-before-clone order, partial-copy failure behavior, parameter presence bounds/message and boolean-vector content. Ordinary initialize_frame still occurs after successful copying. Initializer entry still excludes ordinary initialization and supplied presence metadata. Both callers retain their existing cleanup ownership on helper failure. This is shared implementation and a potential footprint/frame reduction, not a measured saving.

FrameStack's clamp(1, VM_MAX_CALL_DEPTH) is equivalent to the previous max(1).min(...) because the constant maximum is 1024, greater than the minimum; no dynamic reversed bound or new reachable panic is introduced. Checked multiplication/error handling precedes clamp unchanged.

The host fixture changes only four occurrences of the reserved identifier pointer to ref_arg. The explicit NULL, omitted default, suspended-local IN_OUT and output assertions, including expected result 133, remain unchanged. Run1 failed during fixture compilation, so its intended semantic assertion remains unverified until an authorized follow-up batch.

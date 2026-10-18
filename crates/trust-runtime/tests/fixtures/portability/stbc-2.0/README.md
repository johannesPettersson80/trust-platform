# STBC 2.0 portability fixture

This fixture is separate from the tracked OSCAT 1.1 artifact. It contains a 25 ms
periodic task, TON, an FB through an interface, a rising-edge input, retained
activation state, a bounded array, nonzero defaults
and a method-local reference initialized to an array element selected by the
changing method input (1, 2, 3 in rotation). `REF(history[delta])` uses persistent
instance storage, as permitted by IEC 61131-3 Ed.3 §6.4.4.10.2; it does not introduce
mutable scalar initializers or references to temporary storage. A4 must pin both
the selected element and per-call reference initialization in its oracle.

Generate on the builder as part of A3's consolidated batch:

```sh
cargo run --locked -p trust-runtime --example portability_fixture
```

The revised `program-v2.stbc` and explicit-field disassembly were generated in A3
run 5. The 6,164-byte artifact was decoded, admitted and compared with fresh
production by the passing run-6 authoring suite. On 18 October 2026 (S3b1) the
compiler began writing the optional LINE_TABLE section (`0x0011`, specification 12
§6.13), and the three artifacts here were regenerated with the command above. The
S3b1 review same day dropped the section's per-line extents and reordered nested
statements, so they were regenerated again; `program-v2.stbc` is now 6,412 bytes.
They are saved worktree artifacts pending commit. The native test decodes the
artifact through the portable consumer and compares it with fresh explicit 2.0
production. Regeneration belongs in a recorded validation batch. A4 must execute it with exact 10 ms logical samples and
freeze the activation/state trace; A3 decoding is not execution evidence.

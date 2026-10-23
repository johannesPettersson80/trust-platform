# Independent source review of the corrected compiler-free fixtures

Reviewed the final correction in:

- `crates/trust-runtime/tests/runtime_core_compiler_free_load.rs`
- `crates/trust-runtime/src/bytecode/encoder/construction/globals.rs`
- `docs/specs/12-bytecode.md` (new function-static capability paragraph)
- `CHANGELOG.md` (matching capability entry)

Four-path SHA256, sorted path/NUL/file bytes/NUL:
`bfa3cffcfd1274aab12c4b7e68c432787a889e21177cc469dc05abaca5bb8606`.

The initially proposed NULL rewrite was rejected during review: the producer
emits one-byte LOAD_NULL, not the assumed five-byte LOAD_CONST. The final helper
instead authors REF(reference_placeholder), resolves exact owner/local/global
metadata, walks actual instruction widths and replaces one matching LOAD_REF_ADDR
operand with the typed local reference. Code ranges, branch targets and opcode
widths remain unchanged. The initializer selection requires the named frame
declaration's Explicit/Ordinary action; the other case selects its POU body.
Both mutated artifacts retain positive wire-admission assertions before preparation.
The class case preserves the earlier-local visibility boundary and C-to-A reference
compatibility; its additional persistent placeholder is included in the measured
baseline instance count. Expected values 124/1018 and local-instance cleanup
assertions remain unchanged. Source lifetime diagnostics are not relaxed.

The date identifier and configuration root corrections preserve their original
runtime assertions. The FB-static producer restriction now permits FB overrides
through the existing staged emitter for both lifecycle triggers, while retaining
the class restriction. This differs from legacy hosted source construction: spec
12 section 11.5.9 and the changelog explicitly identify the opt-in 2.0 capability,
leaving legacy 1.x acceptance and source reference-lifetime rules unchanged.
The existing override rollback assertions remain first=3/second=4 after failure.

No remaining source blocker was identified in this correction. No tests, builds,
formatters, validators or discovery were executed. This does not establish runtime
or hardware correctness; the next consolidated batch remains necessary.

# Expanded B-R1 independent source review

Reviewer `/root/b_independent_review`, canonical AGENTS/skills parity verified. This review covers compact/shared registry signatures and callers, portable error contexts, decoder/validator changes, lifecycle and retained keys, staging reuse, prepared groups and shared sorting, including scheduler work charging. Earlier review covers the unchanged B1 platform/firmware/tooling source; all dirty Rust identities, including inherited files, are pinned together in expanded-br1-reviewed-rust.json with available A4/B1/size-1 identities. No build, test, formatter, link or probe command was executed by the reviewer.

## Findings corrected during source review

1. The error.rs test module accidentally gated ToString on std while portable tests used it. The test import is now unconditional, with only the production import gated.
2. The hosted expected error consumed source_detail before a new exact-render assertion. The expected owned payload now clones it.
3. SectionDiagnostic equality initially rendered both diagnostics. RuntimeError equality is reachable in firmware deadline probes, so this would retain formatting/allocation even without a Display call. It now derives structural equality; explicit message queries/rendering remain separate. Added portable allocation tests assert construction, clone and equality do not allocate.

No further definite implementation blocker was found in the final inspected source. Final formatting, source freeze and authorized measurements/validation remain necessary.

## Behavior and representation

- All 89 registration names, fixed/variadic parameter sequences, bounds and implementation function paths were independently matched to A4's original registrations after expanding the new canonical signature references. No standard function was removed. Shared signature pointers and the borrowed StdFunctionRef returned by get avoid repeating complete parameter descriptors. Custom registration, replacement, conversion fallback and clone isolation remain. This is an explicit Rust API representation change (get returns a borrowed view value; parameter vectors use Cow), not a removal of runtime capability.
- Hosted std BytecodeError InvalidSection retains its SmolStr alias. Portable InvalidSection retains typed SectionDiagnostic; RuntimeError::from retains BytecodeCause without eager string construction. Hosted conversion continues to return its existing Bytecode { code, detail } shape. Display messages and stable error codes are pinned. PreparationDiagnostic retains numeric, name and UTF8 context and maps to the original VmBytecodeDecode category. Static strings use new_static. The successful const-child read no longer formats a length label.
- Portable equality is structural for structured error variants. Explicit textual assertions still render and compare exact messages; hosted textual payload equality is unchanged. The updated vm/mod.rs tests preserve hosted variant assertions and add exact text/codes plus portable UTF8/truncation fields. No assert merely became unconditional success.
- Error variants carry bounded scalars, static labels or the existing SmolStr representation. No recursive error ownership, formatting recursion or new Box allocation was introduced. Actual target enum/stack size remains a measurement question; source inspection does not establish SRAM headroom.

## Collections and ordering

LifecycleMarks uses 16 admitted declaration bits and 33 owner bits, preserving None and every u32 InstanceId including u32::MAX. The existing 65,536 construction-record cap is asserted; insert/lookup reject out-of-range declaration identities instead of truncating. Packed ordering matches the former tuple order. Root indices use checked conversion. Retained graph maps use equivalent u32 keys and preserve identity mapping and declaration order.

Initializer staging reuses the already-built visited set as the exact writable-instance set instead of collecting it again from backups. The loop produces backups for the same visited identities; failures discard the incomplete seed. Prepared Groups retain original ordinals for per-key wire order and separately sort physical declarations by slot. Section sorting uses offset plus ordinal, preserving the old stable order for equal offsets and zero-length sections. The standardized-section bitmap keeps the same 1.x/2.0 and extension-ID interpretation.

Shared heapsort retains existing validator operation/charge order. Final validator/preparation wrappers erase comparator type behind a mutable trait object while retaining generic record types, avoiding unsafe erasure. Call-site comparators are unchanged. Remaining ready-task sorting preserves priority, due time and unique task index order; the engine now charges its actual sorting work. Budget failure remains fail-closed, and native tests cover exhaustion before mutation.

## Remaining evidence boundaries

No fit, savings, passing regression or device execution is claimed. Current sources still require post-format reconciliation and the authorized comparisons. Preserve all functions, diagnostics context, overflow checks and complete device-side validation. The existing full-batch script review and standalone firmware graph limitation continue to apply. Any later source edits are outside this exact identity snapshot until reviewed.

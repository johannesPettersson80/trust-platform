# Independent B-R2 numeric, sort and trace review — correction pending

Canonical bootstrap independently copied and verified from /home/johannes/projects/trust-platform into /home/johannes/projects/trust-platform-portability-b, branch feat/runtime-portability-b, HEAD df427259cc387a7a79fb81132be23e48ea1493d4. Read AGENTS plus architecture, native-test, IEC and Ponytail guidance. Compared source to isolated /home/johannes/projects/trust-platform-portability-br2-m1. No validation commands executed.

## Blocking finding

The shared trace formatter writes fragments via Console::write_str. That method creates, times and flushes a fresh Record per fragment; the prior writeln! dispatched Console::write_fmt once for a whole record. Consequently the 512-byte and 100ms bounds no longer apply to the complete B1 record and each field/comma incurs an extra flush. Wire-byte equality alone does not preserve this transport contract. Root and author notified. Final acceptance waits for one aggregate console transaction and native aggregate-bound/error coverage.

## Clear source areas

- REAL-to-integer conversion retains nonfinite and wide i128 bounds before target dispatch, then exactly bounds IEC signed/unsigned destinations with half-open 2^63/2^64 ranges. This preserves overflow versus unsupported-type ordering, negative zero, endpoint rounding, and existing round/truncate selection. Text parsing retains the wider i128 parser and its distinct text-versus-value errors.
- Signed bitstring masks preserve low destination-width bits; signed character inputs reinterpreted as u64 still reject all negative values and retain positive limits.
- Assertion refactoring preserves arity/type/comparison order and exact original-operand messages for all six comparisons. Formatting occurs on the rejection path only.
- The shared sort loop changes only typed Result propagation into a type-erased Option driver. Callback sequence and first-error propagation remain identical. The native test compares every operation, equal-key permutation and partially mutated state at every failure point against the frozen prior driver.
- Registration tests replace machine-code address equality with matching call results, preserving metadata identity checks and adding successful mixed-case calls spanning every family and hosted explicit registration.
- Trace scalar formatting handles signed minimum via unsigned_abs, unsigned maximum within 20 decimal digits, lowercase fixed-width hexadecimal and CRLF. The call-site values and field order match the old formatter; the outstanding transaction issue above is separate.

No source edits made by the reviewer. Acceptance is source-only; compilation, tests, firmware linking, size and hardware execution remain unverified.

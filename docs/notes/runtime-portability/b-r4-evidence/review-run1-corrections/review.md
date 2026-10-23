# B-R4 run1 corrections independent source review

Accepted after the StableErrorCode argument correction. Reviewer `/root/br1_static_registry`; seven-file identity `17368e7a40975f4adce55cd69e5755a217fd2d65d3c389a55966ab1db6dcf03f`. No product source edits, formatter, builds, tests, links, hardware operations or validation reruns performed by the reviewer. All corrections remain unverified by execution.

The four call/fixture corrections were independently accepted in review-call-corrections: one shared local-input/presence copy helper preserving checks, charges and mutation order; an equivalent constant-bound clamp; reserved fixture identifier rename with unchanged assertions. These files match that accepted identity.

The firmware change replaces eight runner failure formatting sites and the outer main failure site with one concrete cold, non-inlined trace::failure(Console, phase, code). It calls writeln! once, preserving one Console::write_fmt transaction with the existing aggregate 512-byte/100ms limits and flush. Literal output remains B1,FAIL,<phase>,<code> followed by CRLF. All existing safe_off calls remain before the same record; ignored UART failures, returned error tags and stopped-idle behavior remain unchanged. No normal trace or timing boundary moves.

Found and resolved before validation: stable_code() returns the StableErrorCode enum while the new helper accepts &str. All eight runtime-error call sites now use stable_code().as_str(). The enum's Display delegates directly to formatter.write_str(self.as_str()), so this adaptation preserves exact output with no allocation. Main's runner error was already a string. This is an implementation correction, not a protocol change.

The helper consolidation targets the observed 320-byte headroom shortfall but claims no saving until another authorized link. No functionality, stack/heap reservation, flash margin, profile limit or acceptance assertion is removed. Run1 evidence remains bound to its earlier frozen source and does not validate these seven changed files.

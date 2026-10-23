# Run 8 independent reconciliation and correction review

Canonical agent files were copied and verified before this review. Branch remains
feat/runtime-portability-a4 at base 77b91381fcf1b1850611f88b397bbfe8d45e3523.
Reviewer authored no implementation and ran no tests, builds, formatters or validators.

All 48 raw artifacts match their hashes locally and on the builder. The artifact
index digest is `29a26e582aaf99b92856ccabc3bb51de1dc5b53f395cac6ae5a723b9b0f1ede5`.
The 598-record frozen manifest digest is
`f008f9f574853ebb3b0c9d8dfc65bf566676c636a4862e72fd38903adf0f362f`.
The post-batch archive matches the freeze, and builder Rust/deletion records still
match. The ledger contains 19 required passes, eight failures, two prerequisite
unrun steps and two passing advisories.

Core execution now provides real assertion evidence: all-features 313 passed and
two failed; portable 273 passed and two failed; i686 196 passed and one failed.
Failures are the stale stable-code count and program-root expected error below.
Compile-fail doctests produced their expected E0502/E0277/E0616 diagnostics and
passed; those diagnostics are not additional regressions. Both MCU core library
checks passed. Host tests and fixture generation remained blocked by the unused
host re-export. Clippy additionally reported five redundant slice borrows.
No firmware or hardware execution is established.

Four-file correction review:

1. Stable-code inventory now expects 83 entries, matching the 72 existing codes
   plus 11 new dedicated codes. Duplicate checks, exhaustive variant coverage and
   exact snake-case/Display identity assertions remain unchanged.
2. Program-root replacement test expects ProgramRootReplacement, as specified by
   the dedicated protection contract. Its assertion that the root remains unchanged
   is preserved. Genuine reference type incompatibility tests retain TypeMismatch.
3. Remove only the unused host sizeof_type_from_table re-export. Register execution
   calls the shared budgeted sizeof_type_from_table_with directly; the shared stack
   does likewise. Host sizeof_error_to_runtime and the public core helper remain.
4. Five register-tier test interpreter calls pass module.code() directly because
   that accessor already returns a borrowed byte slice. No decoder or assertion
   semantics change, and no Clippy suppression is introduced.

The correction manifest `run-8-corrections-independent-source.json` pins all four
files. The sizeof wrapper was unchanged from the Git base during run 8, so it was
not in the dirty-source manifest. It is present in the full post-batch source
archive; those archived before bytes independently match both the base and retained
builder source. All other before hashes come directly from the frozen manifest
and archived bytes.

All reported run-8 failures are addressed by these owning corrections. No remaining
blocker found by this focused source review. That does not certify downstream host
compilation or behavior; a further authorized consolidated batch must establish it.
No automatic rerun, commit or push was performed by this reviewer.

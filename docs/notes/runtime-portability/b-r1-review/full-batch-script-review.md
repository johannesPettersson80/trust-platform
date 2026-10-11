# Independent full B-R1 batch-script review

Read-only review; no script execution, build, test, formatter or device command. Scripts are pinned separately in full-batch-script-identities.json. They describe the consolidated batch, not the separately authorized early size measurement.

One finding was corrected before this record: the formatting list omitted the touched include fragment crates/trust-runtime/src/host/eval/expr/call/tests.rs. It is now listed alongside runtime/vm/call/tests/stdlib_binding.rs for both formatting and checks. The README was also corrected to include GPIO in allocation measurements.

The scripts retain distinct preparation/validation STARTED markers, reject repeated launches, resolve both locks offline with workspace-only update, format once, refresh current provenance and generate GPIO once, then freeze before suites. Main/numeric artifacts are not regenerated and are compared to HEAD. Shared Cargo commands use the target lease. The environment clears native CC/CXX and encoded flags; firmware link and Clippy explicitly unset global RUSTFLAGS so the standalone target linker scripts/map flags survive. The Windows warning gate does not inherit a native compiler override.

Independent validation steps continue after failures. Firmware map evidence is moved aside before linking and only a freshly produced map is reported; ELF inspection depends on successful linkage. Diagram checks depend on architecture and rendering. Packing depends on the one producer result. Final status counts required FAIL and UNRUN rows without promoting advisory metadata to a gate. Physical execution is a later root-owned dependency step, not implied by builder completion.

Source freeze identities cover modified/untracked source relative to the recorded HEAD; rule/skill hashes are retained separately. Root must review lock/format/provenance changes after preparation and before suites, as the README says. No automatic retry or filtered test loop is present. Hardware failure ledger and the dependent Rust trace verification must still be added by root when that phase is reached.

No further definite script blocker found at this inspection. Any subsequent changes for the separately authorized measurement or a map-driven implementation adjustment need their own source review before the consolidated freeze. This note does not authorize or report execution.

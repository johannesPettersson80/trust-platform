# B-R4 run7 independent software reconciliation

Accepted as scope software/ELF evidence by `/root/br1_static_registry`, not physical-board proof. No tests, compiler, formatter, validator, linker or board commands executed. Raw evidence identity c4bbe49acde9e1c168406d534dda863323fe03f71529ddb9081f54bb21d1d82c pins 120 files excluding the live board directory.

The41-step ledger contains39 required PASS and two advisory PASS, with no required failures/unrun steps. Source and measurement approval markers both equal frozen manifest e1213d088ec94eb0767cc4ad88c4f402218f06df91caaed29d0f670909aa3aff. All current worktree manifest records match at review time. Final ELF f11b73ae4b6138d253df2aeea386c77c4c5e88d6a493b94d581325699e5f7caf matches the inspected image, with18592 upper bytes free.

Recounted run7 results: core all-features401 across29 result groups, portable362 across29, i686282 across8, focused host integrations170 across20 binaries, native adapter/xtask91, firmware library2 and provenance helper4. No ignored or filtered tests and no failed assertions in these groups. Runtime vertical and restart/retain suites are included in the20 host binaries. Applicable MCU checks, feature graphs, lint, supply chain, metadata/provenance, architecture, diagrams, formatting and artifact checks pass in their own retained logs.

The hosted unit suite did not rerun in run7. Run4 retains3841 unit passes and542 integration passes across68 binaries; these are historical full-corpus results, not run7 results. Runs5 and6 retained source-bound measurements only: no validation-started or board-started marker exists in either evidence directory. Run6's unfavorable phase split was reverted before run7. The run7 core/host integration allocation covers the final shared implementation boundaries.

The linked-path analysis supports proceeding to the guarded device test but does not establish physical headroom, timing or GPIO/reset behavior. Board evidence is intentionally excluded while capture runs. ScopeB physical acceptance must use the final completed capture and verifier record, including any failed guards.

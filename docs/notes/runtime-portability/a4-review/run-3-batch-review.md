# A4 run-3 batch source review

Source inspection only; canonical AGENTS/skills parity passed by file comparison.
The four setup copies and retained evidence copies match. Reviewed aggregate
basename/NUL/file bytes/NUL SHA256:
`f64e2cc3d254d89833d09187f46626f61f662c13d5ac95454292c71900082f9b`;
runner SHA256 `4c9c238087fde47221e2b19dab834055a25eeaf8bc8db933acd6bf015749834d`.

The run-path migration, earlier required metadata-index preparation, captured
prerequisite status and reuse for advisory metadata preserve gate allocation.
No required gate or runtime assertion was removed. A failed index leaves tooling
required-UNRUN and metadata advisory-UNRUN while other gates continue.

Launch blocker: the combined mutation-tooling command exports GIT_INDEX_FILE to
both test modules. focused_mutation_runner_tests.py creates a temporary Git
repository at lines 261–270 and calls its _git subprocess helper at 329 without
clearing that inherited variable. Its git add/commit would use and mutate the A4
review index with a different object database. Keep the indexed environment only
on the mutation_program_contract_tests command, or isolate the temporary-repository
helper's environment. This must be corrected and source-reviewed before launch.

No scripts, syntax checks, tests, formatters, validators or discovery were executed.

## Corrected script disposition

The follow-up source read confirms the leak is removed. Focused mutation-tooling
runs independently under `env -u GIT_INDEX_FILE`; a separate required
mutation-contracts step receives the isolated index only after its prerequisite
succeeds. Both steps are in the planned ledger. Index failure skips only contract
validation and advisory metadata, leaving focused tooling and independent gates
reachable. No tests or assertions were removed. Retained setup copies match.

Corrected runner SHA256:
`befcce8b0b29f43bc662dd5e4521b921ebd8d7d05c7494490c4b7e595e8c7b1f`.
Corrected four-script aggregate:
`5c0acd41218dda0e2a777aebdf2eb8db2f31b74e0d065ce8c372e000d68b34e4`.
No remaining command-plan blocker was identified. This is source clearance only,
not execution evidence. No checks were run for this follow-up.

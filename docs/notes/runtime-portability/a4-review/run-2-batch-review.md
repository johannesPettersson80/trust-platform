# A4 run-2 command-plan and run-1 bookkeeping review

Source inspection only; no builds, tests, formatters, syntax checks, validators or
mutation discovery executed. This review excludes the author's provenance helper.

Compared with reviewed run-1 setup, the only script changes are run-2 output/setup
paths, PYTHONDONTWRITEBYTECODE=1, and the affected mutation_program_contract_tests
module added to the existing mutation-tooling step. Required runtime assertions,
prerequisite handling, formatter cadence and no-automatic-retry guard remain.
The retained run-2 setup copies match the ignored operational copies. All three
checked-in lease/path/removal helper files match their reviewed retained versions.

Run-1 README accurately reports 10 required passes, 12 failures, four unrun steps,
and advisory one pass/two failures. The saved logs confirm two helper tests and
16 mutation-tooling tests passed; core/runtime assertions did not run after
compilation failed. STARTED/FINISHED match 05:36:26–05:40:04 UTC on 10 October.
The formatted manifest SHA256 matches the README, and the README's own hash matches
its artifact index entry. These are prior-run evidence checks, not rerun outcomes.

The execution record's current top and checklist correctly leave A4 unverified.
Requested one wording correction: the unlabelled pre-run-1 handoff paragraphs still
say no A4 commands/validation have run; label them historical or use past tense.

Reviewed setup script SHA256 values:

- `run-on-builder.sh`: `656a5c5f574374c8d24aeee01965953b470982e5adeb419f67fadd5a95dec045`
- `format-once.sh`: `7cbcf1b0aa72c382a0e499d0a7d2d4dfe5a032d182d40a6ab62add122cd68f30`
- `freeze-source.py`: `1ea4d910a2740068641e25572c6bfb58d754761f6be641c0f461d36a10a098d6`
- `prepare-metadata-index.py`: `bcb5d82ade52a9164b18e5187bed99a74a4961858999ed2bf30ffa57a3f59236`

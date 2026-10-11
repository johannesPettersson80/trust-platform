# Publication evidence checkpoints

## Corrected source and fresh hardware: run 5

Tested source: `a8937a07c12042fa655297a53338fa787c1a978c`, version 0.24.73,
based on released main `3bed89b47b4e0ed8ecc11410b592228fff5f0112`.
PR #132's first complete 26-check attempt found the Windows native path assertion,
macOS Bash 3.2 transport failure and an automatic-review restart initialization
defect. The full first CI ledger is retained; its 75 MiB raw job logs remain in
the external artifact store with their official SHA pins and GitHub job links.
The three root causes were corrected together and independently source-reviewed.

The exact-SHA guard passed every required command: 8,690 native tests across
315 result blocks, 24 pre-existing ignored tests, 519 VS Code tests, Clippy,
cross warnings, both lock audits, architecture, MP parity and final cleanliness.
The planner/catalog findings remain advisory. All 20 required supplement commands
passed, as did its advisory metadata command and all twelve strict public-docs
steps. The new restart case passed in the full native, portable no-default and
selected i686 lib/five-suite lanes (363 no-default native tests; 271 selected
i686 tests). The two MCU checks are library compilation,
not C6 hardware or firmware execution. The tooling suite passed 104 tests.

The seven-step fresh-image suffix passed, retaining the actual new ELF/map and
checking its source bytes. Inspected ELF
`4761d2bc5153b3892da352ca1f0be1f7041f421457158bfc2a9682ddbbb39a61`
was installed and physically replayed with the previously reviewed protocol.
UART `1a15c7b9c615e6893699addc04988f836f6dbc84e7a0135eb1c64df5bd18171b`
passed the Rust verifier: 101 oracle samples, 40 activations/zero misses, numeric
exact/tolerance, GPIO safe outputs, depths 1–4 with interrupts and watchdog reset/DONE.
Upper flash free remains 18,720 bytes; heap peak is 28,272 / 73,728 bytes;
MSP peak is 13,596 / 16,384 bytes, leaving 2,788 bytes. No function, overflow
check, compiler or qualification floor was removed. Raw ELF/maps, flash backups,
OpenOCD logs and archives remain outside Git. Capture exit 124 is the planned
serial-window timeout; the separate Rust verifier determines acceptance.

[Run-5 retained evidence](run5/) pins the actual source, commands and reports.
The independent read-only [evidence review](run5/independent-evidence-review.md)
reconciles the raw counts, hashes, flash bytes and preservation comparisons. The documentation/evidence successor changes no
product or fixture bytes and still requires its own exact-SHA guard plus image
identity check before push. Native corrected-head Windows/macOS CI, guarded merge,
main CI, tag, Release assets and all Marketplace targets remain separate stages.
Printed package/PCB markings, manual button actuation and optical LED observation
remain unverified. Fixture timing is not WCET, and production/M3, durable
installation/retain and C6 physical qualification remain later gates.

# Earlier publication evidence checkpoint

Tested source: `ca205132e0f5626046353ffbe13fe402246da38f`, based on main
`3bed89b47b4e0ed8ecc11410b592228fff5f0112`, version 0.24.73. The exact-SHA
guard passed every required step. Its planner and catalog results are advisory,
retained as such. Native workspace: 8,689 passed in 315 result blocks, 24 ignored;
VS Code: 519 passed. MP-001 and final source cleanliness passed.

Run 1's isolated portable corpus passed 362 tests, its selected i686 lib/five-suite
lane passed 270, both MCU graphs and firmware checks passed, and the public-docs
ledger passed twelve steps. Those unchanged-source results are reused proof,
not newly executed counts. The final guard owns the current native/Clippy/cross/
supply-chain/architecture/VS Code proof. The historical failures remain retained.

The final map collector was corrected after catching an unledgered copy failure:
Cargo reused the fresh ELF without regenerating an ignored map deleted by sync.
The explicit seven-step final-image suffix forced a link through mtime only,
asserted unchanged source bytes, retained the actual map, inspected the ELF and
packed the current application. It passed every step. Raw maps, ELF images,
full flash backups and archives remain outside Git.

Fresh physical replay used the exact inspected 0.24.73 ELF
`0597a300375b7279ee203c18a573f86e0fdab56eb760e6de7787fa8aff990cce`.
UART `1a15c7b9c615e6893699addc04988f836f6dbc84e7a0135eb1c64df5bd18171b`
passed the Rust verifier: 101 oracle samples, 40 periodic activations and zero
missed intervals; numeric exact/tolerance, GPIO safe outputs, call depths 1–4
with interrupts and IWDG reset/DONE. Upper flash free: 18,720 bytes. Observed
heap peak: 28,272 / 73,728 bytes. Observed MSP peak: 13,596 / 16,384 bytes,
leaving 2,788 bytes. These are measured fixture results, not worst-case proof.

Independent root reconciliation reproduced image/application/UART digests,
checkpoint and both firmware-region preservation comparisons, memory margins,
activation counts and the complete reset/DONE footer. Capture exit 124 is the
planned bounded serial-window timeout; the Rust verifier determines acceptance.
Physical package/PCB markings, manual button transition and optical LED observation
remain unverified. Production bounded operation, durable installation/retain,
whole-profile timing and C6 hardware qualification are later gates.

That earlier evidence/docs closeout changed no product Rust, firmware, artifacts,
functions, compiler, flags or versions. Its successor exact-head guard passed at
`75b42a667` before the first PR push. The later corrected source and current
publication allocation are recorded above; old proof cannot certify a successor.

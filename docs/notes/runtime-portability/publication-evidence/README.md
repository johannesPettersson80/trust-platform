# Publication evidence checkpoint

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

This evidence/docs closeout changes no product Rust, firmware, artifacts, functions,
compiler, flags or versions. Its successor exact-head guard is still required
before push. A prior source artifact does not certify that later commit; the current
release artifact and GitHub checks supply that publication stage.

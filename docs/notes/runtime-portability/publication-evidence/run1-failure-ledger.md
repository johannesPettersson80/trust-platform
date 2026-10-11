# Run 1 publication failure ledger

Candidate: 5b7f2ea55bb7803e594ebd84983eb9e60285f898; source remained frozen.

One required failure: external unsafe AST gate reported 18 registered sites missing
(11 firmware sites, seven hosted allocation-instrumentation test sites). The sites
exist at their exact registered lines. Root cause is the external shell scanner's
inventory: it traverses crates/third_party but not firmware and excludes every test
file, whereas the Rust full-map policy includes the reviewed firmware/test sites.
Do not delete registrations or broaden unsafe delegations.

Guard stages after architecture safety remain unrun (Clippy, test-all, MP-001 and
final cleanliness). Passing earlier guard stages include exact source/bootstrap,
strict smoke, 519 VS Code tests, formatting, cross-target warnings and supply-chain
checks for both lockfiles. Planner and catalog findings remain advisory.

The separately reviewed supplemental ledger passed all 22 required steps and its
advisory metadata step, including portable native and selected actual i686 tests,
both MCU graphs, firmware checks/link/ELF inspection, MSRV, Rustdoc, tooling,
Windows LSP, eight one-attempt mesh/TLS iterations, formatting, drift, retained
tracked artifact integrity and clean source. All 12 strict-public-docs steps passed.
The final 0.24.73 ELF has 18,720 bytes upper flash free; hardware was not attempted
because the release guard failed. The ELF is retained as a source-specific result.

Correction: align external scanner inventory and classification in Rust with the
existing full-map production scope plus exact registered excluded test paths, keep
all raw AST/report evidence and fail conditions, and add native regression fixtures.
Independent review precedes corrected preparation/commit/new exact-SHA validation.

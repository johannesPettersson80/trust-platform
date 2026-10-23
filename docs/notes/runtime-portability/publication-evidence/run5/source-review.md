# Final prepared source reconciliation

Accepted: no actionable findings in the final 15-path prepared correction. The publication author may proceed with the authorized correction commit, clean exact-SHA synchronization and consolidated guard/supplement validation. This acceptance concerns source and command preparation; it does not certify compilation, regression execution, native Windows/macOS behavior, release readiness or hardware qualification.

## Identity and bootstrap

Canonical: /home/johannes/projects/trust-platform.
Reviewed destination: /home/johannes/projects/trust-platform-portability-release.
Branch: integrate/runtime-portability-stm32.
HEAD before correction commit: 75b42a6679348830c201f423d41e4751a849804b.
Canonical AGENTS.md, CLAUDE.md and complete .codex/skills parity were independently rechecked and match. Existing dirty checkouts were preserved.

final-prepared-source-identity.json and final-prepared-source-sha256.txt pin all 15 changed paths, including the untracked new core restart regression file that git diff alone omits. The 13-path earlier identity remains preserved separately.

## Reconciliation

The following paths remain byte-identical to the initial independent source review: CHANGELOG.md; engine/restart.rs; source_free_restart_graph.rs; the execution PUML; the portability checklist; specifications 12 and 34; and supply_chain_gate.sh.

engine/mod.rs retains the accepted startup/restart build argument and image-copy-before-construction ordering. Formatter relocation of the cfg(test) restart_tests module before storage_tests is the only preparation difference there.

xtask portability build.rs, supply_chain_tests.rs and the new engine/restart_tests.rs retain the reviewed behavior and assertions after formatting: native path-component joins and platform fixtures; both exception-list modes and all four independent shell failures through /bin/bash; fully admitted encoded initializer bodies/results and both-mode flat/hierarchical direct-image restart assertions. No production behavior, ownership, wire admission, limits or native assertion changed during preparation.

The generated execution SVG contains the three reviewed warm/cold staging, ordered-target precedence and transactional-failure statements. The manifest changes only this diagram's source entry to PUML SHA-256 9153a84253abc48caef97cdf36b54e30feb745a3edf1fd86e1fec78fae892480, which matches the reviewed source. This is generated-content source inspection, not a rendered visual acceptance claim.

The integration note now records successful preparation, pending exact-SHA software and fresh physical proof, the corrected retained mesh/TLS allocation and fresh linked-image requirement. It keeps historical CI/guard/hardware stages separate from the current correction. The reviewed command scripts remain those accepted in command-script-review.md.

## Retained preparation evidence

The reviewer read the external run-5 preparation ledger and raw environment/provenance/architecture/diagram records. All seven ledger rows have exit 0: environment, formatter, fragment-formatter, mutation-selector-refresh, architecture, diagrams and source-preparation-diff. The environment identifies scena-rust-builder and the expected remote checkout/HEAD with stable Rust 1.99.0. No extra provenance-record source drift appears in the completed diff.

No tests, builds, formatter, gate or hardware actions were launched by this reviewer. New behavior assertions remain unrun until the authorized consolidated validation. The actual guard, required supplements, strict docs and fresh retained ELF inspection must all pass before hardware; exact source/image identity and qualification floors still apply.

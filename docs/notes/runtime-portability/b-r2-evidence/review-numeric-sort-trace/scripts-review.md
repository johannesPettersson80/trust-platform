# B-R2 scripts and installation guidance — source acceptance

Accepted for the authorized preparation, exact-manifest review, M2 measurement and one final software/conditional hardware batch. No commands from these scripts were executed, and no formatter, shell syntax checker, build, test, validator or hardware tool was launched by this review.

Preparation is single-use, formats once, preserves both lockfiles/artifacts, refreshes existing metadata and freezes modified/untracked/deleted paths. M2 checks exact manifest approval before consuming its once-only marker, retains failed-link maps and records missing maps or failed attribution as failures. Final software batch includes the new native tests through owning suites, i686 and MCU checks, both audit graphs, lint, architecture before diagrams, provenance, format and immutable artifact checks. Runtime vertical appears once in the hosted integration command. Independent failures continue; dependent rendering/inspection remain unrun. No firmware relink occurs in that software batch. The coordinator retains responsibility for source freeze, marking software approval only after all required passes, identity-preserving artifact transfer and native parsing of the final UART trace.

Two review findings were corrected before acceptance:

1. M2 previously accepted any nonempty review marker and could return success after failed map attribution. It now compares the approved manifest digest and fails on absent map/failed attribution.
2. Automatic `flash write_image erase` for a discontiguous ELF may erase reserved sector holes (documented by the official OpenOCD manual: https://openocd.org/doc/html/Flash-Commands.html). The board script now explicitly erases sector 0 and sectors 4 through 7, writes the firmware ELF without automatic erase, then separately erases/writes application sector 1. Sectors 2 and 3 are omitted from erase commands. Firmware and checkpoint byte-preservation comparisons remain. Updated firmware README warns against automatic gap erasure.

Board script retains original flash backup, option/reset observations, verified writes, UART stale-byte separation, capture before run reset, timeout interpreted solely as bounded acquisition, and no automatic retry. A failed command prevents launch of dependent hardware steps. This source review is not evidence of physical sector preservation, valid UART behavior or successful firmware fit.

Script identity includes all nine reviewed executable sources plus the updated firmware README. Earlier pending notes are historical findings; this acceptance supersedes their open status.

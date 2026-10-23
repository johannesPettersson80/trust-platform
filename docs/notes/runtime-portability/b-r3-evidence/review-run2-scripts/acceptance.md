# Authorized B-R3 correction batch script review

Reviewer: `/root/br1_static_registry`. Verdict: accepted for the owner's newly authorized test/remeasure/conditional-hardware batch. Compared all setup-run2 files with accepted setup-m2; baseline.map exactly matches retained M2 measurement/firmware.map.

Changes are run2 paths and one-use batch marker, plus one required firmware-link step after native suites and before inspection. That step invokes measure.sh once; the child retains its own one-use measurement marker and exact frozen-review digest check. No retry loop or duplicate link added. Remaining suites, features, allocation profile, leases, six-job limit, disabled sccache, locked inputs and frozen-review gating are unchanged. Independent checks continue after a failure; diagram work remains conditional on architecture and board work requires every required software row to pass.

The unchanged board script requires an approved software marker and inspected ELF, saves existing flash before installation, writes only selected firmware/application sectors, verifies preserved checkpoint sectors, and captures the pinned board serial topology. The coordinator remains responsible for copying the exact accepted ELF and software evidence to the board directory. This source review does not claim a hardware run.

No scripts, builds, tests, formatters, validators, links or board commands executed by this reviewer. No repository edits. `source-identity.json` SHA-256: `a471934883b25c88a80d3dfa8e1bb8f431650c8173beac548d18e16358d137c0`.

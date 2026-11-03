# What and why

<!-- One lane of work, a few hours: what changes for the user and why. Name the checklist item. -->

# Specification

<!-- The sections of docs/specs this follows or changes. A missing or wrong section is fixed in this pull request. A section that it follows without changing it is also named in a commit trailer, such as `Specification: 35 B3.4`, which the Verification Gate checks (specification 30). -->

# Tests

<!-- The native tests that prove it, each with the wrong behaviour it catches. -->

# Definition of done

- [ ] The specification describes the behaviour, and this pull request changed it where it was missing or wrong.
- [ ] Native tests prove it; every expected value is worked out from the specification.
- [ ] No logic added or grown in TypeScript.
- [ ] A user-visible change has a changelog fragment under `changelog.d/` (`changelog.d/README.md`).
- [ ] Docs, READMEs and screenshots describe the new behaviour.
- [ ] The compile check and the focused tests passed before the push; CI runs the full suites.
- [ ] Codex's findings are fixed or answered in this pull request.

# Real-time paths (T0 only)

- [ ] No heap allocation, blocking, lock waiting or syscalls in a T0 hot path.
- [ ] No key-string parsing or route decisions in the T0 publish/read hot path.

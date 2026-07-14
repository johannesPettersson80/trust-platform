# Runtime control authorization product fix

Date: 2026-07-14

## Missing contract and tests

`SPEC_GAP_DEBUG_AUTHORIZATION_001` and
`SPEC_GAP_CONTROL_AUTHORIZATION_MATRIX_001` recorded that debug mutation and
runtime control role boundaries lacked a complete written contract. The
contract is now written in `docs/specs/11-runtime-engine.md` and
`docs/specs/13-debug-adapter.md`. The runtime-control authorization policy is a
truST product security extension, recorded as DEV-050 rather than an IEC
conformance deviation.

Two focused Rust tests were added:

- `debug_activation_and_mutation_follow_the_reviewed_role_boundary` exercises
  denied and allowed debug activation, force, and release paths through the
  control dispatcher;
- `security_sensitive_debug_activation_and_unclassified_requests_fail_safe`
  pins the policy boundary for debug activation and future unclassified
  operations.

The live control dispatcher contains 78 operation strings, and the reviewed
policy classifies the same 78 strings. The fallback remains independently
fail-safe if a future handler is added before its explicit classification.

## Red result

Clean commit `40eee13dd803a49dbf90ee66c1b8731defbdb24a` was run on
`trust-builder` with the shared validation target. Both new tests exposed the
same authorization defect:

```text
cargo test -p trust-runtime --lib \
  control::tests::debug_activation_and_mutation_follow_the_reviewed_role_boundary \
  -- --exact
# FAILED: Engineer must not activate debug

cargo test -p trust-runtime --lib \
  control::policy::tests::security_sensitive_debug_activation_and_unclassified_requests_fail_safe \
  -- --exact
# FAILED: left Engineer, right Admin
```

`control.debug_enabled` was classified as Engineer even though it activates a
security-sensitive surface, and an operation omitted from the policy inherited
Viewer authority.

## Green result

At clean commit `62b9b2a671581e0529c9a44fff3991ffbf6557e3`,
`control.debug_enabled` requires Admin and the unmatched-operation fallback
requires Admin. The dispatcher still rejects unsupported request types before
any handler action.

```text
cargo test -p trust-runtime --lib \
  control::tests::debug_activation_and_mutation_follow_the_reviewed_role_boundary \
  -- --exact
# 1 passed

cargo test -p trust-runtime --lib control::policy::tests
# 5 passed

cargo test -p trust-runtime --test debug_control \
  rbac_authorization_matrix_enforces_sensitive_endpoint_roles -- --exact
# 1 passed
```

## Honest posture

This row is `proof_kind = "none"`. These Rust tests do not emit same-run case
artifacts, so neither invariant is promoted beyond S0 and neither gap is
closed. Both gaps advance only to `test_mapped`; producer-authentic red/green
proof and the consolidated batch broad gate remain explicit debt.

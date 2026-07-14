# Runtime Non-Finite Ingress Gap Closeout

Date: 2026-07-14
Source checkpoint: `e62457f0`
Proof posture: focused specification closeout and G2 promotion, not complete
floating-point adequacy

## Written Contract

`docs/specs/11-runtime-engine.md#floating-point-boundary-admission-policy`
defines fail-closed admission for the shipped typed `%I`, Modbus, MQTT, OPC UA,
ADS, mesh, HMI, managed-debug, retain, simulation-threshold, and typed `%Q`
boundaries. It specifies declared-width validation, whole-transaction rejection
where a batch or snapshot is involved, preservation of the last accepted PLC
state, and no clamping or default substitution.

The contract cites IEC 61131-3 Ed.3 Section 6.4.2.1, Table 10 and Section
6.6.2.5.15, Table 39 for the REAL/LREAL representation and `IS_VALID`
background. DEV-045 records that external host/protocol admission is a truST
policy rather than an IEC-defined transport rule.

## Product Defect And Targeted Proof

The hand-authored case
`RT_SAFE_NAN_001_MQTT_MAPPED_BATCH_REJECTION_AND_RECOVERY_ISOLATION` found a
real MQTT state leak: a rejected batch could retain an earlier accepted point
and expose it after a later valid point recovered. `prove.py v1` recorded the
failing case at clean commit `ff5d737e`, the product fix at `5179fe05` stages a
mapped batch transactionally, and paired green evidence at clean descendant
`0b443d15` binds the same case file and proof contract.

## Boundary Test Set

The exact closeout denominator is the 22 `RT_SAFE_NAN_001` catalog rows listed
in `SPEC_GAP_RUNTIME_NONFINITE_INGRESS_001.affected_tests`. They cover typed
input/output, debug REAL/LREAL writes and forces, simulation thresholds,
Modbus, MQTT, HMI, OPC UA, ADS, mesh, and retain save/load. Each row is mapped
to the written oracle and has an exact runnable command.

The final clean trust-builder gate at commit `30aee660` ran `just fmt`,
`just clippy`, and `just test-all`, then executed the bound MQTT case again with
a fresh run stamp. Evidence `EVID_BROAD_REMOTE_PR_20260714_2E2FE51F4D40`
records the successful broad gate and case artifact. This advances the
invariant to G2; it does not create release proof.

## Honest Residual Debt

The focused behavior gap is closed because the named shipped boundaries now
have a written policy and mapped tests. `RT_SAFE_NAN_001` keeps
`boundary_value_matrix_depth` missing and its `wrong_type_or_shape` cell open:
the program has not established an exhaustive cross-product of width,
subnormal, signed-zero, aggregate-shape, attach-mode, and future boundary
cases. No validated, R1, IEC-conformance, or universal floating-point claim is
made.

During the final broad run, the shared Cargo target exposed a separate test
infrastructure defect: stamped case artifacts inherited the compile-time path
of the first worktree using the cache. A regression test reproduced that
failure, and commits `bb10ad50` and `30aee660` make the fresh run-stamped
artifact directory authoritative without changing product behavior.

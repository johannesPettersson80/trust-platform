# A4 run-3 fixture and run-4 batch source review

No actionable finding in the seven paths pinned by the [manifest](run-3-fixture-and-batch-review.json).

- Profile admission: the Retain case rebinds an actual physical ProgramRoot key,
  including matching path references. The declaration/ref/root indexes and demand
  remain unchanged. Declaration bindings and construction roots admit Retain,
  while the profile still rejects it. The unused I/O reference does not acquire
  storage coverage obligations. Both cases keep positive wire validation before
  profile rejection. This replaces the invalid External coverage assumption.
- Assignment: Disabled/Enabled avoid reserved tokens while retaining enum identity
  assertions. DivisorType's default recipe establishes one during the declaration
  default pass before Payload member recipes; the former explicit initializer ran
  too late. Matching VAR_EXTERNAL type and new before/after assertions preserve
  the central claim that assignment at Divisor zero does not replay defaults.
- Restart graphs: writes now occur inside the owning FB body; Holder invokes its
  private child. Seed inputs establish 42, with explicit pre-restart value checks.
  Retained alias/self-reference, nonretained reset 4/9, interface remapping and
  cold-reset assertions remain. No public output-write restriction was relaxed.
- Run-4 scripts differ from reviewed run-3 only in run identifiers/paths. All four
  retained copies match their setup originals byte-for-byte. Separate unindexed
  focused tooling and indexed mutation contracts, isolated-index failure accounting,
  freeze/format/fixture prerequisites and the existing gate list are preserved.

Only source, saved run logs and script diffs were inspected. No tests, formatter,
validator, script execution, mutation discovery or batch was launched. The
reviewer's compiler-free test/producer correction is excluded and requires the
other agent's independent review. Existing failed runs remain historical evidence.

Aggregate SHA256: `d9065a474f90209d6f3054ebc97070dc10e7ab7b5069fb78a54faeb90c28b7dc`.

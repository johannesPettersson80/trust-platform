# Scope B run 1 evidence

**Failed, not board-qualified.** The ledger has 18 passes and four failures in 22 steps.
Firmware linking failed; there is no installed test image, captured board trace or
on-device heap/stack/timing result.

Text outputs here retain their raw bytes. `raw-artifact-sha256.json` indexes the full
external raw directory, not a claim that every indexed binary/snapshot is tracked here.
Full raw cache: `/home/johannes/projects/.artifacts/runtime-portability-b/run-1/raw/`;
failed map: its parent `failed-firmware.map`. Frozen changed sources are in the sibling
`frozen-source/` directory; the unchanged base is committed A4 `df427259cc`.
No source tarball, raw full map, application binary or generated firmware is committed.

The command scripts are retained as text for review, not automatic retry launchers.
The board script did not run because its firmware-link prerequisite failed.
Standalone-only dependencies still require their own future supply-chain check.

See [execution and blocker record](../b-execution.md) and [independent source review](../b-review/independent-source-review.md).

# Editor rename-conflict specification closeout

Date: 2026-07-14

Focused source: `6a872be0ceaf505cdd0fa8eade9f5834015ca825`

## Missing contract and tests

`SPEC_GAP_EDITOR_RENAME_CONFLICT_001` lacked a written partition for local,
field, imported, project-wide, and cross-file collision checks. The contract is
now written in `docs/specs/14-lsp.md` under "Rename Conflict Safety" and binds
IEC 61131-3 Ed.3 section 6.1.2 case-insensitive identifiers to the existing
atomic rename implementation.

Eight focused tests cover:

- declaring-scope shadow capture;
- imported-use checks against the origin scope;
- case-insensitive same-scope collisions;
- a valid case-only change of the same symbol;
- case-insensitive structure-field collisions;
- project-wide top-level POU collisions;
- cross-file reference capture; and
- the named LSP refusal response.

Five of those tests were added by this vertical; three existing tests were
given reviewed catalog identities.

## Focused result

The complete rename-focused integration filter passed against the unchanged
product implementation on `trust-builder`:

```text
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-ide --test ide_features rename_ -- --nocapture
# 14 passed; 0 failed; 38 filtered out
```

The new tests did not reproduce a product defect. They demonstrated that the
existing scratch-database and merged-symbol checks already implement the now
written partitions. This closeout therefore changes specification, tests, and
verification metadata only; `crates/trust-ide/src/rename.rs` is unchanged.

## Honest posture

The specification gap closes because the boundary is written and every
reviewed partition has a focused test. `EDIT_RENAME_001` and
`EDIT_RENAME_002` remain `S0/gap_open`: ordinary Rust tests are not
producer-authentic red/green proof, and the next broad gate is deferred until a
batch contains a product change.

## Boundaries

- No runtime, IDE implementation, LSP implementation, validator, schema,
  suite, workflow, or approved proof producer changed.
- No product bug or behavioral fix is claimed.
- No invariant was promoted above S0 and no public claim was created.

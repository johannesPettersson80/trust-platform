# Reusable Structured Text Libraries

Reusable truST Structured Text packages belong under `libraries/` in this repo.

Package shape:

```text
libraries/
  my_library/
    trust-lsp.toml
    src/
```

Use `[dependencies]` in a consuming project's `trust-lsp.toml` to reference a
package under `libraries/`.

Use `[[libraries]]` for external/index-only stub packs or vendor documentation
attachments, not for normal reusable truST library packages.

`crates/.../tests/fixtures/` is test-only infrastructure and must not be used as
a user library location.

Every package has a manual under `docs/guides/`, published under Develop › Libraries:

- `libraries/control/`: `docs/guides/CONTROL_LIBRARY_GUIDE.md`
- `libraries/plcopen_motion/`: `docs/guides/PLCOPEN_MOTION_LIBRARY_GUIDE.md` and
  `docs/guides/PLCOPEN_MOTION_OOP_LIBRARY_GUIDE.md`
- `libraries/oscat/`: `docs/guides/OSCAT_LIBRARY_GUIDE.md` and `docs/guides/OSCAT_OOP_LIBRARY_GUIDE.md`
- `libraries/alarm/` (specification 40): `docs/guides/ALARM_LIBRARY_GUIDE.md`
- `libraries/comm/` (specification 42; it depends on `libraries/alarm/`):
  `docs/guides/COMM_LIBRARY_GUIDE.md`
- `libraries/plant/` (specification `trust-plant-library`; it depends on `libraries/alarm/` and
  `libraries/control/`): `docs/guides/PLANT_LIBRARY_GUIDE.md`, examples in
  `libraries/plant/examples/`

A new package ships its manual the same way, with a test that compiles and runs every example of
the manual (for example `crates/trust-runtime/tests/alarm_guide.rs`).

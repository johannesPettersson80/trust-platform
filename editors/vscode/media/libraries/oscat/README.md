# OSCAT For truST

This package is the manual-aligned OSCAT library for truST.

Source layout mirrors the OSCAT manual chapters under `src/`, and the runtime
conformance fixtures mirror the same chapter structure under
`crates/trust-runtime/tests/fixtures/oscat/*/src/`. New maintenance work
should keep tests in the matching chapter folder and land implementation in that
same chapter directory.

Shipped scope:

- manual chapter folders `03_data_types` through `26_list_processing`
- OOP companion package under `libraries/oscat/oop`
- matching runtime chapter fixtures under
  `crates/trust-runtime/tests/fixtures/oscat/core/src/<chapter>/tests.st`
- full shipped core fixture currently validated as `126` tests / `0` failures /
  `0` errors via `trust-runtime test --project crates/trust-runtime/tests/fixtures/oscat/core --timeout 120 --ci`
- ST-only negative public-surface sentinel under
  `crates/trust-runtime/tests/fixtures/oscat/negative_public_surface`, which
  now exists only to pin the deliberate upstream `OVERRIDE` name deviation
- the full manual Chapter 23 control-module surface, including the
  OSCAT_BUILDING environmental/control blocks that the manual places in that
  chapter
- shared OSCAT-style `MATH`, `PHYS`, and `LANGUAGE` carriers loaded by
  `OSCAT_BASIC_Constants()`
- strict IEC-compatible `CALENDAR` local field names `LOCAL_DT`, `LOCAL_DATE`,
  and `LOCAL_TOD` in place of upstream `LDT`, `LDATE`, and `LTOD`

User-facing reference material lives in:

- `docs/guides/OSCAT_LIBRARY_GUIDE.md`
- `docs/guides/OSCAT_OOP_LIBRARY_GUIDE.md`
- `examples/oscat_smoke/README.md`
- `examples/OSCAT/README.md`

Upstream reference source for this package lives in:

- `docs/internal/references/OSCAT/OSCAT_BASIC/upstream/oscat_basic_333.txt`
- `docs/internal/references/OSCAT/OSCAT_BASIC/manuals/oscat_basic333_en.pdf`
- `docs/internal/references/OSCAT/OSCAT_BASIC/license/oscat_license_agreement.html`
- `docs/internal/references/OSCAT/OSCAT_BUILDING/upstream/oscat_building_100.txt`

Conformance consumers live under:

- `crates/trust-runtime/tests/fixtures/oscat/core`
- `crates/trust-runtime/tests/fixtures/oscat/oop_core`
- `crates/trust-runtime/tests/fixtures/oscat/negative_public_surface`

Current public-surface parity snapshot against upstream OSCAT BASIC:

- upstream unique public symbols: `536`
- shipped local symbols: `563`
- remaining upstream names intentionally not exposed unchanged: `1`
- deliberate rename: upstream `OVERRIDE` ships as `OVERRIDE_3`

IEC compatibility note:

- The shipped `CALENDAR` port uses `LOCAL_DT`, `LOCAL_DATE`, and `LOCAL_TOD`
  instead of upstream `LDT`, `LDATE`, and `LTOD`, because those upstream names
  collide with reserved IEC date/time keywords and truST keeps that rule
  strict.
- The upstream Chapter 19 helper `OVERRIDE` ships as `OVERRIDE_3`, because
  `OVERRIDE` is a reserved truST keyword and the parser is intentionally not
  relaxed for OSCAT compatibility.

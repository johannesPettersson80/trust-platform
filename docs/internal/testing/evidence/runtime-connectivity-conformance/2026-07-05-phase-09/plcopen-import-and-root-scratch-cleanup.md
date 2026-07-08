# Phase 9 PLCopen Import And Root Scratch Cleanup Evidence

Date: 2026-07-05

Validation copy:
`trust-builder:/home/johannes/projects/trust-platform-rtconn-validation`

## Implementation

PLCopen import now rejects non-ST POU bodies before source reconstruction:

- `crates/trust-plcopen/src/plcopen/xml_common.rs`
  - Removed the fallback that extracted all text under `<body>`.
  - Added non-ST body classification for FBD, LD, SFC, and unknown body tags.
- `crates/trust-plcopen/src/plcopen/import.rs`
  - Skips POUs with non-ST bodies before `synthesize_import_pou_source`.
  - Emits error-level migration diagnostics instead of generating scraped ST.
- `crates/trust-plcopen/src/plcopen/tests/roundtrip_import.rs`
  - Adds regression coverage for FBD, LD, SFC, and unknown body payloads.
- `crates/trust-plcopen/tests/fixtures/plcopen/non_st_bodies.xml`
  - IP-clean synthetic fixture proving graphical/unknown body rejection.

Named diagnostics:

- `PLCO215`: unsupported FBD graphical body.
- `PLCO216`: unsupported LD graphical body.
- `PLCO217`: unsupported SFC graphical body.
- `PLCO218`: unknown non-ST body element.

Docs/profile updates:

- `crates/trust-plcopen/src/plcopen/profile.rs`
- `docs/guides/PLCOPEN_INTEROP_COMPATIBILITY.md`
- `docs/guides/OPENPLC_INTEROP_V1.md`
- `docs/internal/community-request-roadmap.md`

Fixture provenance cleanup:

- Existing tiny vendor-family fixtures were renamed with a `synthetic-` prefix:
  - `synthetic-codesys.xml`
  - `synthetic-twincat.xml`
  - `synthetic-siemens.xml`
  - `synthetic-rockwell.xml`
  - `synthetic-schneider.xml`
  - `synthetic-openplc.xml`
- `crates/trust-runtime/tests/fixtures/plcopen/README.md` records that these
  are hand-authored synthetic fixtures, not real vendor exports.

Root scratch cleanup:

- Deleted tracked repo-root `src/main.st`.
- Deleted tracked repo-root `src/config.st`.
- Confirmed `.gitignore` already ignores `/program.stbc`, `/runtime.toml`, and
  `/io.toml`; ignored local runtime artifacts were left untouched.
- Updated `docs/diagrams/architecture/full-software-map-generated.puml` so the
  source-derived tracked root PLC surface is `none`.
- Added `ARCH-RTCONN-02` to
  `docs/internal/testing/checklists/architecture-improvements.md`.

## Real Vendor Export Corpus Status

`RTCONN-P9-004` is not complete in this environment.

Findings:

- No repo-local real CODESYS or TwinCAT export XML fixtures were found.
- No `codesys`, `TcXaeShell`, `devenv`, or comparable vendor export tool was
  available on `PATH`.
- `/home/johannes/Downloads/Globals-test.project` exists, but `file` reports it
  as binary `data`, not an exported PLCopen XML file. It was not imported into
  the repo and does not satisfy the real-export fixture requirement.

Decision:

- Do not fabricate "real" CODESYS/TwinCAT export fixtures.
- Keep the synthetic corpus explicitly labeled.
- Add real CODESYS/TwinCAT export fixtures only after they are generated from
  scratch demo projects in a vendor-tool environment, with provenance recorded
  beside the fixtures.

2026-07-05 recheck:

- `command -v codesys`, `command -v CODESYS`, `command -v TcXaeShell`,
  `command -v devenv`, and `command -v XAE` found no vendor export tool on
  `PATH`.
- `find /home/johannes/Downloads /home/johannes/projects ...` found only:
  - `/home/johannes/Downloads/Globals-test.project`, still binary `data`.
  - Current `synthetic-codesys.xml` and `synthetic-twincat.xml` fixtures.
  - Older sibling-checkout copies of the pre-rename `codesys.xml` and
    `twincat.xml` fixtures.
  - Existing `examples/plcopen_xml_st_complete/interop/codesys-small.xml`.
- The sibling-checkout `codesys.xml`/`twincat.xml` files are 934 and 867 bytes
  respectively with the same tiny vendor-labeled fixture shape as the renamed
  synthetic fixtures. They are not acceptable real vendor export corpus inputs.

2026-07-05 deeper artifact inspection:

- `/home/johannes/Downloads/Globals-test.project`:
  - `file`: `data`
  - size: 234000 bytes
  - signature scan found no ZIP, SQLite, XML, OLE, 7z, or RAR header.
  - `7z l` and `unzip -l` both failed to open it as an archive.
  - byte/string scan found no `PLCopen`, `CODESYS`, `TwinCAT`, `<project`,
    `<pou`, `VAR_GLOBAL`, `PROGRAM`, `FUNCTION_BLOCK`, or `globalVars`
    payload.
  - conclusion: not usable as a PLCopen XML fixture or safe extraction source
    without vendor tooling.
- `/home/johannes/Downloads/OopMotionControlLibrary.library`:
  - `file`: `data`
  - `7z l` failed to open it as an archive.
  - conclusion: a CODESYS library artifact is not a scratch demo project
    PLCopen export and does not satisfy `RTCONN-P9-004`.
- Local tool/runtime checks:
  - no `wine`, `wine64`, `flatpak`, `codesys`, `CODESYS`, `TcXaeShell`,
    `devenv`, or `XAE` command is available.
  - `snap list` contains no CODESYS, TwinCAT, or Beckhoff vendor tool.
  - broader search found NuGet Beckhoff ADS libraries and Rust crates, but no
    TwinCAT XAE/export tooling.

2026-07-05 remote-builder recheck:

- `trust-builder` has no `codesys`, `CODESYS`, `TcXaeShell`, `devenv`, `XAE`,
  `wine`, or `wine64` command on `PATH`.
- `find "$HOME/Downloads" "$HOME/projects" ...` on `trust-builder` found many
  old validation-worktree copies of `crates/trust-runtime/tests/fixtures/plcopen/codesys.xml`
  and `twincat.xml`, plus `examples/plcopen_xml_st_complete/interop/codesys-small.xml`.
  These are the same tiny pre-rename/synthetic fixture family already rejected
  by this phase, not real generated vendor exports.

Required external input to close `RTCONN-P9-004`:

- A CODESYS PLCopen XML export generated from a scratch demo project owned by
  this repository.
- A TwinCAT PLCopen XML export generated from a scratch demo project owned by
  this repository.
- Provenance beside each fixture documenting tool name/version, project source
  files, export command/UI path, export date, and confirmation that no
  third-party project content was copied.

## Root Reference Check

Command:

```bash
find src -maxdepth 2 -type f -printf '%p %s bytes\n' 2>/dev/null
rg -n "src/main\\.st|src/config\\.st" README.md docs examples crates editors scripts \
  --glob '!**/out/**' \
  --glob '!**/node_modules/**' \
  --glob '!docs/internal/testing/evidence/**' \
  --glob '!editors/vscode/media/**'
```

Result:

- Before deletion, only `src/main.st` and `src/config.st` were tracked root
  scratch files.
- Remaining references are project-template, example, generated-doc, test-temp,
  or generic project-layout references, not dependencies on the repo-root
  tracked files.
- The generated full-software map was the only source-derived root-surface
  claim and was updated.

## Local Proof

Commands:

```bash
cargo fmt --all -- --check
cargo test -p trust-plcopen
cargo test -p trust-runtime --test plcopen_command --test plcopen_migration --test plcopen_st_complete_parity
git diff --check
```

Results:

- `trust-plcopen`: 14 passed, including
  `import_rejects_non_st_bodies_with_named_diagnostics`.
- `plcopen_command`: 9 passed.
- `plcopen_migration`: 7 passed.
- `plcopen_st_complete_parity`: 3 passed.
- Formatting and whitespace checks passed.

## Remote Focused Proof

Disk preflight before focused remote gates:

```text
/home/johannes: 112G free
/tmp: 6.7G free
```

Commands:

```bash
export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation"
cargo test -p trust-plcopen
cargo test -p trust-runtime --test plcopen_command --test plcopen_migration --test plcopen_st_complete_parity --test plcopen_codesys_import_runtime
```

Results:

- `trust-plcopen`: 14 passed.
- `plcopen_codesys_import_runtime`: 6 passed.
- `plcopen_command`: 9 passed.
- `plcopen_migration`: 7 passed.
- `plcopen_st_complete_parity`: 3 passed.

## Remote Diagram And Common Gate

Disk preflight before diagram/common gates:

```text
/home/johannes: 105G free
/tmp: 6.7G free
```

Commands:

```bash
scripts/render_diagrams.sh
python scripts/check_diagram_drift.py
just fmt
just clippy
just test
```

Results:

- Diagram render and drift check passed.
- `just fmt`: pass.
- `just clippy`: pass.
- `just test`: pass. `cargo-nextest` was absent, so the repo fallback ran
  `cargo test -p trust-runtime --lib` with 838 passed and 16 ignored.

Rendered diagram artifacts were copied back from the isolated builder copy:

- `docs/diagrams/generated/`
- `docs/diagrams/manifest.json`

## Builder Cleanup

After the remote Phase 9 gates, generated validation caches were removed again:

```bash
rm -rf "$HOME/.cache/codex-targets/trust-platform-rtconn-validation" "$HOME/.cache/sccache"
```

Final remote disk state:

```text
/home/johannes: 112G free
/tmp: 6.7G free
~/.cache/codex-targets: 4.0K
~/.cache: 1.2G
```

# PLCopen Fixture Provenance

This directory contains regression fixtures for PLCopen import/export behavior.

## Synthetic Vendor-Family Fixtures

These files are hand-authored, IP-clean synthetic fixtures. They exercise
ecosystem detection and migration diagnostics, but they are not exports from
CODESYS, TwinCAT, Siemens, Rockwell, Schneider, or OpenPLC tooling:

- `synthetic-codesys.xml`
- `synthetic-twincat.xml`
- `synthetic-siemens.xml`
- `synthetic-rockwell.xml`
- `synthetic-schneider.xml`
- `synthetic-openplc.xml`

The `codesys_st_complete/` fixtures are also synthetic ST-complete parity cases.
They cover deterministic import/export shape, not real vendor XML diversity.

## Real Export Corpus Requirement

Real CODESYS and TwinCAT export fixtures must be generated from scratch demo
projects owned by this repository, with provenance recorded beside the fixture.
Do not scrape or vendor third-party project exports into this corpus.

No real CODESYS or TwinCAT export fixture is present in this checkout as of
2026-07-05.

# CODESYS And TwinCAT

This is the day-to-day authoring path for the most important vendor-family
surface in truST.

## Fits

- `vendor_profile = "codesys"`
- `vendor_profile = "twincat"`
- authoring style and formatting expectations
- PLCopen/CODESYS interchange
- TwinCAT-adjacent authoring expectations
- library stub strategy

## What truST is good at here

- authoring ST in a CODESYS/TwinCAT-adjacent style
- formatting and diagnostics under the vendor profile
- PLCopen interchange and migration workflows
- keeping editor productivity when vendor-specific libraries are not fully
  implemented yet

## Day-to-day workflow

1. Set `vendor_profile` in `trust-lsp.toml`.
2. Keep application code in normal `src/` folders.
3. Add `[[libraries]]` stub folders for vendor-only symbols you still need for
   completion, hover, and navigation.
4. Use PLCopen import/export when moving source in or out of vendor tooling.

## Library stub pattern

For symbols from vendor libraries that truST does not ship, use the stub
pattern shown in `examples/vendor_library_stubs`:

- declare the symbols locally in ST
- wire the folder through `[[libraries]]`
- keep expectations clear: symbol/type contracts, not vendor runtime semantics

## Best starting points

- [PLCopen interoperability](plcopen.md)
- [Vendor libraries](vendor-libraries.md)
- [Vendor profile reference](../reference/config/vendor-profiles.md)
- `examples/vendor_library_stubs`

## Limits

truST is strongest on authoring and interchange, not on reproducing every
vendor project package behavior.

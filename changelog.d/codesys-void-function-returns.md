### Added

- The `VOID` return type of a function, under the CODESYS profile: `FUNCTION MemCopy : VOID` declares a
  function without a result, as the vendor libraries declare it (`WagoSysPlainMem.MemCopy`). The function
  carries no return slot, a call captures no result, and the body may exit early with a value-less `RETURN;`
  (specification 04 section 2, `docs/IEC_DEVIATIONS.md` DEV-043).

### Changed

- Under the IEC rules, a function declared `: VOID` is now an error (`E207`): IEC 61131-3 has no `VOID` data
  type. Before, every profile accepted it. A project that uses it either selects the CODESYS profile
  (`vendor_profile = "codesys"` in `trust-lsp.toml`) or declares the function without a result type
  (`FUNCTION LogMessage`, specification 04).

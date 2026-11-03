### Added

- The `VOID` return type of a function, under the CODESYS profile:
  `FUNCTION MemCopy : VOID` declares a function without a result — the
  function carries no return slot, a call captures no result, and the
  body may exit early with a value-less `RETURN;` — as the vendor
  libraries declare it (`WagoSysPlainMem.MemCopy`). The IEC dialect now
  rejects the declaration with the return-type error `E207`, because
  IEC 61131-3 knows no `VOID` data type; before, every dialect accepted
  it silently (specification 04 section 2, `docs/IEC_DEVIATIONS.md`
  DEV-043).

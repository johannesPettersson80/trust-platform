### Fixed

- Completion and hover know `DIFU`, `DIFD`, `TP_TIME`, `TON_TIME` and `TOF_TIME` as standard
  function blocks. They show the standard documentation, and the `stdlib` setting of
  `trust-lsp.toml` now applies to them: `none` hides them like every other standard block, and
  `iec` offers the `_TIME` timers but not Mitsubishi's `DIFU` and `DIFD`, which are not IEC names
  (specifications 08 and 14).

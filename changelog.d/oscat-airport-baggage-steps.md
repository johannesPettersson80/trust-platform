### Fixed

- The OSCAT example `airport_baggage_command_observer` walks through both of its projects step by step, with the
  output of real test runs: how the Command pattern separates picking a decision from carrying it out, how the
  Observer bus tells every audit consumer, and where the classic `IF` strains. Its OOP program decided a "bag" in
  every 50 ms scan, so the audit trail counted twenty events a second while one bag stood at the scanner; it now
  decides each bag once, when the scanner reports a new bag id, and wires the diverter only once. The two short
  READMEs of its projects are gone.

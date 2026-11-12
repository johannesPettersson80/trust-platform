### Added

- Under the CODESYS profile, a declared name that is the name of a CODESYS operator, such as an enumeration member `log`
  (`LOG`), a variable `Max` or a constant `LIMIT`, is now reported with the warning `W016`. CODESYS takes its operator
  names as keywords and refuses them as names, so the warning shows before a build in CODESYS what its compiler would
  refuse; truST still runs the program (#477).

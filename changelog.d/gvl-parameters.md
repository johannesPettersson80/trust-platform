### Fixed
- Control Studio's parameter sets (Apply's gains) take parameters and a mode that a CODESYS project keeps in GVLs, when the
  loop's task owns each global, and refuse one that another task or a function block writes with its reason. Experiments
  step a writable global, such as a GVL's manual output, as they step a program's variable (protocol annex E2.3, E7.1).

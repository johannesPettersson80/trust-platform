### Added
- `{attribute 'hide'}` and `{attribute 'hide_all_locals'}` now take effect at run time too: the debugger's lists of
  variables (globals, a frame's locals, a program's or an instance's members, the instances) and the HMI Builder's
  variable picker leave a hidden variable out, on every runtime, a board included, as CODESYS hides it from monitoring
  and debugging. Code, `eval`, `set`, `var.force`, a watch and an HMI binding still reach it by its name
  (specification 10 §4.8).

### Fixed
- Control Studio's Keep in project writes gains that a CODESYS project keeps in a GVL (`GVL_Par.lrKp`) into the GVL's
  declarations, as it writes a program's own gain variables, instead of refusing them as "not declared in PROGRAM
  GVL_Par" (specification 35 B6.5).

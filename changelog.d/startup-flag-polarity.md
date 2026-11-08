### Added

- Control Studio's setup recognises CODESYS's startup pattern `IF bInit THEN … bInit := FALSE; END_IF`
  as a startup flag. Validate starts such a flag FALSE, its value once the startup has run, so the
  startup code no longer overwrites the values Control Studio seeds (specification 35 C1). A flag
  tested as `IF NOT <flag> THEN … <flag> := TRUE` starts TRUE as before.

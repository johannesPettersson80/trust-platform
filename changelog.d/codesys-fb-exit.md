### Added

- Under the CODESYS profiles a cold or a warm restart of a program that has run, and a download that replaces it,
  first call `FB_exit(bInCopyCode := FALSE)` of every instance whose type declares it, in the reverse of `FB_init`'s
  order, as CODESYS does; a declaration marked `{attribute 'no-exit'}` leaves out the instances it declares. A fault
  in `FB_exit` fails a restart; at a download the new program is installed all the same and its event records begin
  with the fault.

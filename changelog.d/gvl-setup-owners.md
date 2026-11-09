### Fixed
- Control Studio's setup and loop discovery refuse up front a loop whose parameter set, setpoint or output is a global that
  the loop's task does not own (another task, or a function block that any task may run, writes it), with the sentence
  that names the global, who writes it and the loop's task, instead of accepting the loop and refusing at Identify or
  Apply. The compiled catalog states each global's owner (specification 35 B1.2, B1.3).

### Fixed
- Control Studio's transfers (Identify's and Apply's switches between Manual and Auto) take a loop whose mode, setpoint or
  output lives in a GVL, as CODESYS projects keep them, when the loop's task owns each global: no program writes it, or
  only the programs that task runs do. A global that another task, or a function block that any task may run, writes is
  refused with its reason instead of "transfer members must belong to one compiled program". The measurements and other
  guards, which a transfer only reads, may be any variable, another task's included (protocol annex E2.3).

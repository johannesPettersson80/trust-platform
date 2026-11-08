### Fixed
- A PID group whose shared Auto/Manual signal, setpoints, manual outputs or measurements live in a GVL, as CODESYS projects
  keep them, is no longer refused as belonging to two programs: its members, gains and states stay in one program, and
  setup refuses up front only a written signal that the group's task does not own, or one in another program (specification
  35 C7).

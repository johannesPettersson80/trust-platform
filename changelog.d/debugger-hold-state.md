### Fixed

- On a computer, the debugger reported a breakpoint's, a pause's or a step's stop a moment before
  it published the halted scan's values, so a client that read them as soon as it found the stop
  could show the last completed scan's values at the stop until the next one. The stop and the
  halted state are now published together (specification 11 §6.13.1).

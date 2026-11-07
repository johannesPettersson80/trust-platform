### Changed

- `trust-runtime ctl breakpoints-set` takes one-based lines, as an editor shows them, and
  `breakpoints-set` and `breakpoints-list` print one-based lines and columns; a line 0 is refused.
  The command passed its lines on as the protocol's zero-based ones, so `breakpoints-set main.st 12`
  set the breakpoint at the editor's line 13, and printed the lines zero-based. The console's
  `break <file>:<line>` counts the same way, and `breaks` shows each breakpoint's line instead of a
  byte offset.

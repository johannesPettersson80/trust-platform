### Changed

- W012 counts every write a program makes to a global that tasks share (specification 28 §6): an
  output binding (`fbTimer(Q => gDone)`), an argument to an in-out parameter, by name or by
  position, a FOR loop's control variable, and a call of a global function block instance or of a
  method of one, as Beckhoff's SA0006 counts a method call. It counted only the targets of `:=`, so
  two tasks writing a global in those ways stayed silent. An index of an assigned element, such as
  `i` in `gArr[i] := 0`, is a read, where W012 counted it written.

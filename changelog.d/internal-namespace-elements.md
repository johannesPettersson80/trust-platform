### Added

- A namespace's types, functions, function blocks, programs, classes and interfaces can be declared `INTERNAL`, as
  IEC 61131-3 §6.9.2 and its examples write it: `FUNCTION INTERNAL TimeTick : DWORD`, `CLASS INTERNAL B`, and
  `TYPE INTERNAL` for every type of the block. Such an element is used only inside its own namespace: a call of it, a
  name of it brought in by `USING`, or a variable of its type elsewhere is an error, and so is `INTERNAL` outside any
  namespace. Before, `FUNCTION INTERNAL` was a syntax error ("expected function name").

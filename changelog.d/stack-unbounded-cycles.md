### Fixed

- The stack bound of a compiled program counts a function's call of itself, which the call relocations
  leave out, and the build refuses a module whose call graph has a cycle that no recursive call of
  Structured Text bounds, naming its functions, where the bound took it for a recursion the call
  depth bounds (specification 44 §8.5).

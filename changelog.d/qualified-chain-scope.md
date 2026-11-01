### Fixed

- An unused-symbol warning can no longer hide behind a local variable that shares its name with a
  global list: when a qualified use (`GVals.Member`) resolves its first segment, that segment
  resolves in the expression's scope before the project-wide lookup, so a local named like the list
  marks only itself (specification 28 §1).

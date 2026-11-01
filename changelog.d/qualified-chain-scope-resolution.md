### Fixed

- Unused-symbol analysis: the head of a qualified value access chain
  resolves in the referencing expression's scope first — a local variable
  named like a global list (`GVals`) no longer makes `GVals.Member` mark
  the global list's member used and hide its unused warning. A head the
  scope does not declare still walks the namespaces and the global lookup
  as before (specification 28 §1).

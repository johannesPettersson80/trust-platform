### Fixed

- A local, a parameter or a member of a POU named like a global namespace or program now hides it in a dotted name,
  as local names shadow global names (specification 09 §1.3). Before, `GVals.Member` read the namespace's `Member`
  even with a local `GVals`: a running program read the wrong variable, and the editor reported neither a wrong type
  nor the namespace member's unused warning (issue #203).

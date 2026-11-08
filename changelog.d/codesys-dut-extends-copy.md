### Fixed

- A value of a DUT that `EXTENDS` another is compiled where the base's type is wanted
  (specification 02 §3.5.1): as a method's or function's input, as an assignment, into a
  `STRUCT OVERLAP`'s member included, and as a call's result. The build refused every one of
  the copies with "an assignment between different types" (specification 44 NX-09).

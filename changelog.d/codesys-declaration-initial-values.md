### Fixed

- A declaration's initial value converts by the program's conversion rules (specification 07
  §2.10): `bXMax : BYTE := ANY_TO_BYTE(255)` in a DUT compiles, and `ANY_TO_BYTE(300)` keeps the
  low-order bits of the target's width, 44. The build refused every such conversion with
  "ANY_TO_BYTE in a declaration of a program built with the CODESYS conversion rules is not
  compiled yet" (specification 44 NX-09). A conversion outside an initial value (an array's
  bound, the program configuration) is still refused.

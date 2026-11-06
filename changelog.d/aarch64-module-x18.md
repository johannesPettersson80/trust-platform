### Fixed

- On a Mac with Apple silicon, a compiled program could compute a wrong value or crash at random, more often the
  busier the computer. Functions of the program library that a program links kept values in the processor register
  x18, which macOS may overwrite when it switches threads; among them were the functions behind `CONCAT`, `FIND`,
  numbers as text and the trigonometric functions. The library now leaves x18 alone, as the program's own code
  already did (specification 44 §4.4).

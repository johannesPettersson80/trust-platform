### Added

- An interface may extend several interfaces, as IEC 61131-3 §6.6.6.6 and CODESYS allow:
  `INTERFACE IServiced EXTENDS IControl, IMaintenance`. It inherits the prototypes of every base, a value of it
  converts to each base, and a function block or class that implements it implements them all. Two bases that give
  prototypes of one name with different signatures make the interface an error (E208).

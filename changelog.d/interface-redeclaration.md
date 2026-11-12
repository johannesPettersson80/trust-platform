### Changed

- An interface that declares a method or property prototype again that it inherits from a base interface is now an
  error (E208), as IEC 61131-3 §6.6.6.6.2 requires, reported at the repeated prototype. A project that does so gets
  the error after updating; remove the repeated prototype, which the interface already inherits. Under the CODESYS
  profile the declaration stays accepted.

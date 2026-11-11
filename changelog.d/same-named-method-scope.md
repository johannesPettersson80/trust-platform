### Fixed

- Inside a method whose name another POU also has (a method of two function blocks or classes, or an
  interface's method and the method that implements it), go to definition, completion, hover, find
  references, rename and the call hierarchy now work in that method's own scope. They took the first
  POU of that name before, so a local went to another method's declaration and a call was shown as a
  call of the interface's method (specification 14 §6.3.1, §6.3.2).

### Fixed
- Who writes a global is judged along the call tree, as CODESYS's static analysis does: a global that a function block, a
  function or a method writes belongs to the task whose programs call it. Control Studio therefore takes a loop whose output
  a function block writes, as CODESYS projects often do, instead of refusing it as shared, and warning W012 now reports a
  global that a function block writes from two tasks (specification 28 §6; protocol annex E2.3).

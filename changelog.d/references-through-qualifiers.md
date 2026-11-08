### Fixed

- Find references, rename, the references code lens and the call hierarchy's incoming calls find a
  global used through its GVL's or namespace's qualifier (`GVL_IO.level`) and a member or method
  reached through one (`GVL_IO.tank.Start()`), where they found only the declaration
  (specification 14 §6.3.2). A rename left such uses unchanged, so the project no longer built.

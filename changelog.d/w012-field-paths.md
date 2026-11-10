### Fixed

- W012, the warning for a global that several tasks use unsafely, now also sees a use through a field
  of a dereferenced reference or of an array element, such as `Ref^.value` or `Items[0].value`, and
  counts a global used as an index inside such a path as read, as specification 28 §6 states. A
  program that wrote such a path in one task while another task read it was not warned before.

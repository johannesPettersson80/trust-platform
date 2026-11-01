### Fixed

- Deleting an element in the Ladder editor takes it out of the rung's logic, as CODESYS and TwinCAT do: its wires go
  with it, a series element closes the gap and a parallel leg made of the element alone leaves the other legs
  (specification 25 §11.4). Before, the wires to a deleted element stayed in the file and kept the power from the
  node they ended at, so a coil behind a deleted contact never switched on, and the implicit local of the deleted
  contact's operand stayed declared.

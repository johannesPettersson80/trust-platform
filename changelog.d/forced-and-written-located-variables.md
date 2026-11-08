### Fixed

- `var.force` of a variable bound to an input address (`Level AT %IW0 : INT`) holds: the program
  reads the forced value instead of the input until the force is released. Before, the scan's input
  sample replaced the forced value before the program ran, though `var.force` answered `forced`.
- A `set` of a variable bound to a marker address lasts. Before, the next scan replaced it with the
  marker's old value. A written input keeps its value until a driver reads that input again.
- A `TIME` bound to a 32-bit address (`AT %ID8`) refuses a write that its whole milliseconds cannot
  hold, a negative duration or one past about 49.7 days, with a `write_rejected` event.

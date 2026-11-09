### Fixed

- A write at an output address (`io.write`, a Live Values write, an experiment's step on an addressed output) also writes
  the variable bound there, so the program reads it and the output keeps it until the program writes the variable. Before,
  the next scan's publication of the bound variable replaced it, so Identify could not step a loop whose program stops
  writing its output in Manual, with the output named as its own manual output (specification 44 §13.3).

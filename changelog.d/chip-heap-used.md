### Fixed

- An ESP32-C6's `capabilities` reports the heap's used bytes over its data regions, the regions its
  capacity counts. They counted the program region's blocks too, an installed module's copy and an
  upload's image, so the used bytes could come close to the capacity while the data heap still had
  room (specification 34 MEM-12).

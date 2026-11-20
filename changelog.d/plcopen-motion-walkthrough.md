### Changed

- The PLCopen Motion starter is now a step-by-step walkthrough. After the axis's first move to 25, a request on
  `%IX0.0` moves it to the other end, and its position shows at `%MD4`. Writing the request in Live Values shows that
  a motion command starts on a rising edge: writing TRUE again over TRUE starts nothing, while FALSE and then TRUE
  moves the axis back. Its values come from a run of the packaged extension, which a new acceptance journey repeats
  on every package.

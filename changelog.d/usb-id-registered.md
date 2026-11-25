### Changed

- The STM32U5A9 firmware presents truST's own USB ID, `1209:7472`, which pid.codes has registered
  for truST's PLC runtime, instead of the pid.codes test ID `1209:0001`. The device search
  recognizes both as a truST runtime.

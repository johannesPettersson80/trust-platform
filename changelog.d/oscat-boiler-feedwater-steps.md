### Fixed

- The OSCAT example `boiler_feedwater_alarm` walks through both of its projects step by step, with the output of real
  test runs: the hysteresis band, the alarm queue between two scans, and how the OOP version composes two library
  objects. Its README called the alarm a low-level alarm, but the program raises a high-level one; and the OOP
  program reset its alarm queue in every scan, which threw away codes no one had read yet; it now sets the switch
  and the queue up once. The two short READMEs of its projects, which only pointed one folder up, are gone. CI runs
  every `trust-dev test` that a converted OSCAT README quotes and compares the output.

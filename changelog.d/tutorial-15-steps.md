### Fixed

- Tutorial 15 walks through two runtimes sharing a value over the mesh step by step, with the values of a real run:
  PLC-B's lamp crossing to PLC-A's variable and back, and PLC-A's search finding PLC-B by its resource name. Its
  Try-its show a subscription under the wrong name, a token on one mesh only, and a publisher that goes away, whose
  last value the subscriber keeps. CI runs its steps with two runtimes.

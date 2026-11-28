### Fixed

- The PLCopen motion single-axis demo walks through its sequence step by step, with the values of a real run: how
  the program calls every block every scan and steps on their outputs, the buffered relative move that the software
  limit stops at 90.0, and the globals read with `trust-runtime ctl read`. It built and ran the project inside the
  repository; it now copies the project with its library beside it, so that a run leaves the checkout as it was.
  Its Try-its change the move, turn the limit off and give the absolute move an acceleration it refuses, which takes
  the program to its fault step and removes the power. CI runs the walkthrough and the Try-its.

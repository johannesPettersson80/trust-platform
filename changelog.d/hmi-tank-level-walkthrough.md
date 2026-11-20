### Changed

- The HMI tank level example is now a step-by-step walkthrough of operating its screens. You run its runtime,
  connect to it, open the Overview in the HMI Builder's Live mode, log in as `operator`, raise the setpoint until
  the high-level alarm comes in, acknowledge it, and lower the setpoint until the alarm leaves the list. Its values
  come from a run of the packaged extension, which a new acceptance journey repeats on every package.

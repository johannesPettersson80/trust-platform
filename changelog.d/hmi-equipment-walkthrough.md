### Changed

- The HMI equipment example is now a step-by-step walkthrough of its equipment panels. P-101 runs in Auto with Start
  and Stop greyed. LIC-101 follows a new setpoint, drains the tank in Manual until LT-101's Low alarm comes in, and
  refills it in Auto without a bump. Its values come from a run of the packaged extension, which a new acceptance
  journey repeats on every package. The README no longer says the loop holds 1.5 m from the start: after a cold start
  the level dips first, because the loop starts from a closed valve.

### Added

- Identify stops an experiment once the measured output leaves its permitted range. Every plan that writes carries the
  loop's measured output and its permitted range, the range **Limits…** sets; the plan's stop conditions and a physical
  target's approval name it, and the runtime restores what it wrote. The stop names the value the runtime read and the
  limit it passed, and asks for a smaller step (specification 35, B3.4 step 6).

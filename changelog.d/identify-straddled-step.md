### Fixed

- Identify placed a step whose edge fell inside an averaged sample at the larger of its two partial changes. Past the
  middle of a sample the straddling sample joined the baseline and moved the fitted gain (1.527 instead of 1.5 at 55 %
  on a simulated first-order plant), and a restoration that straddled less evenly could be taken for the step, which
  refused the recording with "A bounded input step with pre- and post-step data is required.". A change through a
  straddling sample now counts whole, and the step starts at that sample (numerical annex N2.2).

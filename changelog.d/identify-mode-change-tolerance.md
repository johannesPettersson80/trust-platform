### Fixed

- Identify's switches between Manual and Auto, and Validate's live model check that uses them, take
  the transfer tolerance that the study's requirements review, as Apply does. Without one they had
  allowed a first-output change of 0.000001, finer than a REAL output resolves above 8, so a
  bumpless hand-back that moved the output by one REAL step was refused and the loop stayed in Manual.
- No Auto-entry tolerance is below the output's resolution, 2⁻²³ of its larger limit.
- A refused Auto entry names the change the runtime measured, from what to what, and the tolerance it
  allowed.

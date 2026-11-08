### Fixed

- Tune's finite search over a program's coefficients ("Other law", specification 35 C5) designs to the
  requirements Automatic fills from the model when the study holds none, as every other Tune does
  (B4.3). It refused with "The finite search needs the requirements", which left the command line,
  whose verbs set no requirements, no way to tune such a loop.
- Setup gives a finite search the budgets of specification 35 A7: 10,000,000 scans for its trials
  together and 60 minutes. It wrote 50,000 scans and 60 s, which a loop that settles in a few minutes
  spent before its second trial. A loop file written before keeps its own values; raise
  `controller.project.maximumCycles` and `wallBudgetMs` there, before identifying, since a changed
  loop file is a new loop revision.

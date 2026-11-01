### Changed

- An HMI switch sends `toggle` with the value it shows, and the runtime writes the opposite, so the value the operator
  saw decides. A switch, selector, mode selector or slider that shows no value is now refused by the runtime with its
  reason, which goes to the audit log like every other command; before, the page kept that refusal to itself
  (specification 39 §5.8, decided by the owner on 1 November 2026).

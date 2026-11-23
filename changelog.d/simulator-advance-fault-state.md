### Fixed

- In `trust-runtime simulate`, an advance that a cycle's fault ends answers with the state the
  project's fault policy leaves, `faulted` after a halt, instead of sometimes `running`: the answer
  waits until the scan loop has applied the policy.

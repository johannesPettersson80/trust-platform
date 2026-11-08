### Fixed

- A loop whose return to Auto the runtime refuses is no longer locked. Identify, Validate's model
  check and a Cancel end the operation, say that the loop stays in Manual and is to be returned to
  Auto by hand, and leave nothing journaled. Before, every Cancel retried the refused transfer, and
  only deleting the loop's journal freed the loop.
- When an identification fails and its loop cannot be returned to its mode either, the answer states
  both.

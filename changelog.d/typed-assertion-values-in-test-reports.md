### Added

- The JSON report of `trust-dev test` gives a failed assertion's values, typed: the test's
  `assertion` object holds its `name` and, as the assertion has them, its `expected` and `actual`
  values, a comparison's `bound` and `ASSERT_NEAR`'s `delta`, each as `{"type": "INT", "value": 2}`.
  A device's run gives the same, from its `cycle.step`.

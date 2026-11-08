### Added

- An ordinary `PROGRAM` whose declaration carries `{attribute 'test'}` is a test program, as a
  `TEST_PROGRAM` is (specification 22): `trust-dev test` and the VS Code Testing view run it, a
  production build leaves it unbound, and it is no unused program. CODESYS has no `TEST_PROGRAM`
  and ignores an attribute it does not know, so a CODESYS project can keep its tests in the project
  itself. The attribute works under every vendor profile.

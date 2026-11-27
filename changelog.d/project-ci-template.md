### Fixed

- The project CI template, `.github/workflows/templates/trust-runtime-project-ci.yml`, now works in a project's own
  repository: it installs the truST release that `TRUST_VERSION` names, checked against the release's `SHA256SUMS`,
  instead of building truST with `cargo`, which only a truST checkout could; it starts on changes to `src/` and the
  project's `.toml` files instead of a `sources/` folder that projects do not use; and it keeps the JUnit report of
  a failing run too.
- Tutorial 21 walks through CI for a PLC project step by step, with the values of a real run: the build, validation
  and test reports in CI mode, the exit codes of a failed test (12), a build failure (11) and an invalid
  configuration (10), and the template that runs them on every change.

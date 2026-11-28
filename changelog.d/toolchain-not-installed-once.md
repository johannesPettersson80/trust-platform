### Fixed

- A command that needs the truST toolchain and finds none now names it once: `trust-dev test` said "the tests need
  the toolchain: the Rust toolchain: the truST toolchain 0.44.1 is not installed: ...", and `trust-runtime build`
  and `trust-runtime toolchain status` began with "the Rust toolchain: ". Each now says "the truST toolchain
  0.44.1 is not installed: `trust-runtime toolchain install` installs it, ...", which a build without a terminal
  ends with "; or build again with `--install-toolchain`". `trust-runtime bench` and `conformance` and
  `trust-dev bench` lost the same extra words.

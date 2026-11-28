### Fixed

- The project CI image, `docker/ci/trust-runtime-ci.Dockerfile`, now works for a project's own pipeline: it installs
  the truST release that the build argument `TRUST_VERSION` names, for the image's platform, checked against the
  release's `SHA256SUMS` before it is unpacked, and that release's toolchain. It was a Rust image whose usage built
  truST with `cargo run`, which only a truST checkout can, and ran the deprecated alias `trust-runtime test`. The CI
  guide shows how to build it and run the three CI steps in it.

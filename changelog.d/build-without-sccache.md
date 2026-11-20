### Fixed
- A clone of the repository builds with `cargo build` alone: the repository no longer makes sccache
  the compiler wrapper, which failed every build on a computer without it. sccache stays optional
  (`CONTRIBUTING.md`). Found through the IEC 61131-3 conformance suite, whose CI had to switch the
  wrapper off to build truST.

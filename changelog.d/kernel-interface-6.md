### Changed

- The kernel interface is version 6, which adds the fault of a Rust POU (specification 44 Appendix A,
  specification 48 RU-14). A program image built by an earlier release is refused with `kernel
  interface 5 required, this runtime has 6: rebuild the project`; build the project again, and
  update a board's firmware to this release before deploying to it.

### Changed

- The kernel interface is version 7, which adds the fault of a Rust POU and the interval of the
  running task (specification 44 §8.2, Appendix A; specification 48 RU-14, RU-16). A program image
  built by an earlier release is refused with `kernel interface 5 required, this runtime has 7:
  rebuild the project`; build the project again, and update a board's firmware to this release
  before deploying to it.

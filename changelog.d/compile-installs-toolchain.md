### Fixed
- On a computer that never ran truST, **Compile** of a project with a Rust crate installs the truST
  toolchain, as **Start** does, and checks again. It failed with "the truST toolchain is not
  installed", which Problems did not show, and the failed Compile held back the Start that would
  have installed it: the Rust function block starter and the projects of Create Project's two Rust
  starts could not start.

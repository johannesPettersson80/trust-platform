### Fixed
- The **Rust function block** starter's walkthrough works in Live Values: its program's setpoint is the
  simulated input `%IL0` and its speed and at-speed flag are the outputs `%QL0` and `%QX8.0`, so Live
  Values shows them and writes the setpoint. Its README told the user to watch and write variables
  without an address, which Live Values does not list. An acceptance journey now runs the walkthrough
  on the packaged extension, Start building the crate with truST's own toolchain.

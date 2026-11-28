### Fixed

- PLCopen Motion: a motion command (`MC_Home`, `MC_Stop`, `MC_Halt`, the moves and the homing blocks) issued while
  the axis is Disabled now ends in `Error` with `mcERR_NotPowered`, and one issued in ErrorStop with
  `mcERR_InvalidState`, as PLCopen Part 1 §2.1 requires: in Disabled the blocks do not move the axis, and in
  ErrorStop no motion command is accepted until `MC_Reset`. Before, the virtual axis took such a command, moved
  and left Disabled or ErrorStop. The gallery starter's copy of the library has the change too.

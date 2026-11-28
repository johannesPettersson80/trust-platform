### Fixed

- PLCopen Motion: `MC_Stop` now aborts the on-going move (`CommandAborted`), and while the axis is in Stopping
  every other motion block is refused with `mcERR_InvalidState`, as PLCopen Part 1 §3.3 requires; a second
  `MC_Stop` is accepted. Releasing it takes the axis from Stopping to Standstill and no longer takes an axis that
  went to ErrorStop in the meantime out of it without a reset. Before, the move went on during the stop and any
  block could move the stopping axis. The gallery starter's copy of the library has the fix too.

### Fixed

- PLCopen Motion: `MC_HomeDirect`, `MC_HomeAbsolute` and `MC_FinishHoming` no longer take a Disabled axis to
  Standstill or an axis out of ErrorStop. As PLCopen Part 5 allows, they home an axis without power: on a Disabled
  axis or one in ErrorStop they set the position and the homed flag and leave the state; from Homing, or after the
  motion command they abort, the axis ends at Standstill. `MC_FinishHoming`'s relative move needs a powered axis
  without an error and is refused otherwise. Before, the three passed through Homing to Standstill from any state,
  so that an unpowered axis became one that took moves.

### Added

- The Plant library's control valve, variable-speed motors and heater implement the new interface
  `IControlledDevice`: `FollowsAuto()` tells whether the device follows its `Auto…` value now, and
  `GetActualValue()` is what it does (its position, speed or power), so a controller can track the
  device while an interlock, Manual, a stop or a missing permissive keeps it from following, and
  neither wind up nor bump (Plant library specification §5, §6.2, §6.3, §6.9).

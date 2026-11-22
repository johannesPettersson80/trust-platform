### Changed

- A PID loop that Create PLC code writes tracks, in Auto, the equipment its output drives while the
  equipment does not follow it: while a control valve is interlocked, in Manual or not yet released, or a
  drive or heater is stopped or in Manual, the loop starts each scan from what the equipment does
  (`PIDMode#Track` with its `GetActualValue()`), so it no longer winds up against a value the equipment
  does not take and takes over without a bump when the equipment follows again; after a restart it
  starts from the equipment as the restart left it. In Manual the operator's output stands. In Auto and
  while tracking the loop's manual output follows its output. The PID's component definition asks for it
  under `[program.tracking]` (specification 39, §10.5; Plant library §8.1). Press Create PLC code again in
  a project with PID loops to take it.
- `Control.PID` runs a `Track` that the program passes as `Auto` while a simulated plant holds its
  measurement through a `pid:` feedback target, as Control Studio's model runs do. The model takes the
  controller's output as its input, as tracked equipment does when it follows, so Validate tests the
  controller of a tracking loop instead of the equipment's idle output (specification 46, §13.2). Live
  runs and Identify still track.

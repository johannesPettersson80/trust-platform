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

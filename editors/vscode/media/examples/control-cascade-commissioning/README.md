# Cascade temperature control

A jacketed vessel heated through its jacket, controlled by two `Control.PID` blocks in cascade: the outer
PID sets the jacket-temperature setpoint from the vessel error, the inner PID sets the heater power from the
jacket error. Each has its own Auto/Manual switch (`OuterAuto`, `InnerAuto`) and manual output
(`OuterManualC`, `InnerManualPct`).

Set it up in Control Studio with **Configure control loop**: the form recognises the cascade from the program
and writes two linked loops. Tune the inner loop first with the outer controller in Manual, apply it, then
tune the outer loop with the inner controller in Auto.

Give each loop its own identification duration. The jacket answers the heater within about a second, while
the vessel follows over tens of seconds: identify the inner loop over a few seconds, so its model describes
the jacket, and the outer loop over about half a minute, so its model describes the vessel with the inner
loop closed. Because the program sets its manual outputs in its startup code, the form asks for the startup flag
(`Initialized`).

The plant is simulated (see `simulation.toml`); results are simulation evidence, not a site qualification.

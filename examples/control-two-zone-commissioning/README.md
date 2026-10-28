# Two-zone temperature control

Two heated zones that share a wall, each controlled by its own `Control.PID` with its own Auto/Manual
switch (`Zone1Auto`, `Zone2Auto`) and manual output (`Zone1ManualPct`, `Zone2ManualPct`). Each heater
warms its own zone three times as much as the neighbouring zone.

Set it up in Control Studio with **Configure control loop**, once per zone: choosing a controller fills in
its own signals. Because the program sets its manual outputs and initializes both PIDs in its startup code,
the form asks for the startup flag (`Initialized`).

Tune the zones one after the other (sequential loop closing). Tune zone 1 with zone 2 in Manual and apply
it. Then identify and tune zone 2 while zone 1 runs in Auto: zone 1 now compensates the heat that heater 2
pushes through the wall, so zone 2 responds less to its own heater. At steady state the gains are 0.225 and
0.200 °C per %; their ratio, 1.125, is the relative gain of the pairing, close to 1, so one PID per zone works.
In a recorded run with 40 s identification steps, zone 1 was identified as 0.220/(7.7s + 1) and zone 2, with
zone 1 in Auto, as 0.204/(6.8s + 1). SIMC gave Kp = 3.41 %/°C, Ki = 0.443 %/(°C·s) and Kp = 3.67 %/°C,
Ki = 0.540 %/(°C·s); both validated (settling 16.7 s and 16.2 s for a 1 °C step) and were applied.

The plant is simulated (see `simulation.toml`); results are simulation evidence, not a site qualification.

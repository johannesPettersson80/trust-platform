# ON/OFF temperature control

A cold room held near 5 °C by switching a compressor on and off. The program switches on above
`SetpointC + HysteresisC` and off below `SetpointC - HysteresisC`, and keeps the compressor in each state
for at least `MinimumOnS` or `MinimumOffS` seconds to protect it. `AutoMode` selects Auto; in Manual the
compressor follows `ManualPct` (0 or 100 %).

Set it up in Control Studio with **Configure control loop** and the **ON/OFF** structure: the
compressor command switches between 0 and 100 %, the hysteresis is half the switching band (`HysteresisC`),
the compressor itself needs at least 20 s ON and 20 s OFF (the equipment minimum times), and the retained
switching states are the latch `CoolingOn` and the dwell timer `DwellS`. Tune then chooses the band from the
required accuracy and the minimum times from the equipment limits; Validate checks the switching levels, the
dwell times and the temperature band on the actual ST.

## Measured in Control Studio

- **Identify** (the compressor switched from 0 to 100 % for 300 s): G(s) = −0.0986/(57.5 s + 1)·e^(−1.1 s),
  close to the simulated room's −0.1/(60 s + 1).
- **Tune** with a settling band of 0.5 °C, an allowed error of 2 °C, a settling time of 90 s and a test change
  of −7 °C from 12 °C: hysteresis 0.5 °C (the half band equals the settling band) and minimum ON and OFF times
  of 20 s (the compressor's own minimums; Tune never goes below them).
- **Validate** on this program: the room reaches the setpoint band in 61.6 s and then cycles within 1.63 °C of
  the setpoint; the compressor takes only 0 and 100 %, and every ON and OFF interval lasts at least 20 s.

Why the cycle is wider than the band: the compressor switches off only after it has run for 20 s and when the
room is below 4.5 °C, and on only after 20 s off. While it waits, the room keeps moving, fastest when it
warms (the open-loop steady state is 12 °C). The allowed error must cover that swing, not only the band.

The plant is simulated (see `simulation.toml`); results are simulation evidence, not a site qualification.

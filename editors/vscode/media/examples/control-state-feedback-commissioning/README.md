# State-feedback level control

Two open tanks in series: the pump fills tank 1, which drains into tank 2. The program holds tank 2's level at
its setpoint with a state-feedback law that uses both measured levels:

PumpM3PerS = 0.010 + N·(Tank2SetpointM − 0.25) − K1·(Tank1M − 0.25) − K2·(Tank2M − 0.25)

The law works in deviations from the operating point, both tanks at 0.25 m with the pump at 0.010 m³/s, so the
plant must be identified at that point. `FeedbackEnabled` selects Auto; in Manual the pump follows
`ManualM3PerS`.

Set it up in Control Studio with **Configure control loop** and the **State feedback** structure: the plant
input is the pump (0–0.02 m³/s), the controlled output tank 2's level with its setpoint and the gain `K2`, the
reference gain `N`, and one further measured output, tank 1's level with the gain `K1`. Studio identifies a
model whose states are the two measured levels, so each gain belongs to one level. Tune designs the LQR gains,
weighted by the signal ranges; Validate runs this program with them.

## Measured in Control Studio

- **Identify** (Automatic chose a pseudo-random binary sequence of ±0.002 m³/s around 0.010 m³/s for 300 s): a
  model whose states are the two levels, sampled every 0.1 s,
  x[k+1] = [0.996, 0.004; −2.1·10⁻⁵, 0.996]·x[k] + [0.0004; 0.1996]·(u[k] − 0.01), with x = (tank 2, tank 1)
  measured from 0.25 m. The pump fills tank 1 directly; tank 2 only through tank 1.
- **Tune** (LQR) with a test change of 0.01 m, an allowed error of 0.003 m and a settling time of 150 s:
  K2 = 0.0106, K1 = 0.0291 and N = 0.0598 m³/s per m. The model was identified between 0.235 and 0.262 m,
  so Tune refuses larger test changes and names the largest it supports.
- **Validate** on this program: tank 2 settles in 35.6 s with a steady error of 0.00004 m, and returns from a
  disturbed state in 20 s.
- **Apply** waits until both tanks are back at 0.25 m, enters Auto and observes the setpoint step: **Applied**.
  The law has no integrator to make the switch bumpless, so its first output responds to how far the tanks
  still are from 0.25 m; Apply allows exactly that response to the deviation it accepts as settled.

K1 acts on tank 1, which responds to the pump before tank 2 does, so the law corrects a change before it
reaches the controlled level. N scales the setpoint so that, on the model, the level settles exactly at it.

The plant is simulated (see `simulation.toml`); results are simulation evidence, not a site qualification.

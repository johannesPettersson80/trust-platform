# Control Studio: PID loop

Commission a simulated heater with Control Studio: find out how the heater responds, let Control Studio design a PI
controller for the response you ask for, prove it on the actual program, switch it on and switch it back. You learn
the four steps every new control loop goes through, and why the controller's numbers are what they are.

## What you learn

- How **Identify plant** turns one output step into a model of the process: a gain, a time constant and a delay.
- How **Tune controller** designs a PI controller from that model for the settling time you ask for.
- Why **Validate controller** runs the actual Structured Text before anything is written to the running loop.
- How **Apply** switches the loop to Auto with the new settings, and how **Restore previous settings** undoes it.

## What you need

- truST in VS Code. No hardware: the heater is a simulated plant in the project (`simulation.toml`).
- About 15 minutes.

## Step by step

The values below are those of a run of the packaged extension; yours match them to the digits shown or come very
close.

1. In the truST view, choose **Start from example**, pick **Control Studio: PID loop** and open the copy.
2. Press **Start**. The status bar shows `truST: Simulator running`, and **Live Values** opens beside the editor. The
   heater is at 20 °C, in Manual, with 0 % output: nothing heats yet.
3. Press **Control Studio**. It opens the loop **Temperature control** on the **Identify** page.
4. Keep the method **Automatic** and press **Identify plant**. Control Studio steps the heater output, records how the
   temperature answers, puts the output back and fits a model. After about 20 seconds it shows:

   > First-order model with delay · Provisional
   >
   > G(s) = 1.49988 e^(-0.5s) / (3.99898s + 1)
   >
   > Gain 1.49988 °C per %, time constant 3.99898 s, delay 0.5 s

5. Open the **Tune** page. Set **Controller** to **P1.Controller · PI** and keep **Tuning method** on **Automatic**.
6. Press **Requirements…** and ask for this response: settling time `12 s`, settling band `0.05`, maximum steady-state
   error `0.1`; under **More…**, setpoint change `1` and validation duration `24 s`. Press **Done**.
7. Press **Tune controller**. Control Studio shows the PI controller it designed and how much margin it leaves:

   > Proportional gain (Kp) 0.8421 %/°C, Integral gain (Ki) 0.2106 %/(°C·s)
   >
   > Gain margin 19.95 dB · phase margin 80.95° · delay margin 4.47 s · peak sensitivity Ms 1.15
   >
   > Meets the requirements

   **Predicted response** plots the tuned and the current settings.
8. Open the **Validate** page and press **Validate controller**. Control Studio checks the model against a fresh
   recording, then runs the program itself, once with the current settings and once with the new ones, against the
   simulated heater, without touching the running loop. After about 50 seconds the table shows:

   | Requirement | Measured | Limit |
   |---|---|---|
   | Model agreement | 0.002001 % | ≤ 5 % |
   | Current settings: settling time, steady-state error | 7.1 s, 0.0377 °C | ≤ 12 s, ≤ 0.1 °C |
   | Candidate: settling time, steady-state error | 6.6 s, 0.0131 °C | ≤ 12 s, ≤ 0.1 °C |
   | Ranges, states and transfer | 53 checks | within declared limits |

   Every row passes, so **Candidate overall** is **Pass** and **Apply** becomes available.
9. Press **Apply**. Apply starts where the settings were validated, at 20 °C, so it first waits while the heater cools
   back from the identification step (about 13 seconds here). Then it writes the new settings, switches the loop to Auto
   and steps the setpoint to 21 °C. In **Live Values**, `P1.ManipulatedValue` jumps to about 0.93 % and settles at
   0.667 %, and `P1.ProcessValue` is within 0.05 °C of 21 °C about 8 seconds after the step. The page reports:

   > Control.PID Auto commissioning passed: recorded target error 0.01466 °C; controller remains in Auto.

10. Press **Restore previous settings**. The page reports:

    > Previous values restored and observed after ST execution; the loop stays in Manual.

    The controller has its previous settings again and is in Manual, holding the output it had (0.667 %), so the
    temperature stays at 21 °C until you change the manual output.

## What happened

**The model.** A first-order model with delay describes a process that answers a change of its input after a dead
time and then moves towards a new steady value along an exponential curve. In words: turn the heater up by 1 %,
nothing happens for half a second, and then the temperature rises towards 1.5 °C higher, two thirds of the way after
4 seconds. As a formula,

G(s) = K e^(−θs) / (T s + 1)

with K the gain (°C of temperature per % of output), T the time constant (s) and θ the delay (s). Identify measured
K = 1.49988 °C/%, T = 3.99898 s and θ = 0.5 s; the simulated heater is configured with 1.5, 4 and 0.5.

**The controller.** A PI controller sets its output from the error e = setpoint − temperature: a proportional part that
answers the error at once, and an integral part that keeps adding while any error is left, so that the error ends at
zero. As a formula, u = Kp·e + Ki·∫e dt, with Kp in %/°C and Ki in %/(°C·s). The 0.667 % the heater settles at in
step 9 is the integral part's work: 1 °C more needs 1 / 1.5 = 0.667 % more output.

**The design (SIMC).** Choose the integral time Ti = Kp / Ki equal to the plant's time constant, so that the controller
cancels the plant's lag, and choose

Kp = T / (K·(λ + θ))

so that the loop answers like a first-order curve whose time constant is λ, after the delay. λ is the speed you choose:
Automatic tries Faster (λ = max(θ, T/3) = 1.333 s), Balanced (twice that, 2.666 s) and More robust (four times,
5.332 s), and takes the most robust one whose predicted response meets the requirements. More robust settles in about
13 s (Validate measures 12.8 s for it below), too slow for 12 s, so Automatic takes Balanced. Worked:

Kp = 3.99898 / (1.49988 × (2.666 + 0.5)) = 0.8421 %/°C and Ki = 0.8421 / 3.99898 = 0.2106 %/(°C·s).

A smaller λ settles faster but leaves less margin: the delay margin of 4.47 s says the loop stays stable until the
plant's delay grows by that much, and the faster designs below leave less.

**Why Validate runs the program.** The design uses the model; Validate runs the actual `Control.PID` code in the
project, with its output limits and rate limits, against the simulated heater, and checks every requirement on that
run. The current settings (Kp 1, Ki 0.2) happen to meet these requirements too; the next section shows a requirement
they fail.

## Try it yourself

- Ask for a settling time of `6 s` (validation duration `24 s`). Balanced is now too slow, and Automatic takes Faster:
  Kp = 1.45 %/°C and Ki = 0.3637 %/(°C·s), gain margin 15.21 dB, phase margin 74.37°, delay margin 2.38 s. Validate
  measures 3.4 s for it, while the current settings fail: they do not stay within the band within 6 s.
- Ask for a settling time of `30 s` with a validation duration of `60 s`. Automatic takes More robust: Kp = 0.4572
  %/°C and Ki = 0.1143 %/(°C·s), gain margin 25.26 dB, delay margin 8.66 s. Validate measures 12.8 s: a slower loop,
  gentler on the heater and more tolerant of a model that is off.
- In `simulation.toml`, change the plant's `time_constant_s` from `4.0` to `8.0`, press **Start** again and repeat
  steps 4 to 8. Identify finds the slower heater, G(s) = 1.49975 e^(-0.5s) / (7.99755s + 1). Tune doubles the
  proportional gain to 1.68 %/°C but keeps the integral gain at 0.2106 %/(°C·s) and the same margins, and Validate
  again measures 6.6 s. Automatic now takes Faster, whose λ = T/3 = 2.666 s is the λ Balanced had for the 4 s heater.
  With Ti = T the controller cancels the plant's lag: controller times plant is Kp·K·e^(−θs)/(T·s) =
  e^(−θs)/((λ + θ)·s), in which T no longer appears. The same λ gives the same loop, whatever the time constant.

## Reference

| File | What it holds |
|---|---|
| `src/Main.st` | the program: `Control.PID` configured from the `Parameters` record and updated every 100 ms |
| `control-loop.json` | the loop Control Studio opens: the controller, its signals, its parameters and its declared states |
| `simulation.toml` | the simulated heater: gain 1.5 °C/%, time constant 4 s, delay 0.5 s, output 0–100 % at ±25 %/s |
| `fopdt-model.json` | the same plant as a model file, for the **Supplied plant model** method |
| `src/control-library/` | the bundled control library, unchanged; `SOURCE-MANIFEST.txt` records its identity |

| Signal | Binding | Unit |
|---|---|---|
| Setpoint | `P1.Setpoint`, `%ML0` | °C |
| Temperature | `P1.ProcessValue`, `%IL0` | °C |
| Controller output | `P1.ManipulatedValue`, `%QL0` | % |
| Manual command | `P1.ManualOutput`, `%ML16` | % |
| Auto/Manual mode | `P1.AutoMode`, internal BOOL | TRUE = Auto |

The task and the controller's update interval are both 100 ms. The program copies the complete `Parameters` record
into the controller every scan (`Controller.Configure`), so a change does not reset the controller's history, and
`ParameterRevision` counts the record's changes. `Controller.Initialize` runs once at startup, guarded by
`P1.Initialized`; the loop declares that flag, so Validate and Apply start the controller from its declared states and
the startup code cannot overwrite them.

**Adapting it.** Keep the controller library and its Configure, Initialize and Update calls. Change the I/O bindings,
units, scaling, output limits, the process model and the requirements together, then identify, tune and validate
again. Change the task interval and `Input.CycleTime` together.

**Before a real plant.** This example runs on the simulator. A physical loop also needs the site's I/O driver,
measurement-quality and fault supervision, a watchdog and a safe state, permission to commission, and qualification on
that plant. The simulated response is not that qualification.

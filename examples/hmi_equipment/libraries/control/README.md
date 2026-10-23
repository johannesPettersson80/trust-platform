# truST Control Library

The manual is the [Control library guide](../../docs/guides/CONTROL_LIBRARY_GUIDE.md).

The Control library provides a combined-form PID controller, its independent
low-pass filter and the advanced structures composed from them. It is intended
for cyclic Structured Text applications that supply the actual elapsed cycle
time on every update.

The public namespace contains:

- `Control.PID`, with `Configure`, `Initialize`, `Update`, and read-only `GetState` methods;
- `Control.LowPass`, with `Configure`, `Initialize`, and `Update` methods;
- `Control.SmithPredictorFOPDT`, a Smith predictor for a first-order process with dead time, and
  `Control.PIDGainScheduler`, which interpolates PID tuning over an operating point, each with
  `Configure`, `Initialize`, and `Update` methods (specification 37);
- `Control.StaticDecoupler2x2`, which maps two controller commands to two actuators and back,
  with `Configure`, `ToActuators`, `ToVirtual`, and `Snapshot` methods;
- the function `Control.ConvertPIDForm`, which converts PID tuning between the parallel, ideal
  and series forms;
- parameter, input, and initialization records; and
- enums for controller mode, anti-windup equation, and filter order.

Add the package to a project's `trust-lsp.toml`:

```toml
[dependencies]
Control = { path = "../../libraries/control", version = "0.1.0" }
```

This call-order example assumes a fixed 10 ms task. Initialize once at startup;
each later scan preserves the controller history. The complete
[commissioning example](../../examples/control-pid-commissioning/README.md)
includes signal bindings, modes, plant configuration and adaptation guidance.

```iecst
USING Control;

PROGRAM Main
VAR
    Controller : PID;
    Parameters : PIDParameters := (
        ProportionalGain := LREAL#2.0,
        IntegralGain := LREAL#0.5,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-1000.0,
        RateMaximum := LREAL#1000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    State : PIDInitialState;
    Input : PIDInput;
    Output : LREAL;
    Initialized : BOOL := FALSE;
END_VAR

Controller.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    Controller.Initialize(State := State);
    Initialized := TRUE;
END_IF;

Input.Setpoint := LREAL#1.0;
Input.ProcessValue := LREAL#0.0;
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.01;
Output := Controller.Update(Input := Input);
END_PROGRAM
```

`Controller.GetState()` returns a copy of the seven retained histories without
updating the controller. Changing that copy leaves the controller untouched.
Its `NominalOutput` is the history after anti-windup correction, so it must not
be labelled as the pre-limit request.

The normative behavior, initialization requirements, update order, equations,
and baseline limitations are defined in
[`docs/specs/45-control-pid.md`](../../docs/specs/45-control-pid.md). The
baseline intentionally does not include validation, safe/fault modes,
identification, tuning, or performance monitoring.

# truST Control Library

The Control library provides the paper-compatible combined-form PID controller
and its independent low-pass filter. It is intended for cyclic Structured Text
applications that supply the actual elapsed cycle time on every update.

The public namespace contains:

- `Control.PID`, with `Configure`, `Initialize`, and `Update` methods;
- `Control.LowPass`, with `Configure`, `Initialize`, and `Update` methods;
- parameter, input, and initialization records; and
- enums for controller mode, anti-windup equation, and filter order.

Add the package to a project's `trust-lsp.toml`:

```toml
[dependencies]
Control = { path = "../../libraries/control", version = "0.1.0" }
```

Then instantiate the function blocks by namespace or import their declarations:

```iecst
USING Control;

PROGRAM Main
VAR
    Controller : PID;
    Parameters : PIDParameters;
    State : PIDInitialState;
    Input : PIDInput;
    Output : LREAL;
END_VAR

Parameters.ProportionalGain := LREAL#2.0;
Parameters.IntegralGain := LREAL#0.5;
Parameters.OutputMinimum := LREAL#0.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#1.0;
Controller.Configure(Parameters := Parameters);
Controller.Initialize(State := State);

Input.Setpoint := LREAL#1.0;
Input.ProcessValue := LREAL#0.0;
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.01;
Output := Controller.Update(Input := Input);
END_PROGRAM
```

The normative behavior, initialization requirements, update order, equations,
and baseline limitations are defined in
[`docs/specs/32-control-pid.md`](../../docs/specs/32-control-pid.md). The
baseline intentionally does not include validation, safe/fault modes,
identification, tuning, or performance monitoring.

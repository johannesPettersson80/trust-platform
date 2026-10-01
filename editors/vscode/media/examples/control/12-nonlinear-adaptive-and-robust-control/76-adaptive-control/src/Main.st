USING Control;

(* The least-squares heat loss, Ĥ = Σxy/Σx², for a positive Σx². *)
FUNCTION LeastSquaresHeatLoss : LREAL
VAR_INPUT
    SumXY : LREAL;
    SumXX : LREAL;
END_VAR
IF SumXX > LREAL#0.0 THEN
    LeastSquaresHeatLoss := SumXY / SumXX;
ELSE
    LeastSquaresHeatLoss := LREAL#0.0;
END_IF;
END_FUNCTION

(* One second of the heat balance, C·dT/dt = a·u(t − 1 s) − H·(T − 20), integrated over [k, k + 1]: the delivered command
   is the one applied during the second before, and the mean temperature above ambient is taken at the interval's middle.
   x is that mean in °C; y = a·u − C·(T(k+1) − T(k)) is the heat lost in kW, which equals H·x. *)
FUNCTION BalanceX : LREAL
VAR_INPUT
    StartC : LREAL;
    EndC : LREAL;
END_VAR
BalanceX := (StartC + EndC) / LREAL#2.0 - LREAL#20.0;
END_FUNCTION

FUNCTION BalanceY : LREAL
VAR_INPUT
    StartC : LREAL;
    EndC : LREAL;
    DeliveredPct : LREAL;
END_VAR
BalanceY := LREAL#2.0 * DeliveredPct - LREAL#50.0 * (EndC - StartC);
END_FUNCTION

(* One estimation window of 30 one-second intervals: Ĥ from all intervals, from each half, the span of x, the largest
   residual |y − Ĥ·x|, and the decision. Accepted only for Ĥ in 7–13 kW/°C, a span of at least 1 °C, residuals within 5 kW
   and halves within 0.5 kW/°C of each other. *)
FUNCTION_BLOCK HeatLossWindow
VAR_INPUT
    Temperatures : ARRAY[0..30] OF LREAL;
    Delivered : ARRAY[0..29] OF LREAL;
END_VAR
VAR_OUTPUT
    EstimateKwPerC : LREAL := 0.0;
    SpanC : LREAL := 0.0;
    Accepted : BOOL := FALSE;
END_VAR
VAR
    K : INT;
    X : ARRAY[0..29] OF LREAL;
    Y : ARRAY[0..29] OF LREAL;
    SumXY : LREAL;
    SumXX : LREAL;
    FirstXY : LREAL;
    FirstXX : LREAL;
    Residual : LREAL;
    XMin : LREAL;
    XMax : LREAL;
END_VAR
SumXY := LREAL#0.0;
SumXX := LREAL#0.0;
FirstXY := LREAL#0.0;
FirstXX := LREAL#0.0;
XMin := LREAL#1000.0;
XMax := LREAL#-1000.0;
FOR K := 0 TO 29 DO
    X[K] := BalanceX(StartC := Temperatures[K], EndC := Temperatures[K + 1]);
    Y[K] := BalanceY(StartC := Temperatures[K], EndC := Temperatures[K + 1], DeliveredPct := Delivered[K]);
    SumXY := SumXY + X[K] * Y[K];
    SumXX := SumXX + X[K] * X[K];
    IF K < 15 THEN
        FirstXY := FirstXY + X[K] * Y[K];
        FirstXX := FirstXX + X[K] * X[K];
    END_IF;
    XMin := MIN(XMin, X[K]);
    XMax := MAX(XMax, X[K]);
END_FOR;
EstimateKwPerC := LeastSquaresHeatLoss(SumXY := SumXY, SumXX := SumXX);
SpanC := XMax - XMin;
Residual := LREAL#0.0;
FOR K := 0 TO 29 DO
    Residual := MAX(Residual, ABS(Y[K] - EstimateKwPerC * X[K]));
END_FOR;
Accepted := EstimateKwPerC >= LREAL#7.0 AND EstimateKwPerC <= LREAL#13.0 AND SpanC >= LREAL#1.0 AND Residual <= LREAL#5.0
    AND ABS(LeastSquaresHeatLoss(SumXY := FirstXY, SumXX := FirstXX)
        - LeastSquaresHeatLoss(SumXY := SumXY - FirstXY, SumXX := SumXX - FirstXX)) <= LREAL#0.5;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    (* Two heaters whose heat loss changed from 10 to 8 kW/°C, both at 35 °C with 60 points. A keeps the PI gains designed
       for 10 kW/°C with λ = 10 s: Kp = C/(2λ) = 2.5 points/°C and Ki = H/(2λ) = 0.5 points/(°C·s). B adapts. The project
       experiment holds B at 60 points for 30 s (window 1), steps it to 70 points for 30 s (window 2) and returns it to 60
       points. After each window the program estimates B's heat loss by least squares; an accepted estimate sets B's
       Ki = Ĥ/(2λ). At 90 s both heaters return to Auto and step to 38 °C. A temperature outside 0–80 °C or an update
       interval outside 100 ms ± 10 ms latches zero output. *)
    FixedC : LREAL := 35.0;
    AdaptiveC : LREAL := 35.0;
    FixedPct : LREAL := 60.0;
    FixedLoadKw : LREAL := 0.0;
    AdaptivePct : LREAL := 60.0;
    AdaptiveLoadKw : LREAL := 0.0;
    SetpointC : LREAL := 35.0;
    Estimate1 : LREAL := 0.0;
    Estimate2 : LREAL := 0.0;
    Span1 : LREAL := 0.0;
    Span2 : LREAL := 0.0;
    AdaptiveKi : LREAL := 0.5;
    ManualPct : LREAL := 60.0;
    Updates : DINT := 0;
    Second : DINT := 0;
    Temperatures : ARRAY[0..30] OF LREAL;
    Delivered : ARRAY[0..29] OF LREAL;
    PreviousSecondPct : LREAL := 60.0;
    FixedLoop : Control.PID;
    AdaptiveLoop : Control.PID;
    FixedParameters : Control.PIDParameters := (
        ProportionalGain := LREAL#2.5,
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
    AdaptiveParameters : Control.PIDParameters := (
        ProportionalGain := LREAL#2.5,
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
    State : Control.PIDInitialState;
    FixedInput : Control.PIDInput;
    AdaptiveInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    Window : HeatLossWindow;
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    Accepted1 : BOOL := FALSE;
    Accepted2 : BOOL := FALSE;
    FaultLatched : BOOL := FALSE;
    ClockPrimed : BOOL := FALSE;
    ClockValid : BOOL := TRUE;
    NowTime : TIME;
    PreviousTime : TIME;
END_VAR
NowTime := TIME();
ClockValid := NOT ClockPrimed OR ((NowTime - PreviousTime) >= T#90ms AND (NowTime - PreviousTime) <= T#110ms);
IF NOT ClockValid OR FixedC < LREAL#0.0 OR FixedC > LREAL#80.0 OR AdaptiveC < LREAL#0.0 OR AdaptiveC > LREAL#80.0 THEN
    FaultLatched := TRUE;
END_IF;
IF ExperimentEnabled AND NOT WasEnabled THEN
    Updates := 0;
    AdaptiveKi := LREAL#0.5;
    Accepted1 := FALSE;
    Accepted2 := FALSE;
ELSIF ExperimentEnabled THEN
    Updates := Updates + 1;
END_IF;
(* The windows: B in Manual at 60 points, then 70, then 60 again until 90 s. Every whole second the program samples B's
   temperature and the command it received during the second before, delivered over the second that starts now. *)
SetpointC := LREAL#35.0;
ManualPct := LREAL#60.0;
IF ExperimentEnabled AND Updates >= 300 AND Updates < 600 THEN
    ManualPct := LREAL#70.0;
END_IF;
IF ExperimentEnabled AND Updates >= 900 THEN
    SetpointC := LREAL#38.0;
END_IF;
IF ExperimentEnabled AND Updates <= 600 AND Updates MOD 10 = 0 THEN
    Second := (Updates / 10) MOD 30;
    IF Updates > 0 AND Second = 0 THEN
        Temperatures[30] := AdaptiveC;
        Window(Temperatures := Temperatures, Delivered := Delivered);
        IF Updates = 300 THEN
            Estimate1 := Window.EstimateKwPerC;
            Span1 := Window.SpanC;
            Accepted1 := Window.Accepted;
        ELSE
            Estimate2 := Window.EstimateKwPerC;
            Span2 := Window.SpanC;
            Accepted2 := Window.Accepted;
        END_IF;
        IF Window.Accepted THEN
            AdaptiveKi := Window.EstimateKwPerC / LREAL#20.0;
        END_IF;
    END_IF;
    Temperatures[Second] := AdaptiveC;
    Delivered[Second] := PreviousSecondPct;
    PreviousSecondPct := ManualPct;
END_IF;
(* Both heaters start from rest: setpoint 35 °C, 35 °C and 60 %. B's integral gain is the adapted one; a configuration change
   keeps the controller's state, so the next update continues from it. *)
IF NOT Initialized THEN
    (* AdaptiveKi is bound to %ML40, which reads 0 in the first scan; B starts with the designed 0.5. *)
    AdaptiveKi := LREAL#0.5;
    State.Setpoint := LREAL#35.0;
    State.ProcessValue := LREAL#35.0;
    State.NominalOutput := LREAL#60.0;
    State.SaturatedOutput := LREAL#60.0;
    FixedLoop.Configure(Parameters := FixedParameters);
    FixedLoop.Initialize(State := State);
    AdaptiveLoop.Configure(Parameters := AdaptiveParameters);
    AdaptiveLoop.Initialize(State := State);
    Initialized := TRUE;
END_IF;
AdaptiveParameters.IntegralGain := AdaptiveKi;
AdaptiveLoop.Configure(Parameters := AdaptiveParameters);
FixedInput.Mode := PIDMode#Auto;
FixedInput.CycleTime := LREAL#0.1;
FixedInput.Setpoint := SetpointC;
FixedInput.ProcessValue := FixedC;
FixedPct := FixedLoop.Update(Input := FixedInput);
(* B holds the window commands in Manual; Manual to Auto continues from the held command. *)
IF ExperimentEnabled AND Updates < 900 THEN
    AdaptiveInput.Mode := PIDMode#Manual;
ELSE
    AdaptiveInput.Mode := PIDMode#Auto;
END_IF;
AdaptiveInput.ManualOutput := ManualPct;
AdaptiveInput.CycleTime := LREAL#0.1;
AdaptiveInput.Setpoint := SetpointC;
AdaptiveInput.ProcessValue := AdaptiveC;
AdaptivePct := AdaptiveLoop.Update(Input := AdaptiveInput);
IF FaultLatched THEN
    FixedPct := LREAL#0.0;
    AdaptivePct := LREAL#0.0;
END_IF;
WasEnabled := ExperimentEnabled;
PreviousTime := NowTime;
ClockPrimed := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.FixedC AT %IL0 : LREAL;
    P1.AdaptiveC AT %IL8 : LREAL;
    P1.FixedPct AT %QL0 : LREAL;
    P1.FixedLoadKw AT %QL8 : LREAL;
    P1.AdaptivePct AT %QL16 : LREAL;
    P1.AdaptiveLoadKw AT %QL24 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.Estimate1 AT %ML8 : LREAL;
    P1.Estimate2 AT %ML16 : LREAL;
    P1.Span1 AT %ML24 : LREAL;
    P1.Span2 AT %ML32 : LREAL;
    P1.AdaptiveKi AT %ML40 : LREAL;
    P1.ExperimentEnabled AT %MX56.0 : BOOL;
    P1.Accepted1 AT %MX56.1 : BOOL;
    P1.Accepted2 AT %MX56.2 : BOOL;
    P1.FaultLatched AT %MX56.3 : BOOL;
    P1.ClockValid AT %MX56.4 : BOOL;
END_VAR
END_CONFIGURATION

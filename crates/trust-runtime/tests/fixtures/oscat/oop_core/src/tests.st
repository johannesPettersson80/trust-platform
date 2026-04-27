TEST_PROGRAM oscat_oop_context_loads_constants_and_direction_helpers_match_classic
VAR
    Context : AutomationContext;
    ContextObject : IAutomationContext;
    Status : ComponentStatus;
    DirectionName : STRING[3];
    DirectionDegrees : INT;
END_VAR

ContextObject := Context;
ContextObject.Initialize();

Status := ContextObject.Snapshot();
ASSERT_TRUE(IN := Status.Ready);
ASSERT_FALSE(IN := Status.Error);
ASSERT_TRUE(IN := ContextObject.ConstantsLoaded);

ASSERT_TRUE(IN := ContextObject.LoadConstants());
Status := ContextObject.Snapshot();
DirectionName := ContextObject.DirectionName(Degrees := REAL#90.0);
DirectionDegrees := ContextObject.DirectionDegrees(Name := 'E');

ASSERT_TRUE(IN := Status.Ready);
ASSERT_FALSE(IN := Status.Error);
ASSERT_TRUE(IN := ContextObject.ConstantsLoaded);
ASSERT_EQUAL(EXPECTED := OSCAT_VERSION(IN := FALSE), ACTUAL := ContextObject.Version);
ASSERT_NEAR(EXPECTED := MATH.PI2, ACTUAL := ContextObject.Pi2, DELTA := REAL#0.0001);
ASSERT_EQUAL(EXPECTED := DEG_TO_DIR(DEG := INT#90, N := INT#4, L := INT#1), ACTUAL := DirectionName);
ASSERT_EQUAL(EXPECTED := DIR_TO_DEG(DIR := 'E', L := INT#1), ACTUAL := DirectionDegrees);
END_TEST_PROGRAM

TEST_PROGRAM oscat_oop_unit_converter_matches_classic_records_and_scalars
VAR
    Converter : UnitConverter;
    ConverterObject : IUnitConverter;
    ConcreteCelsius : REAL;
    ClassicEnergy : ENERGY;
    ClassicCalories : ENERGY;
END_VAR

ConverterObject := Converter;
ConverterObject.Initialize();
OSCAT_BASIC_Constants();

ASSERT_NEAR(EXPECTED := C_TO_K(Celsius := REAL#25.0), ACTUAL := ConverterObject.KelvinFromCelsius(Celsius := REAL#25.0), DELTA := REAL#0.01);
ConcreteCelsius := Converter.CelsiusFromKelvin(Kelvin := REAL#298.15);
ASSERT_NEAR(EXPECTED := REAL#25.0, ACTUAL := ConcreteCelsius, DELTA := REAL#0.01);

ASSERT_NEAR(EXPECTED := C_TO_F(celsius := REAL#25.0), ACTUAL := ConverterObject.FahrenheitFromCelsius(Celsius := REAL#25.0), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := K_TO_C(Kelvin := REAL#298.15), ACTUAL := ConverterObject.CelsiusFromKelvin(Kelvin := REAL#298.15), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := F_TO_C(fahrenheit := REAL#77.0), ACTUAL := ConverterObject.CelsiusFromFahrenheit(Fahrenheit := REAL#77.0), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := KMH_TO_MS(kmh := REAL#36.0), ACTUAL := ConverterObject.MetersPerSecondFromKilometersPerHour(Kmh := REAL#36.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := MS_TO_KMH(ms := REAL#10.0), ACTUAL := ConverterObject.KilometersPerHourFromMetersPerSecond(Mps := REAL#10.0), DELTA := REAL#0.001);
ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#10.0), ACTUAL := ConverterObject.BeaufortFromMetersPerSecond(Mps := REAL#10.0));

ClassicEnergy(J := REAL#3600.0, C := REAL#0.0, Wh := REAL#0.0);
ASSERT_NEAR(EXPECTED := ClassicEnergy.YWh, ACTUAL := ConverterObject.WattHoursFromJoules(Joule := REAL#3600.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := ClassicEnergy.YC, ACTUAL := ConverterObject.CaloriesFromJoules(Joule := REAL#3600.0), DELTA := REAL#0.001);
ClassicEnergy(J := REAL#0.0, C := REAL#0.0, Wh := REAL#1.0);
ASSERT_NEAR(EXPECTED := ClassicEnergy.YJ, ACTUAL := ConverterObject.JoulesFromWattHours(WattHour := REAL#1.0), DELTA := REAL#0.001);
ClassicCalories(J := REAL#0.0, C := REAL#1.0, Wh := REAL#0.0);
ASSERT_NEAR(EXPECTED := ClassicCalories.YJ, ACTUAL := ConverterObject.JoulesFromCalories(Calorie := REAL#1.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := F_TO_OM(F := REAL#50.0), ACTUAL := ConverterObject.AngularFrequencyFromHertz(Frequency := REAL#50.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := PT_TO_F(PT := T#20ms), ACTUAL := ConverterObject.HertzFromPeriod(Period := T#20ms), DELTA := REAL#0.001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_oop_filter_pid_hysteresis_and_pulse_match_classic_scan_behaviour
VAR
    Filter : Pt1Filter;
    FilterObject : IPt1Filter;
    ClassicFilter : FT_PT1;
    FilterOutput : REAL;
    Limits : RealRange;
    Gains : PidGains;
    Controller : PidController;
    ControllerObject : IPidController;
    ClassicController : CTRL_PID;
    ControllerOutput : REAL;
    Switch : HysteresisSwitch;
    SwitchObject : IHysteresisSwitch;
    ClassicSwitch : HYST;
    FreshSwitch : HYST;
    SwitchOutput : BOOL;
    Pulse : PulseGenerator;
    PulseObject : IPulseGenerator;
    ClassicPulse : GEN_PULSE;
    PulseOutput : BOOL;
END_VAR

FilterObject := Filter;
FilterObject.Initialize();
FilterObject.Configure(TimeConstant := T#1s, Gain := REAL#2.0);
ClassicFilter(in := REAL#10.0, T := T#1s, K := REAL#2.0);
FilterOutput := FilterObject.Update(Sample := REAL#10.0);
ASSERT_NEAR(EXPECTED := ClassicFilter.out, ACTUAL := FilterOutput, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := ClassicFilter.out, ACTUAL := FilterObject.Output, DELTA := REAL#0.001);

Limits.Low := REAL#0.0;
Limits.High := REAL#10.0;
Gains.Kp := REAL#2.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
ControllerObject := Controller;
ControllerObject.Initialize();
ControllerObject.Configure(
    Gains := Gains,
    Limits := Limits,
    SupervisionBand := REAL#0.0,
    Offset := REAL#1.0
);
ControllerObject.SetManual(Manual := TRUE, ManualInput := REAL#50.0);
ClassicController(
    ACT := REAL#0.0,
    SET_POINT := REAL#10.0,
    SUP := REAL#0.0,
    OFS := REAL#1.0,
    M_I := REAL#50.0,
    MAN := TRUE,
    RST := FALSE,
    KP := REAL#2.0,
    TN := REAL#1.0,
    TV := REAL#0.0,
    LL := REAL#0.0,
    LH := REAL#10.0
);
ControllerOutput := ControllerObject.Update(Actual := REAL#0.0, Target := REAL#10.0);
ASSERT_NEAR(EXPECTED := ClassicController.Y, ACTUAL := ControllerOutput, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := ClassicController.DIFF, ACTUAL := ControllerObject.ControlError, DELTA := REAL#0.001);
ASSERT_TRUE(IN := ControllerObject.Limited);

SwitchObject := Switch;
SwitchObject.Initialize();
SwitchObject.SetLimits(Limits := Limits);
ClassicSwitch(In := REAL#12.0, HIGH_LIMIT := REAL#10.0, LOW_LIMIT := REAL#0.0);
SwitchOutput := SwitchObject.Update(MeasuredValue := REAL#12.0);
ASSERT_TRUE(IN := SwitchOutput);
ASSERT_TRUE(IN := SwitchObject.Q = ClassicSwitch.Q);
ASSERT_TRUE(IN := SwitchObject.Window = ClassicSwitch.WIN);

PulseObject := Pulse;
PulseObject.Initialize();
PulseObject.Configure(HighTime := T#1s, LowTime := T#1s);
PulseObject.SetEnabled(Enabled := FALSE);
ClassicPulse(ENQ := FALSE, PTH := T#1s, PTL := T#1s);
PulseOutput := PulseObject.Update();
ASSERT_FALSE(IN := PulseOutput);
ASSERT_TRUE(IN := PulseObject.Output = ClassicPulse.Q);
END_TEST_PROGRAM

TEST_PROGRAM oscat_oop_reset_validation_and_multiscan_behaviour_are_locked
VAR
    I : INT;
    Limits : RealRange;
    InvalidLimits : RealRange;
    Gains : PidGains;
    Filter : Pt1Filter;
    FilterObject : IPt1Filter;
    ClassicFilter : FT_PT1;
    FilterOutput : REAL;
    ResetFilterOutput : REAL;
    Switch : HysteresisSwitch;
    SwitchObject : IHysteresisSwitch;
    ClassicSwitch : HYST;
    FreshSwitch : HYST;
    SwitchOutput : BOOL;
    Controller : PidController;
    ControllerObject : IPidController;
    Status : ComponentStatus;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#10.0;
InvalidLimits.Low := REAL#10.0;
InvalidLimits.High := REAL#0.0;
Gains.Kp := REAL#2.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;

FilterObject := Filter;
FilterObject.Initialize();
FilterObject.Configure(TimeConstant := T#1s, Gain := REAL#2.0);
ClassicFilter(in := REAL#10.0, T := T#1s, K := REAL#2.0);
FilterOutput := FilterObject.Update(Sample := REAL#10.0);
FOR I := 1 TO 10 DO
    ClassicFilter(in := REAL#20.0, T := T#1s, K := REAL#2.0);
    FilterOutput := FilterObject.Update(Sample := REAL#20.0);
    ASSERT_NEAR(EXPECTED := ClassicFilter.out, ACTUAL := FilterOutput, DELTA := REAL#0.5);
END_FOR;
FilterObject.Reset();
ClassicFilter(in := REAL#5.0, T := T#0s, K := REAL#2.0);
ResetFilterOutput := FilterObject.Update(Sample := REAL#5.0);
ASSERT_NEAR(EXPECTED := ClassicFilter.out, ACTUAL := ResetFilterOutput, DELTA := REAL#0.001);

SwitchObject := Switch;
SwitchObject.Initialize();
SwitchObject.SetLimits(Limits := Limits);
ClassicSwitch(In := REAL#12.0, HIGH_LIMIT := REAL#10.0, LOW_LIMIT := REAL#0.0);
SwitchOutput := SwitchObject.Update(MeasuredValue := REAL#12.0);
ASSERT_TRUE(IN := SwitchOutput);
SwitchObject.Reset();
FreshSwitch(In := REAL#5.0, HIGH_LIMIT := REAL#10.0, LOW_LIMIT := REAL#0.0);
SwitchOutput := SwitchObject.Update(MeasuredValue := REAL#5.0);
ASSERT_TRUE(IN := SwitchObject.Q = FreshSwitch.Q);
ASSERT_TRUE(IN := SwitchObject.Window = FreshSwitch.WIN);

SwitchObject.SetLimits(Limits := InvalidLimits);
Status := SwitchObject.Snapshot();
ASSERT_TRUE(IN := Status.Error);
ASSERT_EQUAL(EXPECTED := ComponentErrorInvalidConfiguration, ACTUAL := Status.ErrorId);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := SwitchObject.LowLimit, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := SwitchObject.HighLimit, DELTA := REAL#0.001);

ControllerObject := Controller;
ControllerObject.Initialize();
ControllerObject.Configure(
    Gains := Gains,
    Limits := Limits,
    SupervisionBand := REAL#0.0,
    Offset := REAL#0.0
);
ControllerObject.SetLimits(Limits := InvalidLimits);
Status := ControllerObject.Snapshot();
ASSERT_TRUE(IN := Status.Error);
ASSERT_EQUAL(EXPECTED := ComponentErrorInvalidConfiguration, ACTUAL := Status.ErrorId);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := ControllerObject.LowLimit, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := ControllerObject.HighLimit, DELTA := REAL#0.001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_oop_fifo_and_stack_match_classic_memory_modules
VAR
    Queue : DwordFifo16;
    QueueObject : IDwordQueue;
    ClassicQueue : FIFO_16;
    Stack : DwordStack16;
    StackObject : IDwordStack;
    ClassicStack : STACK_16;
END_VAR

QueueObject := Queue;
QueueObject.Initialize();
QueueObject.Reset();
ClassicQueue(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
ASSERT_TRUE(IN := QueueObject.Empty = ClassicQueue.EMPTY);
ASSERT_TRUE(IN := QueueObject.Capacity = MaxDwordFifo16Capacity);

ASSERT_TRUE(IN := QueueObject.Push(Value := DWORD#16#12345678));
ClassicQueue(DIN := DWORD#16#12345678, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
ASSERT_TRUE(IN := QueueObject.Full = ClassicQueue.FULL);
ASSERT_TRUE(IN := QueueObject.Empty = ClassicQueue.EMPTY);
ASSERT_TRUE(IN := QueueObject.Push(Value := DWORD#16#87654321));
ClassicQueue(DIN := DWORD#16#87654321, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
ASSERT_TRUE(IN := QueueObject.TryPop());
ClassicQueue(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := ClassicQueue.DOUT, ACTUAL := QueueObject.Value);
ASSERT_TRUE(IN := QueueObject.TryPop());
ClassicQueue(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := ClassicQueue.DOUT, ACTUAL := QueueObject.Value);
ASSERT_TRUE(IN := QueueObject.Empty = ClassicQueue.EMPTY);

StackObject := Stack;
StackObject.Initialize();
StackObject.Reset();
ClassicStack(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
ASSERT_TRUE(IN := StackObject.Empty = ClassicStack.EMPTY);
ASSERT_TRUE(IN := StackObject.Capacity = MaxDwordStack16Capacity);
ASSERT_TRUE(IN := StackObject.Push(Value := DWORD#16#11111111));
ASSERT_TRUE(IN := StackObject.Push(Value := DWORD#16#22222222));
ClassicStack(DIN := DWORD#16#11111111, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
ClassicStack(DIN := DWORD#16#22222222, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
ASSERT_TRUE(IN := StackObject.TryPop());
ClassicStack(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := ClassicStack.DOUT, ACTUAL := StackObject.Value);
END_TEST_PROGRAM

TEST_PROGRAM oscat_oop_calendar_matches_classic_calendar_and_sun_helpers
VAR
    Clock : CalendarClock;
    ClockObject : ICalendarClock;
    ClassicCalendar : CALENDAR;
    ClassicHolidays : ARRAY[0..29] OF HOLIDAY_DATA;
    ClassicCalculator : CALENDAR_CALC;
    ClassicPosition : SUN_POS;
    ClassicTimes : SUN_TIME;
    Position : SunPosition;
    Times : SunTimes;
END_VAR

ClockObject := Clock;
ClockObject.Initialize();
ClockObject.Configure(
    LocationLatitude := REAL#59.3293,
    LocationLongitude := REAL#18.0686,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    LanguageIndex := INT#1
);
ClockObject.Update(Utc := DT#2026-04-26-10:00:00);

ClassicCalendar.UTC := DT#2026-04-26-10:00:00;
ClassicCalendar.DST_EN := TRUE;
ClassicCalendar.OFFSET := INT#60;
ClassicCalendar.LANGUAGE := INT#1;
ClassicCalendar.LATITUDE := REAL#59.3293;
ClassicCalendar.LONGITUDE := REAL#18.0686;
ClassicCalculator(SPE := TRUE, H := REAL#-0.8333333, XCAL := ClassicCalendar, HOLIDAYS := ClassicHolidays);

ASSERT_EQUAL(EXPECTED := ClassicCalendar.LOCAL_DT, ACTUAL := ClockObject.LocalDateTime);
ASSERT_EQUAL(EXPECTED := ClassicCalendar.YEAR, ACTUAL := ClockObject.Year);
ASSERT_EQUAL(EXPECTED := ClassicCalendar.MONTH, ACTUAL := ClockObject.Month);
ASSERT_EQUAL(EXPECTED := ClassicCalendar.DAY, ACTUAL := ClockObject.Day);
ASSERT_EQUAL(EXPECTED := ClassicCalendar.WORK_WEEK, ACTUAL := ClockObject.WorkWeek);
ASSERT_NEAR(EXPECTED := ClassicCalendar.SUN_VER, ACTUAL := ClockObject.SolarVerticalProjection, DELTA := REAL#0.01);

ClassicPosition(LATITUDE := REAL#59.3293, LONGITUDE := REAL#18.0686, UTC := DT#2026-04-26-10:00:00);
Position := ClockObject.CalculateSunPosition(
    Utc := DT#2026-04-26-10:00:00,
    Latitude := REAL#59.3293,
    Longitude := REAL#18.0686
);
ASSERT_NEAR(EXPECTED := ClassicPosition.B, ACTUAL := Position.Azimuth, DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := ClassicPosition.HR, ACTUAL := Position.RefractedElevation, DELTA := REAL#0.01);

ClassicTimes(LATITUDE := REAL#59.3293, LONGITUDE := REAL#18.0686, UTC := DATE#2026-04-26, H := REAL#-0.8333333);
Times := ClockObject.CalculateSunTime(
    UtcDate := DATE#2026-04-26,
    Latitude := REAL#59.3293,
    Longitude := REAL#18.0686,
    Horizon := REAL#-0.8333333
);
ASSERT_EQUAL(EXPECTED := ClassicTimes.MIDDAY, ACTUAL := Times.SolarNoon);
ASSERT_EQUAL(EXPECTED := ClassicTimes.SUN_RISE, ACTUAL := Times.Sunrise);
ASSERT_EQUAL(EXPECTED := ClassicTimes.SUN_SET, ACTUAL := Times.Sunset);
END_TEST_PROGRAM

TEST_PROGRAM oscat_oop_instances_do_not_share_scan_state
VAR
    FastFilter : Pt1Filter;
    SlowFilter : Pt1Filter;
    FastObject : IPt1Filter;
    SlowObject : IPt1Filter;
    FastOutput : REAL;
    SlowOutput : REAL;
END_VAR

FastObject := FastFilter;
SlowObject := SlowFilter;
FastObject.Initialize();
SlowObject.Initialize();
FastObject.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
SlowObject.Configure(TimeConstant := T#1s, Gain := REAL#2.0);

FastOutput := FastObject.Update(Sample := REAL#10.0);
SlowOutput := SlowObject.Update(Sample := REAL#10.0);

ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := FastOutput, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#20.0, ACTUAL := SlowOutput, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := FastOutput, ACTUAL := FastObject.Output, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := SlowOutput, ACTUAL := SlowObject.Output, DELTA := REAL#0.001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_oop_v1_control_and_filter_components_match_classic
VAR
    Limits : RealRange;
    PiGainsValue : PiGains;
    LoopObject : IClosedLoopController;
    Pi : PiController;
    ClassicPi : CTRL_PI;
    Pid : PidController;
    Pwm : PwmController;
    ClassicPwm : CTRL_PWM;
    Derivative : DerivativeFilter;
    ClassicDerivative : FT_DERIV;
    Integral : IntegratorFilter;
    ClassicIntegral : FT_INT;
    Pt2 : Pt2Filter;
    ClassicPt2 : FT_PT2;
    Delay : DelayLine16;
    ClassicDelay : FT_TN16;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
PiGainsValue.Kp := REAL#2.0;
PiGainsValue.Ki := REAL#1.0;

Pi.Initialize();
Pi.Configure(Gains := PiGainsValue, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#1.0);
Pi.SetManual(Manual := TRUE, ManualInput := REAL#40.0);
ClassicPi(
    ACT := REAL#10.0,
    SET_POINT := REAL#20.0,
    SUP := REAL#0.0,
    OFS := REAL#1.0,
    M_I := REAL#40.0,
    MAN := TRUE,
    RST := FALSE,
    KP := REAL#2.0,
    KI := REAL#1.0,
    LL := REAL#0.0,
    LH := REAL#100.0
);
ASSERT_NEAR(EXPECTED := ClassicPi.Y, ACTUAL := Pi.Update(Actual := REAL#10.0, Target := REAL#20.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := ClassicPi.DIFF, ACTUAL := Pi.ControlError, DELTA := REAL#0.001);

LoopObject := Pi;
ASSERT_NEAR(EXPECTED := ClassicPi.Y, ACTUAL := LoopObject.Update(Actual := REAL#10.0, Target := REAL#20.0), DELTA := REAL#0.001);
Pid.Initialize();
Pid.SetManual(Manual := TRUE, ManualInput := REAL#10.0);
LoopObject := Pid;
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := LoopObject.Update(Actual := REAL#0.0, Target := REAL#10.0), DELTA := REAL#0.001);

Pwm.Initialize();
Pwm.Configure(Frequency := REAL#0.0);
ClassicPwm(CI := REAL#0.5, MAN_IN := REAL#0.0, MANUAL := FALSE, F := REAL#0.0);
ASSERT_TRUE(IN := Pwm.Update(ControlInput := REAL#0.5) = ClassicPwm.Q);

Derivative.Initialize();
Derivative.Configure(Gain := REAL#3.0);
ClassicDerivative(in := REAL#4.0, K := REAL#3.0, run := TRUE);
ASSERT_NEAR(EXPECTED := ClassicDerivative.out, ACTUAL := Derivative.Update(Sample := REAL#4.0), DELTA := REAL#0.001);

Integral.Initialize();
Integral.Configure(Gain := REAL#1.0, Limits := Limits);
Integral.Reset();
ClassicIntegral(IN := REAL#2.0, K := REAL#1.0, RUN := TRUE, RST := TRUE, OUT_MIN := REAL#0.0, OUT_MAX := REAL#100.0);
ASSERT_NEAR(EXPECTED := ClassicIntegral.OUT, ACTUAL := Integral.Update(Sample := REAL#2.0, Run := TRUE), DELTA := REAL#0.001);

Pt2.Initialize();
Pt2.Configure(TimeConstant := T#1s, Damping := REAL#1.0, Gain := REAL#2.0);
ClassicPt2(in := REAL#5.0, T := T#1s, D := REAL#1.0, K := REAL#2.0);
ASSERT_NEAR(EXPECTED := ClassicPt2.out, ACTUAL := Pt2.Update(Sample := REAL#5.0), DELTA := REAL#0.001);

Delay.Initialize();
Delay.Configure(DelayTime := T#16s);
ClassicDelay(in := REAL#7.0, T := T#16s);
ASSERT_NEAR(EXPECTED := ClassicDelay.out, ACTUAL := Delay.Update(Sample := REAL#7.0), DELTA := REAL#0.001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_oop_v1_generators_memory_and_logic_match_classic
VAR
    Fifo32 : DwordFifo32;
    ClassicFifo32 : FIFO_32;
    Stack32 : DwordStack32;
    ClassicStack32 : STACK_32;
    RandomSignal : RandomSignalGenerator;
    ClassicRandomSignal : GEN_RDM;
    SineSignal : SineSignalGenerator;
    ClassicSineSignal : GEN_SIN;
    SquareSignal : SquareSignalGenerator;
    ClassicSquareSignal : GEN_SQR;
    ByteRampValue : ByteRamp;
    ClassicByteRamp : RMP_B;
    WordRampValue : WordRamp;
    ClassicWordRamp : RMP_W;
    LatchValue : Latch;
    ClassicLatch : LTCH;
    ToggleValue : ToggleSwitch;
    ClassicToggle : TOGGLE;
    CounterValue : DwordCounter;
    ClassicCounter : COUNT_DR;
    ShiftValue : ShiftRegister8;
    ClassicShift : SHR_8PLE;
END_VAR

Fifo32.Initialize();
Fifo32.Reset();
ClassicFifo32(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
ASSERT_TRUE(IN := Fifo32.Empty = ClassicFifo32.EMPTY);
ASSERT_TRUE(IN := Fifo32.Capacity = MaxDwordFifo32Capacity);
ASSERT_TRUE(IN := Fifo32.Push(Value := DWORD#16#11112222));
ASSERT_TRUE(IN := Fifo32.Push(Value := DWORD#16#33334444));
ClassicFifo32(DIN := DWORD#16#11112222, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
ClassicFifo32(DIN := DWORD#16#33334444, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
ASSERT_TRUE(IN := Fifo32.TryPop());
ClassicFifo32(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := ClassicFifo32.DOUT, ACTUAL := Fifo32.Value);

Stack32.Initialize();
Stack32.Reset();
ClassicStack32(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
ASSERT_TRUE(IN := Stack32.Empty = ClassicStack32.EMPTY);
ASSERT_TRUE(IN := Stack32.Capacity = MaxDwordStack32Capacity);

RandomSignal.Initialize();
RandomSignal.Configure(Period := T#1s, Amplitude := REAL#2.0, Offset := REAL#1.0);
ClassicRandomSignal(PT := T#1s, AM := REAL#2.0, OS := REAL#1.0);
ASSERT_TRUE(IN := RandomSignal.Update() = ClassicRandomSignal.Q);

SineSignal.Initialize();
SineSignal.Configure(Period := T#1s, Amplitude := REAL#2.0, Offset := REAL#1.0, Delay := REAL#0.0);
ClassicSineSignal(PT := T#1s, AM := REAL#2.0, OS := REAL#1.0, DL := REAL#0.0);
ASSERT_NEAR(EXPECTED := ClassicSineSignal.OUT, ACTUAL := SineSignal.Update(), DELTA := REAL#0.001);

SquareSignal.Initialize();
SquareSignal.Configure(Period := T#1s, Amplitude := REAL#2.0, Offset := REAL#1.0, DutyCycle := REAL#1.0, Delay := REAL#0.0);
ClassicSquareSignal(PT := T#1s, AM := REAL#2.0, OS := REAL#1.0, DC := REAL#1.0, DL := REAL#0.0);
ASSERT_NEAR(EXPECTED := ClassicSquareSignal.OUT, ACTUAL := SquareSignal.Update(), DELTA := REAL#0.001);

ByteRampValue.Initialize();
ByteRampValue.Update(SetHigh := TRUE, RampTime := T#1s, Enabled := TRUE, Up := TRUE);
ClassicByteRamp(SET_HIGH := TRUE, PT := T#1s, E := TRUE, UP := TRUE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := ClassicByteRamp.OUT, ACTUAL := ByteRampValue.Output);

WordRampValue.Initialize();
WordRampValue.Update(SetHigh := TRUE, RampTime := T#1s, Enabled := TRUE, Up := TRUE);
ClassicWordRamp(SET_HIGH := TRUE, PT := T#1s, E := TRUE, UP := TRUE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := ClassicWordRamp.OUT, ACTUAL := WordRampValue.Output);

LatchValue.Initialize();
ClassicLatch(DATA := TRUE, L := TRUE, RST := FALSE);
ASSERT_TRUE(IN := LatchValue.Update(Data := TRUE, Load := TRUE) = ClassicLatch.Q);

ToggleValue.Initialize();
ClassicToggle(CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := ToggleValue.Update(Clock := TRUE) = ClassicToggle.Q);

CounterValue.Initialize();
CounterValue.Configure(StepSize := DWORD#2, MaxValue := DWORD#10);
ClassicCounter(SetIn := FALSE, IN := DWORD#0, UP := TRUE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#10, RST := FALSE);
ASSERT_EQUAL(EXPECTED := ClassicCounter.CNT, ACTUAL := CounterValue.Update(Up := TRUE, Down := FALSE));

ShiftValue.Initialize();
ClassicShift(Din := TRUE, Dload := BYTE#0, CLK := TRUE, UP := TRUE, LoadIn := FALSE, RST := FALSE);
ASSERT_TRUE(IN := ShiftValue.Update(DataIn := TRUE, LoadValue := BYTE#0, Clock := TRUE, Up := TRUE, Load := FALSE) = ClassicShift.DOut);
END_TEST_PROGRAM

TEST_PROGRAM oscat_oop_v1_measuring_calendar_building_and_driver_components_match_classic
VAR
    Ontime : OntimeMeter;
    ClassicOntime : ONTIME;
    ClassicSeconds : UDINT;
    ClassicCycles : UDINT;
    Cycle : CycleTimeMeter;
    ClassicCycle : CYCLE_TIME;
    CalibratorValue : Calibrator;
    ClassicCalibrator : CALIBRATE;
    Bar : BarGraphMeter;
    ClassicBar : BAR_GRAPH;
    Rtc : RtcClock;
    ClassicRtc : RTC_2;
    HolidayValue : HolidayCalendar;
    EmptyHolidays : ARRAY[0..29] OF HOLIDAY_DATA;
    ClassicHoliday : HOLIDAY;
    Tank : TankLevelController;
    ClassicTank : TANK_LEVEL;
    Heat : HeatCurve;
    ClassicHeat : HEAT_TEMP;
    BoilerValue : BoilerController;
    ClassicBoiler : BOILER;
    DriverValue : SingleOutputDriver;
    ClassicDriver : DRIVER_1;
END_VAR

Ontime.Initialize();
Ontime.Reset();
ClassicOntime(IN := FALSE, RST := TRUE, SECONDS := ClassicSeconds, CYCLES := ClassicCycles);
ASSERT_EQUAL(EXPECTED := UDINT#0, ACTUAL := Ontime.Seconds);
ASSERT_EQUAL(EXPECTED := UDINT#0, ACTUAL := Ontime.Cycles);

Cycle.Initialize();
ClassicCycle(RST := FALSE);
Cycle.Update(ResetStatistics := FALSE);
ASSERT_EQUAL(EXPECTED := ClassicCycle.cycles, ACTUAL := Cycle.Cycles);

CalibratorValue.Initialize();
CalibratorValue.Configure(OffsetTarget := REAL#10.0, ScaleTarget := REAL#2.0);
ClassicCalibrator(X := REAL#8.0, CO := TRUE, CS := FALSE, Y_Offset := REAL#10.0, Y_Scale := REAL#2.0);
ASSERT_NEAR(EXPECTED := ClassicCalibrator.Y, ACTUAL := CalibratorValue.Update(Sample := REAL#8.0, CalibrateOffset := TRUE, CalibrateScale := FALSE), DELTA := REAL#0.001);

Bar.Initialize();
Bar.Configure(TriggerLow := REAL#10.0, TriggerHigh := REAL#20.0, AlarmLow := TRUE, AlarmHigh := TRUE, LogScale := FALSE);
ClassicBar(X := REAL#5.0, rst := FALSE, trigger_Low := REAL#10.0, trigger_High := REAL#20.0, Alarm_low := TRUE, Alarm_high := TRUE, log_scale := FALSE);
Bar.Update(Sample := REAL#5.0, ResetAlarm := FALSE);
ASSERT_TRUE(IN := Bar.Low = ClassicBar.LOW);
ASSERT_TRUE(IN := Bar.Alarm = ClassicBar.Alarm);
ASSERT_EQUAL(EXPECTED := ClassicBar.Status, ACTUAL := Bar.Status);

Rtc.Initialize();
Rtc.SetClock(InitialDateTime := DT#2026-04-26-10:00:00, Milliseconds := INT#0, DstEnabled := TRUE, OffsetMinutes := INT#60);
ClassicRtc(SET_RTC := TRUE, SDT := DT#2026-04-26-10:00:00, SMS := INT#0, DEN := TRUE, OFS := INT#60);
ASSERT_EQUAL(EXPECTED := ClassicRtc.UDT, ACTUAL := Rtc.UtcDateTime);
ASSERT_EQUAL(EXPECTED := ClassicRtc.LOCAL_DT, ACTUAL := Rtc.LocalDateTime);

HolidayValue.Initialize();
HolidayValue.Configure(LanguageIndex := INT#1, FridayAsHoliday := FALSE, SaturdayAsHoliday := FALSE, SundayAsHoliday := TRUE);
ClassicHoliday(DATE_IN := DATE#2026-04-26, LANGU := INT#1, FRIDAY := FALSE, SATURDAY := FALSE, SUNDAY := TRUE, HOLIDAYS := EmptyHolidays);
ASSERT_TRUE(IN := HolidayValue.Update(DateIn := DATE#2026-04-26) = ClassicHoliday.Y);
ASSERT_EQUAL(EXPECTED := ClassicHoliday.NAME, ACTUAL := HolidayValue.Name);

Tank.Initialize();
Tank.Configure(MaxValveTime := T#0s, LevelDelayTime := T#0s);
ClassicTank(LEVEL := FALSE, LEAK := FALSE, ACLR := FALSE, MAX_VALVE_TIME := T#0s, LEVEL_DELAY_TIME := T#0s);
ASSERT_TRUE(IN := Tank.Update(LevelReached := FALSE, LeakDetected := FALSE, AlarmClear := FALSE) = ClassicTank.VALVE);
ASSERT_EQUAL(EXPECTED := ClassicTank.STATUS, ACTUAL := Tank.Status);

Heat.Initialize();
Heat.Configure(
    MaxFlowTemperature := REAL#70.0,
    MinFlowTemperature := REAL#25.0,
    ConfigFlowTemperature := REAL#70.0,
    ConfigInsideTemperature := REAL#20.0,
    ConfigOutsideTemperature := REAL#-15.0,
    FlowDifference := REAL#10.0,
    Curve := REAL#1.33,
    Hysteresis := REAL#3.0
);
ClassicHeat(
    T_EXT := REAL#0.0,
    T_INT := REAL#20.0,
    OFFSET := REAL#0.0,
    T_REQ := REAL#0.0,
    TY_MAX := REAL#70.0,
    TY_MIN := REAL#25.0,
    TY_CONFIG := REAL#70.0,
    T_INT_CONFIG := REAL#20.0,
    T_EXT_CONFIG := REAL#-15.0,
    T_DIFF := REAL#10.0,
    C := REAL#1.33,
    H := REAL#3.0
);
ASSERT_NEAR(EXPECTED := ClassicHeat.TY, ACTUAL := Heat.Update(OutsideTemperature := REAL#0.0, InsideTemperature := REAL#20.0, Offset := REAL#0.0, RequestedTemperature := REAL#0.0), DELTA := REAL#0.001);

BoilerValue.Initialize();
ClassicBoiler(T_upper := REAL#40.0, T_lower := REAL#40.0, Pressure := TRUE, enable := TRUE, Req_1 := FALSE, Req_2 := FALSE, Boost := FALSE);
ASSERT_TRUE(IN := BoilerValue.Update(UpperTemperature := REAL#40.0, LowerTemperature := REAL#40.0, PressureOk := TRUE, Enabled := TRUE, Request1 := FALSE, Request2 := FALSE, Boost := FALSE) = ClassicBoiler.Heat);
ASSERT_EQUAL(EXPECTED := ClassicBoiler.status, ACTUAL := BoilerValue.Status);

DriverValue.Initialize();
DriverValue.Configure(ToggleMode := FALSE, Timeout := T#0s);
ClassicDriver(SET_REQ := TRUE, IN := FALSE, RST := FALSE, Toggle_Mode := FALSE, Timeout := T#0s);
ASSERT_TRUE(IN := DriverValue.Update(SetRequest := TRUE, Input := FALSE) = ClassicDriver.Q);
END_TEST_PROGRAM

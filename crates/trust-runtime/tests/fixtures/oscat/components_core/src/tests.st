TEST_PROGRAM oscat_components_context_loads_constants_and_direction_helpers_match_classic
VAR
    Context : AutomationContext;
    ContextObject : IAutomationContext;
    Status : ComponentStatus;
    DirectionName : STRING[3];
    DirectionDegrees : INT;
END_VAR

ContextObject := Context;
ContextObject.Initialize();

ASSERT_TRUE(IN := ContextObject.LoadConstants());
Status := ContextObject.Snapshot();
DirectionName := ContextObject.DirectionName(Degrees := REAL#90.0);
DirectionDegrees := ContextObject.DirectionDegrees(Name := 'E');

ASSERT_TRUE(IN := Status.Ready);
ASSERT_FALSE(IN := Status.Error);
ASSERT_TRUE(IN := ContextObject.ConstantsLoaded);
ASSERT_EQUAL(EXPECTED := DWORD#333, ACTUAL := ContextObject.Version);
ASSERT_NEAR(EXPECTED := MATH.PI2, ACTUAL := ContextObject.Pi2, DELTA := REAL#0.0001);
ASSERT_EQUAL(EXPECTED := DEG_TO_DIR(DEG := INT#90, N := INT#4, L := INT#1), ACTUAL := DirectionName);
ASSERT_EQUAL(EXPECTED := DIR_TO_DEG(DIR := 'E', L := INT#1), ACTUAL := DirectionDegrees);
END_TEST_PROGRAM

TEST_PROGRAM oscat_components_unit_converter_matches_classic_records_and_scalars
VAR
    Converter : UnitConverter;
    ConverterObject : IUnitConverter;
    ConcreteCelsius : REAL;
    ClassicEnergy : ENERGY;
END_VAR

ConverterObject := Converter;
ConverterObject.Initialize();
ConcreteCelsius := Converter.CelsiusFromKelvin(Kelvin := REAL#298.15);
ASSERT_NEAR(EXPECTED := REAL#25.0, ACTUAL := ConcreteCelsius, DELTA := REAL#0.01);

ASSERT_NEAR(EXPECTED := C_TO_K(Celsius := REAL#25.0), ACTUAL := ConverterObject.KelvinFromCelsius(Celsius := REAL#25.0), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := C_TO_F(celsius := REAL#25.0), ACTUAL := ConverterObject.FahrenheitFromCelsius(Celsius := REAL#25.0), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := K_TO_C(Kelvin := REAL#298.15), ACTUAL := ConverterObject.CelsiusFromKelvin(Kelvin := REAL#298.15), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := F_TO_C(fahrenheit := REAL#77.0), ACTUAL := ConverterObject.CelsiusFromFahrenheit(Fahrenheit := REAL#77.0), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := KMH_TO_MS(kmh := REAL#36.0), ACTUAL := ConverterObject.MetersPerSecondFromKilometersPerHour(Kmh := REAL#36.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := MS_TO_KMH(ms := REAL#10.0), ACTUAL := ConverterObject.KilometersPerHourFromMetersPerSecond(Mps := REAL#10.0), DELTA := REAL#0.001);
ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#10.0), ACTUAL := ConverterObject.BeaufortFromMetersPerSecond(Mps := REAL#10.0));

ClassicEnergy(J := REAL#3600.0, C := REAL#0.0, Wh := REAL#0.0);
ASSERT_NEAR(EXPECTED := ClassicEnergy.YWh, ACTUAL := ConverterObject.WattHoursFromJoules(Joule := REAL#3600.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := ClassicEnergy.YJ, ACTUAL := ConverterObject.JoulesFromWattHours(WattHour := REAL#1.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#4.1868, ACTUAL := ConverterObject.JoulesFromCalories(Calorie := REAL#1.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := ClassicEnergy.YC, ACTUAL := ConverterObject.CaloriesFromJoules(Joule := REAL#3600.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := F_TO_OM(F := REAL#50.0), ACTUAL := ConverterObject.AngularFrequencyFromHertz(Frequency := REAL#50.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := PT_TO_F(PT := T#20ms), ACTUAL := ConverterObject.HertzFromPeriod(Period := T#20ms), DELTA := REAL#0.001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_components_filter_pid_hysteresis_and_pulse_match_classic_scan_behaviour
VAR
    Filter : Pt1Filter;
    FilterObject : IPt1Filter;
    ClassicFilter : FT_PT1;
    FilterOutput : REAL;
    Limits : RealRange;
    Controller : PidController;
    ControllerObject : IPidController;
    ClassicController : CTRL_PID;
    ControllerOutput : REAL;
    Switch : HysteresisSwitch;
    SwitchObject : IHysteresisSwitch;
    ClassicSwitch : HYST;
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
ControllerObject := Controller;
ControllerObject.Initialize();
ControllerObject.SetKp(Kp := REAL#2.0);
ControllerObject.SetIntegralTime(Tn := REAL#1.0);
ControllerObject.SetDerivativeTime(Tv := REAL#0.0);
ControllerObject.SetNoiseBand(SupervisionBand := REAL#0.0);
ControllerObject.SetOffset(Offset := REAL#1.0);
ControllerObject.SetLimits(Limits := Limits);
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
ASSERT_NEAR(EXPECTED := ClassicController.DIFF, ACTUAL := ControllerObject.Difference, DELTA := REAL#0.001);
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

TEST_PROGRAM oscat_components_fifo_and_stack_match_classic_memory_modules
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

TEST_PROGRAM oscat_components_calendar_matches_classic_calendar_and_sun_helpers
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
    Language := INT#1
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
ASSERT_NEAR(EXPECTED := ClassicCalendar.SUN_VER, ACTUAL := ClockObject.SunVertical, DELTA := REAL#0.01);

ClassicPosition(LATITUDE := REAL#59.3293, LONGITUDE := REAL#18.0686, UTC := DT#2026-04-26-10:00:00);
Position := ClockObject.CalculateSunPosition(
    Utc := DT#2026-04-26-10:00:00,
    Latitude := REAL#59.3293,
    Longitude := REAL#18.0686
);
ASSERT_NEAR(EXPECTED := ClassicPosition.B, ACTUAL := Position.Azimuth, DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := ClassicPosition.HR, ACTUAL := Position.RefractedHeight, DELTA := REAL#0.01);

ClassicTimes(LATITUDE := REAL#59.3293, LONGITUDE := REAL#18.0686, UTC := DATE#2026-04-26, H := REAL#-0.8333333);
Times := ClockObject.CalculateSunTime(
    UtcDate := DATE#2026-04-26,
    Latitude := REAL#59.3293,
    Longitude := REAL#18.0686,
    Horizon := REAL#-0.8333333
);
ASSERT_EQUAL(EXPECTED := ClassicTimes.MIDDAY, ACTUAL := Times.Midday);
ASSERT_EQUAL(EXPECTED := ClassicTimes.SUN_RISE, ACTUAL := Times.Rise);
ASSERT_EQUAL(EXPECTED := ClassicTimes.SUN_SET, ACTUAL := Times.Sunset);
END_TEST_PROGRAM

TEST_PROGRAM oscat_components_instances_do_not_share_scan_state
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

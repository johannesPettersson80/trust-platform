TEST_PROGRAM GreenhouseTemperatureOopMatchClassicConversionAndSwitch
VAR
    Converter : UnitConverter;
    VentilationSwitch : HysteresisSwitch;
    ClassicSwitch : HYST;
    Limits : RealRange;
    InvalidLimits : RealRange;
    SwitchStatus : ComponentStatus;
END_VAR

Converter.Initialize();
Limits.Low := REAL#18.0;
Limits.High := REAL#28.0;
InvalidLimits.Low := REAL#28.0;
InvalidLimits.High := REAL#18.0;
VentilationSwitch.Initialize();
VentilationSwitch.SetLimits(Limits := InvalidLimits);
SwitchStatus := VentilationSwitch.Snapshot();
ASSERT_TRUE(IN := SwitchStatus.Error);
ASSERT_EQUAL(EXPECTED := ComponentErrorInvalidConfiguration, ACTUAL := SwitchStatus.ErrorId);
VentilationSwitch.ClearError();
VentilationSwitch.SetLimits(Limits := Limits);
ClassicSwitch(In := REAL#29.0, HIGH_LIMIT := REAL#28.0, LOW_LIMIT := REAL#18.0);

ASSERT_NEAR(EXPECTED := REAL#302.15, ACTUAL := Converter.KelvinFromCelsius(Celsius := REAL#29.0), DELTA := REAL#0.01);
ASSERT_TRUE(IN := VentilationSwitch.Update(MeasuredValue := REAL#29.0) = ClassicSwitch.Q);
END_TEST_PROGRAM

(* Multi-temperature greenhouse cycle test: drives the same converter
   and ventilation switch through hot -> comfortable -> cold inputs.
   Verifies both components react to each new sample rather than
   caching the first reading. *)
TEST_PROGRAM GreenhouseComponentsCycleThroughDayNightTemperatures
VAR
    Converter : UnitConverter;
    VentilationSwitch : HysteresisSwitch;
    Limits : RealRange;
END_VAR

Converter.Initialize();
Limits.Low := REAL#18.0;
Limits.High := REAL#28.0;
VentilationSwitch.Initialize();
VentilationSwitch.SetLimits(Limits := Limits);

(* Hot - vent on, conversion reflects 29 C *)
ASSERT_NEAR(EXPECTED := REAL#302.15, ACTUAL := Converter.KelvinFromCelsius(Celsius := REAL#29.0), DELTA := REAL#0.01);
ASSERT_TRUE(IN := VentilationSwitch.Update(MeasuredValue := REAL#29.0));

(* Comfortable - in window, vent retains, conversion reflects 22 C *)
ASSERT_NEAR(EXPECTED := REAL#295.15, ACTUAL := Converter.KelvinFromCelsius(Celsius := REAL#22.0), DELTA := REAL#0.01);
VentilationSwitch.Update(MeasuredValue := REAL#22.0);
ASSERT_TRUE(IN := VentilationSwitch.Window);

(* Cold - vent off, conversion reflects 15 C *)
ASSERT_NEAR(EXPECTED := REAL#288.15, ACTUAL := Converter.KelvinFromCelsius(Celsius := REAL#15.0), DELTA := REAL#0.01);
ASSERT_FALSE(IN := VentilationSwitch.Update(MeasuredValue := REAL#15.0));
END_TEST_PROGRAM

(* Hysteresis behaviour regression: once Q latches above the high limit
   it must stay TRUE while the value is inside the deadband and only
   release when the value drops below the low limit. *)
TEST_PROGRAM GreenhouseHysteresisLatchHoldsAcrossDeadband
VAR
    VentilationSwitch : HysteresisSwitch;
    Limits : RealRange;
END_VAR

Limits.Low := REAL#18.0;
Limits.High := REAL#28.0;
VentilationSwitch.Initialize();
VentilationSwitch.SetLimits(Limits := Limits);

(* Sweep from cool to hot to cool: verify that the deadband holds the
   command latched while the value lives inside the band. *)
ASSERT_FALSE(IN := VentilationSwitch.Update(MeasuredValue := REAL#17.0));
ASSERT_FALSE(IN := VentilationSwitch.Update(MeasuredValue := REAL#22.0));
ASSERT_TRUE(IN := VentilationSwitch.Update(MeasuredValue := REAL#28.5));
(* Inside the deadband: Q stays latched at TRUE *)
ASSERT_TRUE(IN := VentilationSwitch.Update(MeasuredValue := REAL#22.0));
ASSERT_TRUE(IN := VentilationSwitch.Window);
ASSERT_TRUE(IN := VentilationSwitch.Update(MeasuredValue := REAL#19.0));
(* Drop below low limit: Q releases *)
ASSERT_FALSE(IN := VentilationSwitch.Update(MeasuredValue := REAL#17.5));
ASSERT_FALSE(IN := VentilationSwitch.Window);
END_TEST_PROGRAM

(* Boundary tests around the configured limits. The switch uses strict
   greater-than against High and strict less-than against Low; values
   exactly at the limit fall in the Window region. *)
TEST_PROGRAM GreenhouseHysteresisBoundaryValues
VAR
    VentilationSwitch : HysteresisSwitch;
    Limits : RealRange;
END_VAR

Limits.Low := REAL#18.0;
Limits.High := REAL#28.0;
VentilationSwitch.Initialize();
VentilationSwitch.SetLimits(Limits := Limits);

(* Exactly at low limit: Window is TRUE, Q is FALSE *)
ASSERT_FALSE(IN := VentilationSwitch.Update(MeasuredValue := REAL#18.0));
ASSERT_TRUE(IN := VentilationSwitch.Window);
(* Exactly at high limit: Window is TRUE, Q stays FALSE (strict >) *)
ASSERT_FALSE(IN := VentilationSwitch.Update(MeasuredValue := REAL#28.0));
ASSERT_TRUE(IN := VentilationSwitch.Window);
(* Just above high limit: Q latches *)
ASSERT_TRUE(IN := VentilationSwitch.Update(MeasuredValue := REAL#28.0001));
ASSERT_FALSE(IN := VentilationSwitch.Window);
END_TEST_PROGRAM

(* Independent isolation of two switches: limits and Q on switch A do
   not bleed into switch B. Composition gives one instance per
   responsibility; the test pins that property. *)
TEST_PROGRAM GreenhouseTwoZonesAreIndependent
VAR
    HotZone : HysteresisSwitch;
    CoolZone : HysteresisSwitch;
    HotLimits : RealRange;
    CoolLimits : RealRange;
END_VAR

HotZone.Initialize();
CoolZone.Initialize();
HotLimits.Low := REAL#22.0;
HotLimits.High := REAL#30.0;
CoolLimits.Low := REAL#10.0;
CoolLimits.High := REAL#16.0;
HotZone.SetLimits(Limits := HotLimits);
CoolZone.SetLimits(Limits := CoolLimits);

(* Same temperature 25 C: hot zone is in window, cool zone trips on *)
HotZone.Update(MeasuredValue := REAL#25.0);
CoolZone.Update(MeasuredValue := REAL#25.0);
ASSERT_TRUE(IN := HotZone.Window);
ASSERT_FALSE(IN := HotZone.Q);
ASSERT_TRUE(IN := CoolZone.Q);
ASSERT_FALSE(IN := CoolZone.Window);

(* Drop to 12 C: cool zone in window, hot zone off *)
HotZone.Update(MeasuredValue := REAL#12.0);
CoolZone.Update(MeasuredValue := REAL#12.0);
ASSERT_FALSE(IN := HotZone.Q);
ASSERT_FALSE(IN := HotZone.Window);
ASSERT_TRUE(IN := CoolZone.Q);
ASSERT_TRUE(IN := CoolZone.Window);
END_TEST_PROGRAM

(* Unit conversion round-trip: Celsius -> Kelvin -> Celsius restores
   the original value within rounding tolerance. *)
TEST_PROGRAM GreenhouseUnitConverterRoundTrip
VAR
    Converter : UnitConverter;
    Original : REAL;
    Kelvin : REAL;
    Roundtrip : REAL;
    Fahrenheit : REAL;
    BackFromF : REAL;
END_VAR

Converter.Initialize();

(* Celsius -> Kelvin -> Celsius *)
Original := REAL#23.5;
Kelvin := Converter.KelvinFromCelsius(Celsius := Original);
Roundtrip := Converter.CelsiusFromKelvin(Kelvin := Kelvin);
ASSERT_NEAR(EXPECTED := Original, ACTUAL := Roundtrip, DELTA := REAL#0.01);

(* Celsius -> Fahrenheit -> Celsius *)
Fahrenheit := Converter.FahrenheitFromCelsius(Celsius := Original);
BackFromF := Converter.CelsiusFromFahrenheit(Fahrenheit := Fahrenheit);
ASSERT_NEAR(EXPECTED := Original, ACTUAL := BackFromF, DELTA := REAL#0.05);

(* Cold extreme: -40 C is the same in F as in C *)
ASSERT_NEAR(EXPECTED := REAL#-40.0, ACTUAL := Converter.FahrenheitFromCelsius(Celsius := REAL#-40.0), DELTA := REAL#0.01);
END_TEST_PROGRAM

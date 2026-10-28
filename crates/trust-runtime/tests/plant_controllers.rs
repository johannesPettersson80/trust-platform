//! Plant library specification §8 and §14.3: the on/off controller switches with hysteresis and
//! drops its demand on a bad process value. A PID loop is no block of the library: Create PLC
//! code writes it as ordinary code around the real `Control.PID` (specification 39, §10.5), and
//! the generator's tests compile and run that code.

mod plant_support;

use plant_support::Rig;

const PROGRAM: &str = r#"
USING Plant;

PROGRAM Main
VAR
    Temperature : LREAL := 50.0;
    TemperatureGood : BOOL := TRUE;
    Thermostat : Plant.FB_OnOffController := (Setpoint := 50.0, Hysteresis := 4.0);
    Chiller : Plant.FB_OnOffController := (Setpoint := 50.0, Hysteresis := 4.0, Heating := FALSE);
END_VAR
    Thermostat(ProcessValue := Temperature, ProcessValueGood := TemperatureGood);
    Chiller(ProcessValue := Temperature, ProcessValueGood := TemperatureGood);
END_PROGRAM
"#;

fn rig() -> Rig {
    let mut rig = Rig::new(PROGRAM);
    rig.scan();
    rig
}

#[test]
fn the_on_off_controller_switches_with_hysteresis() {
    let mut rig = rig();
    // Setpoint 50 with a hysteresis of 4: heating demand below 48, none above 52, unchanged in
    // between; cooling the other way round.
    for (temperature, heating, cooling) in [
        (47.0, true, false),
        (49.0, true, false),
        (52.5, false, true),
        (51.0, false, true),
        (49.0, false, true),
        (47.5, true, false),
    ] {
        rig.set_real("Temperature", temperature);
        rig.scan();
        assert_eq!(
            rig.flag("Thermostat.Demand"),
            heating,
            "heating demand at {temperature}"
        );
        assert_eq!(
            rig.flag("Chiller.Demand"),
            cooling,
            "cooling demand at {temperature}"
        );
    }
    rig.set_bool("TemperatureGood", false);
    rig.scan();
    assert!(
        !rig.flag("Thermostat.Demand"),
        "a bad process value drops the demand"
    );
}

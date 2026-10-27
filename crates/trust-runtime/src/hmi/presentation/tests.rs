use super::*;

fn page(objects: &str) -> Vec<u8> {
    format!("format = 1\ntitle = \"Formats\"\n{objects}").into_bytes()
}

#[test]
fn a_trend_pen_seeds_the_unit_it_actually_displays() {
    let mut bundle = HmiBundle::default();
    bundle.files.insert(
        "hmi/pages/trends.toml".into(),
        page(&object(
            "trend",
            "trend",
            "unit = \"°C\"\ndecimals = 2",
            "Level = { path = \"Main.Level\", unit = \"m\" }\nTemperature = \"Main.Temp\"",
        )),
    );
    let formats = value_formats(&bundle, Catalogue::shipped());
    assert_eq!(formats["main.level"].unit.as_deref(), Some("m"));
    assert_eq!(formats["main.temp"].unit.as_deref(), Some("°C"));
}
#[test]
fn a_controller_shows_its_measurement_and_setpoint_in_the_format_of_the_value_it_reads() {
    let mut bundle = HmiBundle::default();
    bundle.files.insert(
        "hmi/pages/overview.toml".into(),
        page(
            &(object(
                "controller",
                "controller",
                "measures = \"sensor\"",
                "pv = \"Main.TIC101_Input.ProcessValue\"\nsp = \"Main.TIC101_Setpoint\"",
            ) + &object(
                "sensor",
                "temperature-sensor",
                "",
                "value = \"Main.TT101.Value\"",
            ) + &object(
                "tile",
                "value",
                "unit = \"°C\"\ndecimals = 1",
                "value = \"Main.TT101.Value\"",
            ) + &object(
                "output",
                "value",
                "decimals = 0",
                "value = \"Main.TIC101_Output\"",
            )),
        ),
    );
    let formats = value_formats(&bundle, Catalogue::shipped());
    let shared = ValueFormat {
        unit: Some("°C".into()),
        decimals: Some(1),
    };
    assert_eq!(formats.get("main.tic101_input.processvalue"), Some(&shared));
    assert_eq!(formats.get("main.tic101_setpoint"), Some(&shared));
    assert_eq!(
        formats
            .get("main.tic101_output")
            .and_then(|format| format.unit.as_deref()),
        None,
        "the output is another quantity"
    );
}

fn object(id: &str, kind: &str, properties: &str, binding: &str) -> String {
    format!("\n[[object]]\nid = \"{id}\"\ntype = \"{kind}\"\nat = [0, 0]\nsize = [160, 160]\n{properties}\n[object.bind]\n{binding}\n")
}

#[test]
fn a_value_inherits_project_units_and_precision_without_rewriting_its_objects() {
    let mut bundle = HmiBundle::default();
    bundle.files.insert(
        "hmi/pages/a.toml".into(),
        page(
            &(object(
                "z-override",
                "value",
                "decimals = 0",
                "value = \"MAIN.LEVEL\"",
            ) + &object(
                "a-source",
                "bar",
                "decimals = 2",
                "value = { path = \"Main.Level\", unit = \"m\" }",
            )),
        ),
    );
    bundle.files.insert(
        "hmi/pages/b.toml".into(),
        page(&object("gauge", "gauge", "", "value = \"Main.Level\"")),
    );
    let before = bundle.clone();
    let formats = value_formats(&bundle, Catalogue::shipped());
    assert_eq!(
        formats.get("main.level"),
        Some(&ValueFormat {
            unit: Some("m".into()),
            decimals: Some(2)
        })
    );
    assert_eq!(
        bundle, before,
        "inherited formats are not written into authored files"
    );
}

#[test]
fn unit_and_precision_defaults_are_independent_and_boolean_state_bindings_do_not_seed_them() {
    let mut bundle = HmiBundle::default();
    bundle.files.insert(
        "hmi/pages/a.toml".into(),
        page(
            &(object("unit", "value", "unit = \"°C\"", "value = \"Main.Temp\"")
                + &object(
                    "pump",
                    "pump",
                    "unit = \"rpm\"\ndecimals = 1",
                    "running = \"Main.Run\"",
                )),
        ),
    );
    bundle.files.insert(
        "hmi/popups/b.toml".into(),
        format!(
            "format = 1\ntitle = \"Popup\"\nsize = [480, 320]\n{}",
            object(
                "precision",
                "value",
                "decimals = 1",
                "value = \"Main.Temp\""
            )
        )
        .into_bytes(),
    );
    let formats = value_formats(&bundle, Catalogue::shipped());
    assert_eq!(
        formats.get("main.temp"),
        Some(&ValueFormat {
            unit: Some("°C".into()),
            decimals: Some(1)
        })
    );
    assert!(
        !formats.contains_key("main.run"),
        "a pump's running word does not inherit its numeric unit"
    );
}

/// The Plant library's instrument and tank contracts (Plant §7.1, §7.5), with the members the
/// presentation reads: an instrument derived from `FB_AnalogInput` and a tank.
const PLANT: &str = r#"
NAMESPACE Plant
TYPE E_Priority : (Low, Medium, High); END_TYPE
FUNCTION_BLOCK FB_LimitAlarm
VAR_INPUT
    Priority : E_Priority;
END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_AnalogInput
VAR PUBLIC
    Unit : STRING[16];
END_VAR
VAR PUBLIC RETAIN
    RangeLow : LREAL := 0.0;
    RangeHigh : LREAL := 100.0;
    HighHighEnabled : BOOL;
    HighEnabled : BOOL;
    LowEnabled : BOOL;
    LowLowEnabled : BOOL;
    HighHighLimit : LREAL := 95.0;
    HighLimit : LREAL := 90.0;
    LowLimit : LREAL := 10.0;
    LowLowLimit : LREAL := 5.0;
END_VAR
VAR_OUTPUT
    Value : LREAL;
END_VAR
VAR
    HighHighAlarm : FB_LimitAlarm := (Priority := E_Priority#High);
    HighAlarm : FB_LimitAlarm := (Priority := E_Priority#Medium);
    LowAlarm : FB_LimitAlarm := (Priority := E_Priority#Medium);
    LowLowAlarm : FB_LimitAlarm := (Priority := E_Priority#High);
END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_LevelInput EXTENDS FB_AnalogInput
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Tank
VAR PUBLIC RETAIN
    Height : LREAL := 4.0;
END_VAR
VAR_OUTPUT
    Level : LREAL;
END_VAR
END_FUNCTION_BLOCK
END_NAMESPACE
PROGRAM Main
VAR
    LT101 : Plant.FB_LevelInput := (Unit := 'm', RangeHigh := 5.0, HighEnabled := TRUE, HighLimit := 4.5, LowEnabled := TRUE, LowLimit := 0.5);
    T101 : Plant.FB_Tank;
END_VAR
END_PROGRAM
"#;

/// Specification 39, §5.9: a Plant instrument's value is shown with its unit, its range and its
/// enabled alarm limits; a tank's level with its height as the range.
#[test]
fn a_plant_instrument_and_tank_present_their_unit_range_and_enabled_limits() {
    use crate::hmi::member_paths::MemberPath;
    use crate::hmi::page_files::HmiLimits;
    let harness = crate::harness::TestHarness::from_source(PLANT).expect("the program compiles");
    harness.runtime().with_program_view(|program| {
        let path = |text: &str| MemberPath::parse(text).expect("a path");
        let shown = equipment_presentation(program, &path("Main.LT101.Value"))
            .expect("a level instrument is an analog instrument");
        assert_eq!(shown.unit.as_deref(), Some("m"));
        assert_eq!(shown.range, Some([0.0, 5.0]));
        assert_eq!(
            shown.limits,
            Some(HmiLimits {
                low_low: None,
                low: Some(0.5),
                high: Some(4.5),
                high_high: None,
            }),
            "only the enabled limits"
        );
        assert_eq!(
            shown.priorities.get("high-high").map(String::as_str),
            Some("high")
        );
        assert!(
            equipment_presentation(program, &path("Main.LT101.RangeHigh")).is_none(),
            "only the physical value is presented"
        );
        let tank = equipment_presentation(program, &path("Main.T101.Level")).expect("a tank");
        assert_eq!(tank.range, Some([0.0, 4.0]));
        assert_eq!(tank.limits, None);
        assert_eq!(
            alarm_value_format(program, "Main.LT101", "Main.LT101.HighAlarm"),
            Some(("Main.LT101.Value".to_string(), Some("m".to_string())))
        );
    });
}

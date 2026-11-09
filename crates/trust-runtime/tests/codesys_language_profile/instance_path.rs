//! Variables marked `{attribute 'instance-path'}` (specification 10 §4.8), in a module of their own so that the
//! CODESYS profile's test program stays below the 1,000 lines of a file.

use super::*;

// Specification 10 §4.8: a STRING variable marked {attribute 'instance-path'} and
// {attribute 'noinit'} in a POU marked {attribute 'reflection'} starts with its instance's path, as
// CODESYS gives "the device tree path of the POU which it belongs to" (its Example 277,
// `PLCWinNT.Application.PLC_PRG.myPOU`), under every profile. Worked by hand for the configuration
// `Plant`, its resource `Line1` and the program instance `Cell`: `Plant.Line1.Cell.myPOU`; the
// same cut to a STRING(12), `Plant.Line1.`; a nested instance `Plant.Line1.Cell.outer.inner`, an
// element `Plant.Line1.Cell.pumps[2]` and a global `Plant.Line1.gNamed`; empty for a variable
// without `noinit` and for a block without `reflection`, as the vendor requires all three; the same
// path after a cold restart. Catches the path missing, the configuration's names left out, a
// member's or an element's part wrong, the cut ignored, and the path given without the vendor's
// three attributes.
const INSTANCE_PATH: &str = r#"
CONFIGURATION Plant
RESOURCE Line1 ON PLC
    VAR_GLOBAL
        gNamed : FB_Named;
    END_VAR
    TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Cell WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION

{attribute 'reflection'}
FUNCTION_BLOCK FB_Named
VAR
    {attribute 'instance-path'}
    {attribute 'noinit'}
    path : STRING;
    {attribute 'instance-path'}
    {attribute 'noinit'}
    short : STRING(12);
    {attribute 'instance-path'}
    bare : STRING;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Plain
VAR
    {attribute 'instance-path'}
    {attribute 'noinit'}
    path : STRING;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Outer
VAR
    inner : FB_Named;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    myPOU : FB_Named;
    outer : FB_Outer;
    pumps : ARRAY[1..2] OF FB_Named;
    plain : FB_Plain;
END_VAR
myPOU();
END_PROGRAM
"#;

#[test]
fn an_instance_path_variable_starts_with_its_instances_path() {
    let expected = [
        ("Cell.myPOU.path", "Plant.Line1.Cell.myPOU"),
        ("Cell.myPOU.short", "Plant.Line1."),
        ("Cell.myPOU.bare", ""),
        ("Cell.outer.inner.path", "Plant.Line1.Cell.outer.inner"),
        ("Cell.pumps[2].path", "Plant.Line1.Cell.pumps[2]"),
        ("gNamed.path", "Plant.Line1.gNamed"),
        ("Cell.plain.path", ""),
    ];
    for profile in [ConversionProfile::Codesys, ConversionProfile::Iec] {
        let session = CompileSession::from_source(INSTANCE_PATH).with_conversion_profile(profile);
        let mut harness = TestHarness::from_session(session)
            .unwrap_or_else(|error| panic!("{profile:?}: {error}"));
        for restart in ["installed", "cold"] {
            if restart == "cold" {
                harness
                    .restart(RestartMode::Cold)
                    .expect("the cold restart runs");
            }
            let cycle = harness.cycle();
            assert!(
                cycle.errors.is_empty(),
                "{profile:?} {restart}: {:?}",
                cycle.errors
            );
            for (place, path) in expected {
                assert_eq!(
                    harness.get_output(place),
                    Some(Value::String(path.into())),
                    "{profile:?} {restart} {place}"
                );
            }
        }
    }
}

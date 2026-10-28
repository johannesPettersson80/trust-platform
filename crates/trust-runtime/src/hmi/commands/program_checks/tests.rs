//! Specification 39, §21.2 item 2 and §21.4 item 4: method commands and settings checked against
//! the running program's image, as the web server reads it from the runtime's debug control with
//! debugging disabled, the production default (specification 11 §6.9). Every expected value is
//! worked out by hand from the program below.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use trust_runtime_core::native::program::AdmittedImage;

use super::{equipment, member_enumeration, method_inputs};
use crate::debug::DebugControl;
use crate::harness::TestHarness;
use crate::hmi::commands::capability::{Argument, MethodCommand};
use crate::hmi::commands::write::{write_point, WriteAction, WriteRefused};
use crate::hmi::member_paths::{member_value, MemberPath};
use crate::hmi::page_files::{HmiScalar, HmiSetting};
use crate::hmi::setting_values::requested_write;
use crate::value::{Duration, EnumValue, Value};

/// A drive whose base implements `IDrive`, an interface that extends `IDevice`. `Configure` stores
/// its inputs, whose declared initial values are 50.0, T#2s and Manual; `Measure` takes an output.
const PROGRAM: &str = r#"
TYPE E_Mode : (Auto, Manual, OutOfService); END_TYPE

INTERFACE IDevice
    METHOD SetMode : BOOL
    VAR_INPUT
        Value : E_Mode;
    END_VAR
    END_METHOD
    METHOD GetMode : E_Mode
    END_METHOD
END_INTERFACE

INTERFACE IDrive EXTENDS IDevice
    METHOD Configure : BOOL
    VAR_INPUT
        Speed : LREAL := 50.0;
        Ramp : TIME := T#2s;
        Start : E_Mode := E_Mode#Manual;
    END_VAR
    END_METHOD
    METHOD Measure : BOOL
    VAR_OUTPUT
        Reading : LREAL;
    END_VAR
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK DriveBase IMPLEMENTS IDrive
VAR_OUTPUT
    Mode : E_Mode;
    LastSpeed : LREAL;
    LastRamp : TIME;
    LastStart : E_Mode;
    Configured : DINT;
END_VAR
VAR PUBLIC
    StartMode : E_Mode := E_Mode#Auto;
    Gain : REAL := 1.5;
END_VAR
METHOD PUBLIC SetMode : BOOL
VAR_INPUT
    Value : E_Mode;
END_VAR
    Mode := Value;
    SetMode := TRUE;
END_METHOD
METHOD PUBLIC GetMode : E_Mode
    GetMode := Mode;
END_METHOD
METHOD PUBLIC Configure : BOOL
VAR_INPUT
    Speed : LREAL := 50.0;
    Ramp : TIME := T#2s;
    Start : E_Mode := E_Mode#Manual;
END_VAR
    LastSpeed := Speed;
    LastRamp := Ramp;
    LastStart := Start;
    Configured := Configured + 1;
    Configure := TRUE;
END_METHOD
METHOD PUBLIC Measure : BOOL
VAR_OUTPUT
    Reading : LREAL;
END_VAR
    Reading := LastSpeed;
    Measure := TRUE;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Drive EXTENDS DriveBase
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    D1 : Drive;
    Count : DINT;
END_VAR
    D1();
END_PROGRAM
"#;

/// The program after one scan, and its image as the web server reads it: from the runtime's
/// debug control, here with debugging disabled.
fn running(source: &str) -> (TestHarness, Arc<AdmittedImage>) {
    let mut harness = TestHarness::from_source(source).expect("the program compiles");
    let debug = DebugControl::new().with_enabled_flag(Arc::new(AtomicBool::new(false)));
    harness.runtime_mut().set_debug_control(debug.clone());
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    let image = debug
        .program_image()
        .expect("the debug control holds the image of the program the runtime runs");
    (harness, image)
}

fn command(interface: &str, method: &str, arguments: &[(&str, Argument)]) -> MethodCommand {
    MethodCommand {
        interface: interface.into(),
        method: method.into(),
        instance: "Main.D1".into(),
        arguments: arguments
            .iter()
            .map(|(name, argument)| ((*name).to_string(), argument.clone()))
            .collect(),
    }
}

fn text(value: &str) -> Argument {
    Argument::Constant(HmiScalar::Text(value.into()))
}

fn number(value: f64) -> Argument {
    Argument::Constant(HmiScalar::Number(value))
}

/// The control's own value.
fn entered() -> Argument {
    Argument::Input { input: true }
}

fn mode(name: &str, number: i64) -> Value {
    Value::Enum(Box::new(EnumValue::from_canonical_parts(
        SmolStr::new("E_Mode"),
        SmolStr::new(name),
        number,
    )))
}

/// The inputs `command` gives `Main.D1`, as the runtime checks them.
fn inputs(
    image: &AdmittedImage,
    command: &MethodCommand,
    sent: Option<serde_json::Value>,
) -> Result<Vec<(SmolStr, Value)>, WriteRefused> {
    let image = image.image();
    let at = equipment(&image, &command.instance)?;
    method_inputs(&image, &at, command, sent.as_ref())
}

fn value(harness: &TestHarness, path: &str) -> Option<Value> {
    let path = MemberPath::parse(path).expect("a path");
    harness
        .runtime()
        .with_program_view(|program| member_value(&path, program))
}

#[test]
fn an_instance_implements_the_interface_through_its_base() {
    // Drive declares no interface; its base DriveBase implements IDrive, which extends IDevice
    // (IEC 61131-3 §6.6.6.4). So every method of both interfaces is the drive's: Configure of
    // IDrive, SetMode of IDevice, and SetMode through IDrive. An enumeration input takes the value
    // of that name, `OutOfService` the third value, 2, `Manual` the second, 1. Catches a check
    // that reads only the instance's own type ("Drive does not implement IDrive"), one that misses
    // the methods of an interface's base, and an enumeration value by another number.
    let (_harness, image) = running(PROGRAM);
    assert_eq!(
        inputs(
            &image,
            &command("IDrive", "Configure", &[("Speed", number(75.5))]),
            None
        ),
        Ok(vec![(SmolStr::new("Speed"), Value::LReal(75.5))])
    );
    assert_eq!(
        inputs(
            &image,
            &command("IDevice", "SetMode", &[("Value", text("OutOfService"))]),
            None
        ),
        Ok(vec![(SmolStr::new("Value"), mode("OutOfService", 2))])
    );
    assert_eq!(
        inputs(
            &image,
            &command("IDrive", "SetMode", &[("Value", entered())]),
            Some(json!("Manual"))
        ),
        Ok(vec![(SmolStr::new("Value"), mode("Manual", 1))]),
        "a method of the interface's base"
    );
    let image = image.image();
    assert_eq!(
        equipment(&image, "Main.Count").err().as_deref(),
        Some("the equipment is not available"),
        "a DINT is no instance"
    );
    assert_eq!(
        equipment(&image, "Main.D2").err().as_deref(),
        Some("the equipment is not available"),
        "the program declares no D2"
    );
    let main = equipment(&image, "Main").expect("a program instance is an instance");
    assert_eq!(
        method_inputs(
            &image,
            &main,
            &command("IDevice", "SetMode", &[("Value", entered())]),
            Some(&json!("Auto"))
        ),
        Err(WriteRefused::from("Main does not implement IDevice"))
    );
}

#[test]
fn inputs_a_command_leaves_out_take_the_initial_values_the_interface_declares() {
    // Specification 44 §5.4 and IEC 61131-3 §6.6.1: a method's inputs start from their initial
    // values at each call, and the program gives each input the call does not give the initial
    // value the interface declares. A first call gives all three inputs, 10.0, 5 s and Auto, and
    // the drive stores them. A second call gives only Speed, 75.5: the HMI sends that input alone,
    // and the drive then stores 75.5, T#2s and Manual, the declared initial values, not the 5 s and
    // Auto of the first call, nor zeros. Configured counts the two calls. Catches an HMI that sends
    // defaults of its own, a call that keeps the previous call's inputs, and inputs left at zero.
    let (mut harness, image) = running(PROGRAM);
    let path = MemberPath::parse("Main.D1").expect("a path");
    let all = inputs(
        &image,
        &command(
            "IDrive",
            "Configure",
            &[
                ("Speed", number(10.0)),
                ("Ramp", number(5.0)),
                ("Start", text("Auto")),
            ],
        ),
        None,
    )
    .expect("all three inputs");
    assert_eq!(
        all,
        vec![
            (SmolStr::new("Speed"), Value::LReal(10.0)),
            (SmolStr::new("Ramp"), Value::Time(Duration::from_secs(5))),
            (SmolStr::new("Start"), mode("Auto", 0)),
        ],
        "a TIME in seconds, the inputs in their declared order"
    );
    let mut budget = 10_000;
    let first = harness
        .runtime_mut()
        .invoke_hmi_method(&path, "IDrive", "Configure", &all, &mut budget)
        .expect("the first call runs");
    assert_eq!(first, Value::Bool(true));
    assert_eq!(
        value(&harness, "Main.D1.LastRamp"),
        Some(Value::Time(Duration::from_secs(5)))
    );
    assert_eq!(value(&harness, "Main.D1.LastStart"), Some(mode("Auto", 0)));

    let speed_only = inputs(
        &image,
        &command("IDrive", "Configure", &[("Speed", number(75.5))]),
        None,
    )
    .expect("one input");
    assert_eq!(
        speed_only,
        vec![(SmolStr::new("Speed"), Value::LReal(75.5))],
        "only the input the command gives"
    );
    let second = harness
        .runtime_mut()
        .invoke_hmi_method(&path, "IDrive", "Configure", &speed_only, &mut budget)
        .expect("the second call runs");
    assert_eq!(second, Value::Bool(true));
    assert_eq!(
        value(&harness, "Main.D1.LastSpeed"),
        Some(Value::LReal(75.5))
    );
    assert_eq!(
        value(&harness, "Main.D1.LastRamp"),
        Some(Value::Time(Duration::from_secs(2)))
    );
    assert_eq!(
        value(&harness, "Main.D1.LastStart"),
        Some(mode("Manual", 1))
    );
    assert_eq!(value(&harness, "Main.D1.Configured"), Some(Value::DInt(2)));
}

#[test]
fn a_method_a_tool_cannot_call_or_that_answers_no_bool_is_refused_with_its_reason() {
    // Specification 44 §5.4, §5.7: Measure takes an output, so its tool symbol has no tool entry
    // and the call is refused before anything runs; GetMode returns an E_Mode, not the BOOL whose
    // TRUE accepts a command (§21.2 item 2); IDevice declares no Start, so it has no tool symbol.
    // Catches a method with an output that is called anyway, a result that is not BOOL taken as
    // the outcome, and an undeclared method passed on to the kernel.
    let (_harness, image) = running(PROGRAM);
    assert_eq!(
        inputs(&image, &command("IDrive", "Measure", &[]), None),
        Err(WriteRefused::from(
            "HMI methods accept only input parameters"
        ))
    );
    assert_eq!(
        inputs(&image, &command("IDevice", "GetMode", &[]), None),
        Err(WriteRefused::from(
            "IDevice.GetMode must return BOOL, TRUE when the command is accepted"
        ))
    );
    assert_eq!(
        inputs(&image, &command("IDevice", "Start", &[]), None),
        Err(WriteRefused::from("IDevice has no method Start"))
    );
}

#[test]
fn a_method_that_takes_an_in_out_has_no_tool_entry_and_is_refused() {
    // Specification 44 §5.4: a method that takes a `VAR_IN_OUT` has a tool symbol without a tool
    // entry, as a call block cannot hold the caller's variable. Catches the in-out passed on to the
    // kernel as if it were an input.
    let (_harness, image) = running(
        r#"
INTERFACE IShare
    METHOD Exchange : BOOL
    VAR_IN_OUT
        Shared : LREAL;
    END_VAR
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK Sharer IMPLEMENTS IShare
VAR_OUTPUT
    Last : LREAL;
END_VAR
METHOD PUBLIC Exchange : BOOL
VAR_IN_OUT
    Shared : LREAL;
END_VAR
    Last := Shared;
    Exchange := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    S1 : Sharer;
END_VAR
    S1();
END_PROGRAM
"#,
    );
    let image = image.image();
    let sharer = equipment(&image, "Main.S1").expect("the sharer");
    let exchange = MethodCommand {
        instance: "Main.S1".into(),
        ..command("IShare", "Exchange", &[])
    };
    assert_eq!(
        method_inputs(&image, &sharer, &exchange, None),
        Err(WriteRefused::from(
            "HMI methods accept only input parameters"
        ))
    );
}

#[test]
fn arguments_name_inputs_and_one_entered_value_fits_its_type() {
    // §21.2 items 1 and 2: each argument names an input of the method, compared without case
    // (IEC 61131-3 §6.1.2), and the call takes the input's declared name; a control sends one value
    // at most, only when an argument takes it, and every value must fit its input's type. Catches
    // an argument matched by case, two entered values, a value sent to a control that takes none or
    // missing for one that needs it, and a value outside its input's type.
    let (_harness, image) = running(PROGRAM);
    assert_eq!(
        inputs(
            &image,
            &command("IDrive", "Configure", &[("speed", number(5.0))]),
            None
        ),
        Ok(vec![(SmolStr::new("Speed"), Value::LReal(5.0))])
    );
    let refusal = |command: MethodCommand, sent: Option<serde_json::Value>| {
        inputs(&image, &command, sent).expect_err("refused").reason
    };
    assert_eq!(
        refusal(
            command("IDevice", "SetMode", &[("Mode", text("Auto"))]),
            None
        ),
        "SetMode has no argument Mode"
    );
    assert_eq!(
        refusal(
            command("IDevice", "SetMode", &[("Value", text("Sideways"))]),
            None
        ),
        "Value: \"Sideways\" is not a value of E_Mode"
    );
    assert_eq!(
        refusal(
            command(
                "IDrive",
                "Configure",
                &[("Speed", entered()), ("Ramp", entered())]
            ),
            Some(json!(1))
        ),
        "One visual control supplies one entered value"
    );
    assert_eq!(
        refusal(
            command("IDrive", "Configure", &[("Speed", number(5.0))]),
            Some(json!(1))
        ),
        "this control sends no value"
    );
    assert_eq!(
        refusal(
            command("IDrive", "Configure", &[("Speed", entered())]),
            None
        ),
        "this control needs a value"
    );
    assert!(refusal(
        command("IDrive", "Configure", &[("Speed", text("fast"))]),
        None
    )
    .starts_with("Speed: a LREAL takes a finite number"));
    assert!(refusal(
        command("IDrive", "Configure", &[("Ramp", number(-1.0))]),
        None
    )
    .starts_with("Ramp: a TIME takes a number of seconds from 0"));
}

fn setting(member: &str, title: &str, min: Option<f64>, max: Option<f64>) -> HmiSetting {
    HmiSetting {
        member: member.into(),
        title: title.into(),
        unit: None,
        min,
        max,
        step: None,
        choices: Vec::new(),
        key: false,
        help: None,
    }
}

#[test]
fn a_setting_takes_its_enumerations_values_and_its_reals_range_from_the_running_program() {
    // §21.4 item 4: each value is checked against its variable's type in the running program's
    // type table. StartMode is an E_Mode: Manual by name, and 2, the number of OutOfService, are
    // values of it; Sideways and 7 are not. Gain is a REAL from 0 to 10 in its description: 2.5
    // fits, 11 is above the maximum, and without limits 1e39 is past a REAL's range (3.4e38) and
    // "fast" is no number. Catches the enumeration's values unknown without the project's
    // declarations ("the values of E_Mode are not known", the defect of the empty metadata), a
    // name or number the type does not declare accepted, and a REAL past its range accepted.
    let (harness, image) = running(PROGRAM);
    let image = image.image();
    let start_mode = MemberPath::parse("Main.D1.StartMode").expect("a path");
    let current = value(&harness, "Main.D1.StartMode").expect("StartMode reads");
    assert_eq!(current, mode("Auto", 0));
    let start = setting("StartMode", "Start mode", None, None);
    let write = |requested: serde_json::Value| {
        requested_write(
            &start,
            start.unit.as_deref(),
            &current,
            &requested,
            |name| member_enumeration(&image, &start_mode, name),
        )
    };
    assert_eq!(
        write(json!("Manual")),
        Ok(WriteAction::Set(json!("Manual")))
    );
    assert_eq!(write(json!(2)), Ok(WriteAction::Set(json!(2))));
    assert_eq!(
        write(json!("Sideways")),
        Err("Start mode: \"Sideways\" is not a value of E_Mode".to_string())
    );
    assert_eq!(
        write(json!(7)),
        Err("Start mode: 7 is not a value of E_Mode".to_string())
    );

    let gain_path = MemberPath::parse("Main.D1.Gain").expect("a path");
    let gain = value(&harness, "Main.D1.Gain").expect("Gain reads");
    assert_eq!(gain, Value::Real(1.5));
    let limited = setting("Gain", "Gain", Some(0.0), Some(10.0));
    let free = setting("Gain", "Gain", None, None);
    let write = |setting: &HmiSetting, requested: serde_json::Value| {
        requested_write(
            setting,
            setting.unit.as_deref(),
            &gain,
            &requested,
            |name| member_enumeration(&image, &gain_path, name),
        )
    };
    assert_eq!(
        write(&limited, json!(2.5)),
        Ok(WriteAction::Set(json!(2.5)))
    );
    assert!(write(&limited, json!(11)).is_err_and(
        |reason| reason.starts_with("Gain: 11") && reason.contains("above the maximum 10")
    ));
    assert!(write(&free, json!(1e39)).is_err_and(
        |reason| reason.starts_with("Gain:") && reason.contains("outside the range of a REAL")
    ));
    assert!(write(&free, json!("fast"))
        .is_err_and(|reason| reason == "Gain: enter a number, not \"fast\""));
}

/// A tag whose `Label` takes a `STRING[8]` and a `WSTRING[4]`.
const LABEL_PROGRAM: &str = r#"
INTERFACE ILabel
    METHOD Label : BOOL
    VAR_INPUT
        Name : STRING[8];
        Wide : WSTRING[4];
    END_VAR
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK Tag IMPLEMENTS ILabel
VAR_OUTPUT
    Shown : STRING[8];
    ShownWide : WSTRING[4];
END_VAR
METHOD PUBLIC Label : BOOL
VAR_INPUT
    Name : STRING[8];
    Wide : WSTRING[4];
END_VAR
    Shown := Name;
    ShownWide := Wide;
    Label := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    T1 : Tag;
END_VAR
    T1();
END_PROGRAM
"#;

#[test]
fn a_text_longer_than_its_input_holds_is_refused_never_cut() {
    // §12.8 item 5 and §21.2 item 5: Label's STRING[8] input holds 8 bytes of UTF-8 and its
    // WSTRING[4] input 4 UTF-16 code units. "Pump 101" is 8 bytes and fits; "Pümp 101" is 8
    // characters but 9 bytes, as ü takes two. "Äöü€" is 4 code units and fits; "𝄞abc" is 4
    // characters but 5 code units, as 𝄞 (U+1D11E) takes two. A longer text is refused before the
    // call with the stable code `runtime_string_capacity_exceeded`, as an HMI write is, and never
    // cut, as the conversion of the call's inputs would cut it. Catches a text cut to its input's
    // capacity, a capacity counted in characters, and a refusal without its code.
    let (_harness, image) = running(LABEL_PROGRAM);
    let label = |arguments: &[(&str, Argument)]| {
        let label = MethodCommand {
            instance: "Main.T1".into(),
            ..command("ILabel", "Label", arguments)
        };
        inputs(&image, &label, None)
    };
    assert_eq!(
        label(&[("Name", text("Pump 101")), ("Wide", text("Äöü€"))]),
        Ok(vec![
            (SmolStr::new("Name"), Value::String("Pump 101".into())),
            (SmolStr::new("Wide"), Value::WString("Äöü€".into())),
        ])
    );
    assert_eq!(
        label(&[("Name", text("Pümp 101"))]),
        Err(WriteRefused {
            reason: "\"Name\" holds at most 8 bytes of UTF-8; the text has 9".into(),
            error_code: Some("runtime_string_capacity_exceeded"),
        })
    );
    assert_eq!(
        label(&[("Wide", text("𝄞abc"))]),
        Err(WriteRefused {
            reason: "\"Wide\" holds at most 4 UTF-16 code units; the text has 5".into(),
            error_code: Some("runtime_string_capacity_exceeded"),
        })
    );
}

/// `E_Mode2`, an alias of the enumeration `E_Mode`, as a variable, a method's input and a setting.
const ALIAS_PROGRAM: &str = r#"
TYPE E_Mode : (Auto, Manual, OutOfService); END_TYPE
TYPE E_Mode2 : E_Mode; END_TYPE

INTERFACE IChoose
    METHOD Choose : BOOL
    VAR_INPUT
        Value : E_Mode2;
    END_VAR
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK Chooser IMPLEMENTS IChoose
VAR_OUTPUT
    Chosen : E_Mode2;
END_VAR
VAR PUBLIC
    Preferred : E_Mode2;
END_VAR
METHOD PUBLIC Choose : BOOL
VAR_INPUT
    Value : E_Mode2;
END_VAR
    Chosen := Value;
    Choose := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    C1 : Chooser;
    Mode : E_Mode2;
END_VAR
    C1();
END_PROGRAM
"#;

/// The name and the number of an enumeration's value.
fn variant(value: &Value) -> Option<(&str, i64)> {
    match value {
        Value::Enum(value) => Some((value.variant_name().as_str(), value.numeric_value())),
        _ => None,
    }
}

#[test]
fn an_alias_of_an_enumeration_takes_its_base_enumerations_values_everywhere() {
    // IEC 61131-3 §6.4.4.9: E_Mode2, derived directly from E_Mode, is a type of its own name with
    // E_Mode's values, which the build lays out as E_Mode's own (specification 44 §5.8). So an HMI
    // write, a method's input and a setting of an E_Mode2 take E_Mode's names and numbers: Auto,
    // the first, is 0, Manual 1 and OutOfService 2, and Sideways is none of them. Catches an alias
    // refused with "the values of E_Mode2 are not known" on any of the three paths, and a value
    // taken by another number.
    let (mut harness, image) = running(ALIAS_PROGRAM);
    let mode = MemberPath::parse("Main.Mode").expect("a path");
    let written = harness
        .runtime_mut()
        .with_program(|program| write_point(program, &mode, &WriteAction::Set(json!("Manual"))))
        .expect("the HMI writes Manual");
    assert_eq!(variant(&written.old), Some(("Auto", 0)));
    assert_eq!(variant(&written.readback), Some(("Manual", 1)));

    let choose = MethodCommand {
        instance: "Main.C1".into(),
        ..command("IChoose", "Choose", &[("Value", text("OutOfService"))])
    };
    let given = inputs(&image, &choose, None).expect("the method's input");
    assert_eq!(given.len(), 1);
    assert_eq!(given[0].0, "Value");
    assert_eq!(variant(&given[0].1), Some(("OutOfService", 2)));
    let mut budget = 10_000;
    let path = MemberPath::parse("Main.C1").expect("a path");
    harness
        .runtime_mut()
        .invoke_hmi_method(&path, "IChoose", "Choose", &given, &mut budget)
        .expect("the method runs");
    let chosen = value(&harness, "Main.C1.Chosen").expect("Chosen reads");
    assert_eq!(variant(&chosen), Some(("OutOfService", 2)));

    let preferred = MemberPath::parse("Main.C1.Preferred").expect("a path");
    let current = value(&harness, "Main.C1.Preferred").expect("Preferred reads");
    let setting = setting("Preferred", "Preferred mode", None, None);
    let image = image.image();
    let write = |requested: serde_json::Value| {
        requested_write(
            &setting,
            setting.unit.as_deref(),
            &current,
            &requested,
            |name| member_enumeration(&image, &preferred, name),
        )
    };
    assert_eq!(
        write(json!("Manual")),
        Ok(WriteAction::Set(json!("Manual")))
    );
    assert!(write(json!("Sideways"))
        .is_err_and(|reason| reason.starts_with("Preferred mode: \"Sideways\" is not a value of")));
}

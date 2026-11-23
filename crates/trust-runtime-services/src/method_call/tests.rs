//! `method.call`'s parameters, literals and states (specification 11 §6.8.1, "Calling a method").

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::json;
use smol_str::SmolStr;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::Value;

use super::{literal, parse, takes_calls};

#[test]
fn a_call_names_its_instance_interface_method_and_inputs_as_literals() {
    // Specification 11 §6.8.1: `instance`, `interface` and `method` are names, and `inputs` is an
    // object of Structured Text literals by input name, which the runtime types by each input's
    // declared type as `set` types a literal by its variable's. Catches a member read under another
    // name, an input dropped, a name kept with its spaces, and a literal changed before it is typed.
    let asked = parse(Some(&json!({
        "instance": " Main.Heating ",
        "interface": "Plant.ISequence",
        "method": "Start",
        "inputs": {"Setpoint": "21.5", "Hold": "T#5s", "Label": "'it$'s'"}
    })))
    .expect("a call");
    assert_eq!(asked.instance, "Main.Heating");
    assert_eq!(asked.interface, "Plant.ISequence");
    assert_eq!(asked.method, "Start");
    let mut inputs = asked.inputs;
    inputs.sort_by(|(left, _), (right, _)| left.cmp(right));
    assert_eq!(
        inputs,
        [
            (SmolStr::new("Hold"), "T#5s".to_string()),
            (SmolStr::new("Label"), "'it$'s'".to_string()),
            (SmolStr::new("Setpoint"), "21.5".to_string()),
        ]
    );

    // An input not given takes its declared initial value, so a call may give none.
    let without = parse(Some(&json!({
        "instance": "Main.Heating", "interface": "Plant.ISequence", "method": "Reset"
    })))
    .expect("a call without inputs");
    assert_eq!(without.inputs, Vec::<(SmolStr, String)>::new());
}

#[test]
fn a_call_with_a_member_or_a_name_it_does_not_take_is_refused() {
    // Each refusal names what is wrong, before the runtime is asked. Catches an unknown member
    // ignored, a blank name taken as a name, and inputs that are not an object of literals.
    let refused = |params: serde_json::Value| parse(Some(&params)).expect_err("refused");
    let base =
        || json!({"instance": "Main.Heating", "interface": "Plant.ISequence", "method": "Start"});
    let mut extra = base();
    extra["timeout"] = json!(5);
    assert_eq!(refused(extra), "unknown member timeout");
    let mut blank = base();
    blank["method"] = json!("  ");
    assert_eq!(refused(blank), "method must be a name");
    let mut missing = base();
    missing.as_object_mut().expect("object").remove("instance");
    assert_eq!(refused(missing), "instance must be a name");
    let mut listed = base();
    listed["inputs"] = json!(["21.5"]);
    assert_eq!(refused(listed), "inputs must be an object of literals");
    let mut number = base();
    number["inputs"] = json!({"Setpoint": 21.5});
    assert_eq!(refused(number), "inputs.Setpoint must be a literal");
    assert_eq!(parse(None).expect_err("refused"), "missing params");
}

#[test]
fn without_the_programs_image_a_literal_keeps_the_untyped_grammar() {
    // A platform without the program's image at hand cannot type a literal by its input's declared
    // type, so a literal is a boolean, an integer, a real or a text by its spelling, and the kernel
    // refuses what the input does not take: TRUE, -7, 1.5E3 = 1500 and 'a$$b' = a$b. A text whose
    // dollar sign escapes nothing it knows, or that ends with one, a real that is not finite and a
    // word are refused. Catches an exponent read as text, an escape kept as written, an infinity
    // passed to the program, and a word passed on as a value.
    assert_eq!(literal("TRUE"), Ok(Value::Bool(true)));
    assert_eq!(literal("-7"), Ok(Value::LInt(-7)));
    assert_eq!(literal("1.5E3"), Ok(Value::LReal(1500.0)));
    assert_eq!(literal("'a$$b'"), Ok(Value::String(SmolStr::new("a$b"))));
    assert_eq!(literal("''"), Ok(Value::String(SmolStr::new(""))));
    assert_eq!(literal("'a$b'"), Err("unsupported escape $b".to_string()));
    assert_eq!(literal("'a$'"), Err("a text cannot end with $".to_string()));
    assert_eq!(
        literal("1e999"),
        Err("unsupported value '1e999'".to_string())
    );
}

#[test]
fn only_a_resource_between_its_scans_takes_a_call() {
    // A running or paused resource is between two scans; one still starting makes the call before
    // its first scan. A stopped one has no scans to fault, a faulted one refuses every call until a
    // restart (specification 44 §13.3), and one waiting for its start gate has no loop to make it.
    // Catches a call queued where nothing makes it, and a call refused where the program waits.
    let taking: Vec<ResourceState> = [
        ResourceState::Boot,
        ResourceState::Ready,
        ResourceState::Running,
        ResourceState::Paused,
        ResourceState::Faulted,
        ResourceState::Stopped,
    ]
    .into_iter()
    .filter(|state| takes_calls(*state))
    .collect();
    assert_eq!(
        taking,
        [
            ResourceState::Boot,
            ResourceState::Running,
            ResourceState::Paused
        ]
    );
}

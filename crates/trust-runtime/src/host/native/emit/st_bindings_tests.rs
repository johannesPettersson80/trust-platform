//! The names of a binding's parameters (specification 48 RU-39). Every expected name is worked out
//! by hand.

use super::parameter_name;

#[test]
fn a_bindings_parameters_take_the_structured_text_names_rust_can_hold() {
    // RU-39: a binding's parameter bears the Structured Text parameter's name in lower case,
    // `factor` for FACTOR; a name that Rust keeps as a keyword (TYPE, LOOP, SELF) or that the
    // context's parameter takes (CX) gets `_` after it, so that layer 1 compiles. Catches a
    // binding whose parameter list Rust refuses.
    assert_eq!(parameter_name("FACTOR"), "factor");
    assert_eq!(parameter_name("Set_Point"), "set_point");
    assert_eq!(parameter_name("TYPE"), "type_");
    assert_eq!(parameter_name("LOOP"), "loop_");
    assert_eq!(parameter_name("SELF"), "self_");
    assert_eq!(parameter_name("CX"), "cx_");
}

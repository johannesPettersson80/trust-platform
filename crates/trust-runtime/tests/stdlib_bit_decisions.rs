//! Shift and rotate take ANY_BIT input only (spec 07 section 4).

use trust_runtime::error::RuntimeError;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::Value;

#[test]
fn shift_and_rotate_reject_every_non_bit_string_input() {
    let lib = StandardLibrary::new();
    let inputs = [
        Value::SInt(1),
        Value::Int(1),
        Value::DInt(1),
        Value::LInt(1),
        Value::USInt(1),
        Value::UInt(1),
        Value::UDInt(1),
        Value::ULInt(1),
        Value::Real(1.0),
        Value::LReal(1.0),
        Value::String("1".into()),
    ];

    for name in ["SHL", "SHR", "ROL", "ROR"] {
        for input in &inputs {
            assert_eq!(
                lib.call(name, &[input.clone(), Value::Int(1)]),
                Err(RuntimeError::TypeMismatch),
                "{name} {input:?}"
            );
        }
    }
}

#[test]
fn shift_and_rotate_accept_bool_and_bit_string_input() {
    let lib = StandardLibrary::new();

    // BOOL is ANY_BIT: a one-bit shift leaves nothing behind, a rotation keeps it.
    assert_eq!(
        lib.call("SHL", &[Value::Bool(true), Value::Int(1)]),
        Ok(Value::Bool(false))
    );
    assert_eq!(
        lib.call("ROL", &[Value::Bool(true), Value::Int(1)]),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        lib.call("SHL", &[Value::Byte(0xC0), Value::Int(2)]),
        Ok(Value::Byte(0))
    );
    assert_eq!(
        lib.call("ROL", &[Value::Byte(0xC0), Value::Int(2)]),
        Ok(Value::Byte(0x03))
    );
}

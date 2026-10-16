//! FIND, MID and DELETE boundaries (spec 07 section 8.1).

use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::Value;

#[test]
fn find_with_an_empty_needle_matches_at_position_one() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call(
            "FIND",
            &[Value::String("ABC".into()), Value::String("".into())]
        ),
        Ok(Value::Int(1))
    );
    assert_eq!(
        lib.call(
            "FIND",
            &[Value::WString("ABC".into()), Value::WString("".into())]
        ),
        Ok(Value::Int(1))
    );
    assert_eq!(
        lib.call(
            "FIND",
            &[Value::String("ABCABC".into()), Value::String("BC".into())]
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn mid_and_delete_clamp_a_length_past_the_final_element() {
    let lib = StandardLibrary::new();

    // MID(IN, L, P): the five requested characters from position 2 stop at the end.
    assert_eq!(
        lib.call(
            "MID",
            &[Value::String("ABCDE".into()), Value::Int(10), Value::Int(2)]
        ),
        Ok(Value::String("BCDE".into()))
    );
    // DELETE(IN, L, P): removing from position 2 to the end leaves the first character.
    assert_eq!(
        lib.call(
            "DELETE",
            &[Value::String("ABCDE".into()), Value::Int(10), Value::Int(2)]
        ),
        Ok(Value::String("A".into()))
    );
    assert_eq!(
        lib.call(
            "MID",
            &[
                Value::WString("ABCDE".into()),
                Value::Int(10),
                Value::Int(2)
            ]
        ),
        Ok(Value::WString("BCDE".into()))
    );
    assert_eq!(
        lib.call(
            "DELETE",
            &[
                Value::WString("ABCDE".into()),
                Value::Int(10),
                Value::Int(2)
            ]
        ),
        Ok(Value::WString("A".into()))
    );
}

#[test]
fn mid_and_delete_from_a_start_past_the_end_leave_the_text_unchanged() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call(
            "MID",
            &[Value::String("ABC".into()), Value::Int(2), Value::Int(10)]
        ),
        Ok(Value::String("".into()))
    );
    assert_eq!(
        lib.call(
            "DELETE",
            &[Value::String("ABC".into()), Value::Int(2), Value::Int(10)]
        ),
        Ok(Value::String("ABC".into()))
    );
}

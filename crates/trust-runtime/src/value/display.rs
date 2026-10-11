//! Hosted value formatting compatibility facade.

pub use trust_runtime_core::value::format_user_value;

#[cfg(test)]
use super::Value;

#[cfg(test)]
#[path = "display/contract_tests.rs"]
mod contract_tests;

#[cfg(test)]
mod tests {
    use crate::memory::InstanceId;
    use crate::value::{format_user_value, Duration, Value};

    #[test]
    fn format_user_value_hides_rust_value_debug_names() {
        let samples = [
            (Value::Bool(true), "TRUE"),
            (Value::Bool(false), "FALSE"),
            (Value::Int(1), "1"),
            (Value::DInt(2), "2"),
            (Value::Real(1.0), "1.0"),
            (Value::LReal(1.5), "1.5"),
            (Value::Word(16), "16"),
            (Value::String("pump$'a".into()), "'pump$$$'a'"),
            (Value::Time(Duration::from_millis(250)), "T#250ms"),
            (Value::Instance(InstanceId(0)), "Instance"),
        ];

        for (value, expected) in samples {
            let actual = format_user_value(&value);
            assert_eq!(actual, expected);
            assert!(!actual.contains("Int("), "{actual}");
            assert!(!actual.contains("Real("), "{actual}");
            assert!(!actual.contains("Instance("), "{actual}");
        }
    }
}

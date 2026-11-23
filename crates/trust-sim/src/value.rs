//! The Rust values a machine test writes and reads (specification 48 RU-58, "Values"): each is
//! written as the Structured Text literal that `set` and `var.force` take, and read from the JSON
//! that `variables.read` gives.

use serde_json::Value as Json;

/// A Rust value of a variable's type: `bool`, the integers, `f32`, `f64` and `String`.
pub trait Value: Sized {
    /// The kind of value, as an error names it: `BOOL`, `an integer`, `a real`, `a text`.
    const KIND: &'static str;

    /// The value as a Structured Text literal: `TRUE`, `42`, `95.0`, `'text'`.
    fn literal(self) -> String;

    /// The value that `variables.read` gives as `json`, or none for a value of another kind.
    fn from_json(json: &Json) -> Option<Self>;
}

impl Value for bool {
    const KIND: &'static str = "BOOL";

    fn literal(self) -> String {
        if self { "TRUE" } else { "FALSE" }.to_string()
    }

    fn from_json(json: &Json) -> Option<Self> {
        json.as_bool()
    }
}

macro_rules! integer {
    ($($rust:ty),*) => {$(
        impl Value for $rust {
            const KIND: &'static str = "an integer";

            fn literal(self) -> String {
                self.to_string()
            }

            fn from_json(json: &Json) -> Option<Self> {
                json.as_i64()
                    .and_then(|value| Self::try_from(value).ok())
                    .or_else(|| json.as_u64().and_then(|value| Self::try_from(value).ok()))
            }
        }
    )*};
}

integer!(i8, i16, i32, i64, u8, u16, u32, u64);

impl Value for f64 {
    const KIND: &'static str = "a real";

    fn literal(self) -> String {
        // `{:?}` keeps a decimal point, `95.0`, which Structured Text needs to read a real.
        format!("{self:?}")
    }

    fn from_json(json: &Json) -> Option<Self> {
        json.as_f64()
    }
}

impl Value for f32 {
    const KIND: &'static str = "a real";

    fn literal(self) -> String {
        format!("{self:?}")
    }

    #[allow(clippy::cast_possible_truncation)]
    fn from_json(json: &Json) -> Option<Self> {
        // A REAL's JSON number is the f32's value written as an f64, which narrows back exactly.
        json.as_f64().map(|value| value as f32)
    }
}

impl Value for String {
    const KIND: &'static str = "a text";

    fn literal(self) -> String {
        // A Structured Text string literal: single quotes, a quote inside doubled with `$`.
        format!("'{}'", self.replace('$', "$$").replace('\'', "$'"))
    }

    fn from_json(json: &Json) -> Option<Self> {
        json.as_str().map(str::to_string)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Value;

    #[test]
    fn values_are_written_as_structured_text_literals() {
        // `set` and `var.force` read Structured Text literals (specification 11, "Variable mutation
        // targets"): a real keeps its decimal point, a text its single quotes, and `$` escapes a
        // quote and itself, as IEC 61131-3's character strings write them. Catches a real written
        // as `95`, which Structured Text reads as an integer, and a text whose quote ends it early.
        assert_eq!(true.literal(), "TRUE");
        assert_eq!(42_i16.literal(), "42");
        assert_eq!(95.0_f64.literal(), "95.0");
        assert_eq!(0.25_f32.literal(), "0.25");
        assert_eq!("it's $5".to_string().literal(), "'it$'s $$5'");
    }

    #[test]
    fn values_are_read_from_the_reads_json() {
        // `variables.read` gives a BOOL as a JSON boolean, an integer and a real as numbers and a
        // text as a string. Catches an integer read past its type's range, which would wrap.
        assert_eq!(bool::from_json(&json!(true)), Some(true));
        assert_eq!(i16::from_json(&json!(-7)), Some(-7));
        assert_eq!(u8::from_json(&json!(300)), None);
        assert_eq!(f64::from_json(&json!(95.0)), Some(95.0));
        assert_eq!(String::from_json(&json!("ok")), Some("ok".to_string()));
        assert_eq!(bool::from_json(&json!(1)), None);
    }
}

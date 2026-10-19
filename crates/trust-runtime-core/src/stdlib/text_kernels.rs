//! The standard text functions on borrowed texts (specification 34 ARCH-05; C1-12).
//!
//! The engine runs the built-in `CONCAT`, `DELETE`, `FIND`, `INSERT`, `LEFT`, `LEN`, `MID`,
//! `REPLACE` and `RIGHT`, and the assignment cut `__TRUST_LIMIT_STRING`, on their arguments where
//! they lie and writes a text result into a buffer it keeps, so that a call allocates nothing once
//! the engine is warm. Each gives the result and the refusal that its `fn(&[Value])` form in
//! [`super::string`] gives, the refusals in the same order.

use crate::error::RuntimeError;
use crate::numeric::to_i64;
use crate::stdlib::conversions::{convert_text_into, convert_value_into, ConversionType};
use crate::value::{
    check_string_result, string_capacity_cut, string_delete_into, string_element_count,
    string_find, string_insert_into, string_left_into, string_mid_into, string_replace_into,
    string_right_into, Value,
};
use alloc::string::String;

/// A standard text function the engine runs on borrowed texts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextKernel {
    Concat,
    Delete,
    Find,
    Insert,
    Left,
    Len,
    Mid,
    Replace,
    Right,
    Limit,
    /// A conversion from or to a text: `src_TO_dst`, its source type named or not.
    Convert {
        src: Option<ConversionType>,
        dst: ConversionType,
    },
}

/// The functions by their registered names.
const KERNELS: [(&str, TextKernel); 10] = [
    ("CONCAT", TextKernel::Concat),
    ("DELETE", TextKernel::Delete),
    ("FIND", TextKernel::Find),
    ("INSERT", TextKernel::Insert),
    ("LEFT", TextKernel::Left),
    ("LEN", TextKernel::Len),
    ("MID", TextKernel::Mid),
    ("REPLACE", TextKernel::Replace),
    ("RIGHT", TextKernel::Right),
    ("__TRUST_LIMIT_STRING", TextKernel::Limit),
];

impl TextKernel {
    /// The kernel of the standard function `name`, in any case, or `None` for another function.
    pub(crate) fn of(name: &str) -> Option<Self> {
        KERNELS
            .iter()
            .find(|(registered, _)| registered.eq_ignore_ascii_case(name))
            .map(|(_, kernel)| *kernel)
    }
}

/// One argument of a text function: a borrowed text with its width, or another value, borrowed or
/// read as a scalar from a block.
#[derive(Debug, Clone)]
pub(crate) enum TextArg<'a> {
    /// The characters of a `STRING`, or of a `WSTRING` when `wide`.
    Text { text: &'a str, wide: bool },
    /// Any other value.
    Value(&'a Value),
    /// A value read from a block: a scalar, or the null value for a compound, which no text
    /// function takes.
    Scalar(Value),
}

impl<'a> TextArg<'a> {
    /// The argument for the value `value`: its text, borrowed, or the value.
    pub(crate) fn of(value: &'a Value) -> Self {
        match value {
            Value::String(text) => TextArg::Text {
                text: text.as_str(),
                wide: false,
            },
            Value::WString(text) => TextArg::Text {
                text: text.as_str(),
                wide: true,
            },
            other => TextArg::Value(other),
        }
    }

    /// The argument's text and width, when it is a text.
    fn text(self) -> Option<(&'a str, bool)> {
        match self {
            TextArg::Text { text, wide } => Some((text, wide)),
            TextArg::Value(_) | TextArg::Scalar(_) => None,
        }
    }

    /// The argument as an integer, refused as `to_i64` refuses its value.
    fn integer(self) -> Result<i64, RuntimeError> {
        match self {
            TextArg::Value(value) => to_i64(value),
            TextArg::Scalar(value) => to_i64(&value),
            TextArg::Text { .. } => Err(RuntimeError::TypeMismatch),
        }
    }
}

/// What a text function gives: a text it wrote into the buffer, a `WSTRING` when `wide`, or a
/// value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TextResult {
    /// The text in the buffer.
    Text { wide: bool },
    /// A value.
    Value(Value),
}

/// Run `kernel` on its `count` arguments, argument `i` given by `arg(i)` in formal order, writing a
/// text result into `out`, which comes empty; a conversion cleans a real's separators in
/// `scratch`.
///
/// # Errors
///
/// The refusal of the function's `fn(&[Value])` form for the same arguments, and the refusal of
/// `arg`.
pub(crate) fn run<'a>(
    kernel: TextKernel,
    count: usize,
    arg: impl Fn(usize) -> Result<TextArg<'a>, RuntimeError>,
    out: &mut String,
    scratch: &mut String,
) -> Result<TextResult, RuntimeError> {
    let arity = |expected: usize| {
        if count == expected {
            Ok(())
        } else {
            Err(RuntimeError::InvalidArgumentCount {
                expected,
                got: count,
            })
        }
    };
    let text = |index: usize| arg(index)?.text().ok_or(RuntimeError::TypeMismatch);
    let pair = |left: usize, right: usize| match (arg(left)?.text(), arg(right)?.text()) {
        (Some((left, wide)), Some((right, other))) if wide == other => Ok((left, right, wide)),
        _ => Err(RuntimeError::TypeMismatch),
    };
    match kernel {
        TextKernel::Len => {
            arity(1)?;
            let length = string_element_count(text(0)?.0);
            let length = i16::try_from(length).map_err(|_| RuntimeError::Overflow)?;
            Ok(TextResult::Value(Value::Int(length)))
        }
        TextKernel::Left | TextKernel::Right => {
            arity(2)?;
            let characters = arg(1)?.integer()?;
            let (input, wide) = text(0)?;
            if kernel == TextKernel::Left {
                string_left_into(input, characters, out);
            } else {
                string_right_into(input, characters, out);
            }
            checked(out, wide)
        }
        TextKernel::Mid => {
            arity(3)?;
            let length = arg(1)?.integer()?;
            let position = arg(2)?.integer()?;
            let (input, wide) = text(0)?;
            string_mid_into(input, length, position, out);
            checked(out, wide)
        }
        TextKernel::Concat => {
            if count < 2 {
                return Err(RuntimeError::InvalidArgumentCount {
                    expected: 2,
                    got: count,
                });
            }
            let wide = text(0)?.1;
            for index in 0..count {
                match arg(index)?.text() {
                    Some((part, other)) if other == wide => out.push_str(part),
                    _ => return Err(RuntimeError::TypeMismatch),
                }
            }
            checked(out, wide)
        }
        TextKernel::Insert => {
            arity(3)?;
            let position = arg(2)?.integer()?;
            let (input, insert, wide) = pair(0, 1)?;
            string_insert_into(input, insert, position, out);
            checked(out, wide)
        }
        TextKernel::Delete => {
            arity(3)?;
            let length = arg(1)?.integer()?;
            let position = arg(2)?.integer()?;
            let (input, wide) = text(0)?;
            string_delete_into(input, length, position, out);
            checked(out, wide)
        }
        TextKernel::Replace => {
            arity(4)?;
            let length = arg(2)?.integer()?;
            let position = arg(3)?.integer()?;
            let (input, replacement, wide) = pair(0, 1)?;
            string_replace_into(input, replacement, length, position, out);
            checked(out, wide)
        }
        TextKernel::Find => {
            arity(2)?;
            let (haystack, needle, _) = pair(0, 1)?;
            Ok(TextResult::Value(Value::Int(string_find(
                haystack, needle,
            )?)))
        }
        TextKernel::Convert { src, dst } => {
            arity(1)?;
            match arg(0)? {
                TextArg::Text { text, wide } => {
                    convert_text_into(src, dst, text, wide, out, scratch)
                }
                TextArg::Value(value) => convert_value_into(src, dst, value, out),
                TextArg::Scalar(value) => convert_value_into(src, dst, &value, out),
            }
        }
        TextKernel::Limit => {
            arity(2)?;
            let capacity = u32::try_from(arg(1)?.integer()?).map_err(|_| RuntimeError::Overflow)?;
            let capacity = usize::try_from(capacity).map_err(|_| RuntimeError::Overflow)?;
            let (input, wide) = text(0)?;
            let end = string_capacity_cut(input, wide, capacity).unwrap_or(input.len());
            out.push_str(&input[..end]);
            Ok(TextResult::Text { wide })
        }
    }
}

/// A text result, refused above the implementer maximum as its value form refuses it (IEC
/// 61131-3 Ed.3 §6.6.2.5.11).
fn checked(out: &str, wide: bool) -> Result<TextResult, RuntimeError> {
    check_string_result(out, wide)?;
    Ok(TextResult::Text { wide })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::StandardLibrary;
    use alloc::vec::Vec;
    use smol_str::SmolStr;

    /// Run `name` both ways: the kernel on borrowed arguments and the registered function on the
    /// values, and answer the kernel's result as a value.
    fn both(
        name: &str,
        args: &[Value],
    ) -> (Result<Value, RuntimeError>, Result<Value, RuntimeError>) {
        let views: Vec<TextArg<'_>> = args.iter().map(TextArg::of).collect();
        let mut out = String::new();
        let kernel = TextKernel::of(name)
            .or_else(
                || match crate::stdlib::conversions::conversion_spec(name)? {
                    crate::stdlib::conversions::ConversionSpec::Convert { src, dst } => {
                        Some(TextKernel::Convert { src, dst })
                    }
                    _ => None,
                },
            )
            .expect("a text kernel");
        let mut scratch = String::new();
        let ran = run(
            kernel,
            views.len(),
            |index| Ok(views[index].clone()),
            &mut out,
            &mut scratch,
        )
        .map(|result| match result {
            TextResult::Text { wide: false } => Value::String(SmolStr::new(&out)),
            TextResult::Text { wide: true } => Value::WString(out.clone()),
            TextResult::Value(value) => value,
        });
        (ran, StandardLibrary::new().call(name, args))
    }

    /// Spec 34 ARCH-05 and TIME-08 (C1-12): each kernel gives what its registered function gives,
    /// the refusals included: a count or a position below one, past the end and out of range, a
    /// non-ASCII text whose characters are more than one byte, a `WSTRING`, mixed widths, a missing
    /// argument and a cut that keeps whole characters. The expected values are the registered
    /// functions' answers, which their own suites pin from IEC 61131-3 Table 34. Catches a kernel
    /// that counts bytes instead of characters, one that clamps a position differently, and one that
    /// refuses in another order.
    #[test]
    fn every_kernel_answers_as_its_registered_function() {
        let s = |text: &str| Value::String(SmolStr::new(text));
        let w = |text: &str| Value::WString(String::from(text));
        let cases: Vec<(&str, Vec<Value>)> = alloc::vec![
            ("LEN", alloc::vec![s("Grüße")]),
            ("LEN", alloc::vec![w("Ventil")]),
            ("LEN", alloc::vec![Value::Int(3)]),
            ("LEFT", alloc::vec![s("Grüße"), Value::Int(3)]),
            ("LEFT", alloc::vec![s("pump"), Value::Int(-2)]),
            ("LEFT", alloc::vec![s("pump"), Value::DInt(99)]),
            ("LEFT", alloc::vec![w("Ventil"), Value::Int(2)]),
            ("LEFT", alloc::vec![s("pump"), s("x")]),
            ("RIGHT", alloc::vec![s("Grüße"), Value::Int(2)]),
            ("RIGHT", alloc::vec![s("pump"), Value::Int(0)]),
            ("MID", alloc::vec![s("Grüße"), Value::Int(2), Value::Int(3)]),
            ("MID", alloc::vec![s("pump"), Value::Int(2), Value::Int(9)]),
            ("MID", alloc::vec![s("pump"), Value::Int(-1), Value::Int(1)]),
            ("CONCAT", alloc::vec![s("Grü"), s("ße"), s("!")]),
            ("CONCAT", alloc::vec![w("Ven"), w("til")]),
            ("CONCAT", alloc::vec![s("a"), w("b")]),
            ("INSERT", alloc::vec![s("Grüße"), s("#"), Value::Int(2)]),
            ("INSERT", alloc::vec![s("pump"), s("#"), Value::Int(-4)]),
            ("INSERT", alloc::vec![s("pump"), w("#"), Value::Int(1)]),
            (
                "DELETE",
                alloc::vec![s("Grüße"), Value::Int(2), Value::Int(3)]
            ),
            (
                "DELETE",
                alloc::vec![s("pump"), Value::Int(0), Value::Int(1)]
            ),
            (
                "DELETE",
                alloc::vec![s("pump"), Value::Int(1), Value::Int(9)]
            ),
            (
                "REPLACE",
                alloc::vec![s("Grüße"), s("P"), Value::Int(1), Value::Int(1)]
            ),
            (
                "REPLACE",
                alloc::vec![s("pump"), s("P"), Value::Int(0), Value::Int(2)]
            ),
            (
                "REPLACE",
                alloc::vec![s("pump"), s("P"), Value::Int(1), Value::Int(7)]
            ),
            ("FIND", alloc::vec![s("Grüße"), s("ß")]),
            ("FIND", alloc::vec![s("pump"), s("x")]),
            ("FIND", alloc::vec![w("Ventil"), w("til")]),
            (
                "__TRUST_LIMIT_STRING",
                alloc::vec![s("Grüße"), Value::Int(3)]
            ),
            (
                "__TRUST_LIMIT_STRING",
                alloc::vec![w("Ventil"), Value::Int(3)]
            ),
            (
                "__TRUST_LIMIT_STRING",
                alloc::vec![s("pump"), Value::Int(-1)]
            ),
        ];
        for (name, args) in cases {
            let (kernel, registered) = both(name, &args);
            assert_eq!(kernel, registered, "{name}({args:?})");
        }
    }

    /// Spec 34 ARCH-05 and TIME-08 (C1-12): each conversion from or to a text gives what the
    /// conversion of the value gives, the refusals included: numbers, a character and a byte into a
    /// text, a value outside the named source type, a decimal text with separators and spaces, a
    /// based text, a text that is no number, a real with separators and one too large, a text into
    /// the other width and into a character, a generic conversion, and an integer text outside the
    /// destination. The expected values are the value forms' answers, which the conversion suites
    /// pin from IEC 61131-3 §6.6.2.5.6. Catches a parser that keeps a separator or a sign it should
    /// refuse, a formatter that drops a real's `.0`, and a conversion that skips the source type's
    /// range check.
    #[test]
    fn every_conversion_answers_as_its_value_form() {
        let s = |text: &str| Value::String(SmolStr::new(text));
        let w = |text: &str| Value::WString(String::from(text));
        let cases: Vec<(&str, Vec<Value>)> = alloc::vec![
            ("DINT_TO_STRING", alloc::vec![Value::DInt(-42)]),
            ("REAL_TO_STRING", alloc::vec![Value::Real(1.25)]),
            ("REAL_TO_STRING", alloc::vec![Value::Real(3.0)]),
            ("LREAL_TO_WSTRING", alloc::vec![Value::LReal(-0.5)]),
            ("CHAR_TO_STRING", alloc::vec![Value::Char(b'A')]),
            ("BYTE_TO_STRING", alloc::vec![Value::Byte(255)]),
            ("INT_TO_STRING", alloc::vec![Value::DInt(70_000)]),
            ("STRING_TO_DINT", alloc::vec![s(" 1_000 ")]),
            ("STRING_TO_DINT", alloc::vec![s("16#FF")]),
            ("STRING_TO_DINT", alloc::vec![s("-16#8000_0000")]),
            ("STRING_TO_DINT", alloc::vec![s("1__0")]),
            ("STRING_TO_DINT", alloc::vec![s("pump")]),
            ("STRING_TO_INT", alloc::vec![s("70000")]),
            ("STRING_TO_REAL", alloc::vec![s("1_0.5")]),
            ("STRING_TO_LREAL", alloc::vec![s("1e999")]),
            ("WSTRING_TO_STRING", alloc::vec![w("Ventil")]),
            ("STRING_TO_WSTRING", alloc::vec![s("Grüße")]),
            ("STRING_TO_CHAR", alloc::vec![s("A")]),
            ("WSTRING_TO_WCHAR", alloc::vec![w("V")]),
            ("TO_STRING", alloc::vec![Value::Int(5)]),
            ("STRING_TO_DINT", alloc::vec![Value::DInt(5)]),
        ];
        for (name, args) in cases {
            let (kernel, value_form) = both(name, &args);
            assert_eq!(kernel, value_form, "{name}({args:?})");
        }
    }
}

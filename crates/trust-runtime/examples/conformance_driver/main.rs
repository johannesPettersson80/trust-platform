//! truST's driver for the harness of the IEC 61131-3 conformance suite
//! (github.com/s-celles/iec61131-conformance, `harness/trust/run.py`): it runs one test as the
//! suite's SPEC.md gives a harness, and the harness compares what it prints with the trace.
//!
//! `conformance_driver <file.st> <period_ms> <output>...` compiles the file with truST's test
//! harness, which builds the program for this computer with the toolchain `TRUST_TOOLCHAIN` names,
//! else the installed bundle (specification 44 §4.6), and starts it with its declared initial
//! values. It prints the outputs' declared types first, `types <name>=<TYPE>...`. Then, for each
//! record read on its standard input, `<cycle> <name>=<value>...` with the record's inputs in the
//! trace's value syntax, it writes the inputs, runs exactly one scan at PLC time
//! `cycle * period_ms`, and prints `<cycle> => <name>=<value>...` with the outputs in the trace's
//! value syntax, a string as `#<hex bytes>`. A scan that faults prints `<cycle> => trap: <the
//! fault>` and ends the run.
//!
//! Exit status: 0 after the last record; 1 when the program does not compile, with its errors on
//! standard error; 2 for any other refusal, also on standard error: a variable the program does
//! not have, a value its type does not take, a record out of order.

use std::io::{BufRead, Write};
use std::process::ExitCode;

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

/// Why the driver stops before the last record.
#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    /// The program does not compile.
    Compile(String),
    /// Anything else: an unknown variable, a value its type does not take, a record out of order.
    Other(String),
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [source, period, outputs @ ..] = args.as_slice() else {
        eprintln!("usage: conformance_driver <file.st> <period_ms> <output>...");
        return ExitCode::from(2);
    };
    let result = std::fs::read_to_string(source)
        .map_err(|error| Refusal::Other(format!("{source}: {error}")))
        .and_then(|text| {
            drive(
                &text,
                period,
                outputs,
                std::io::stdin().lock(),
                std::io::stdout().lock(),
            )
        });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Refusal::Compile(text)) => {
            eprintln!("{text}");
            ExitCode::from(1)
        }
        Err(Refusal::Other(text)) => {
            eprintln!("{text}");
            ExitCode::from(2)
        }
    }
}

/// Runs the program `text` record by record, the records read from `input`, and writes what it
/// prints to `out`.
fn drive(
    text: &str,
    period: &str,
    outputs: &[String],
    input: impl BufRead,
    mut out: impl Write,
) -> Result<(), Refusal> {
    let period_ms: i64 = period
        .parse()
        .ok()
        .filter(|ms| *ms > 0)
        .ok_or_else(|| Refusal::Other(format!("period_ms {period} is no positive number")))?;
    let mut harness =
        TestHarness::from_source(text).map_err(|error| Refusal::Compile(error.to_string()))?;
    let mut types = Vec::new();
    for name in outputs {
        types.push(format!("{name}={}", type_name(&read(&harness, name)?)));
    }
    print(&mut out, &format!("types {}", types.join(" ")))?;
    let mut next = 0_u64;
    for line in input.lines() {
        let line = line.map_err(|error| Refusal::Other(format!("standard input: {error}")))?;
        let mut tokens = line.split_whitespace();
        let Some(cycle) = tokens.next() else {
            continue;
        };
        if cycle.parse::<u64>().ok() != Some(next) {
            return Err(Refusal::Other(format!(
                "record {cycle} where {next} is next"
            )));
        }
        if next > 0 {
            harness.advance_time(Duration::from_millis(period_ms));
        }
        for token in tokens {
            let (name, text) = token.split_once('=').ok_or_else(|| {
                Refusal::Other(format!("record {next}: {token} is no name=value"))
            })?;
            let value = input_value(&read(&harness, name)?, text)
                .map_err(|error| Refusal::Other(format!("input {name}={text}: {error}")))?;
            harness
                .try_set_input(name, value)
                .map_err(|error| Refusal::Other(format!("input {name}={text}: {error}")))?;
        }
        let scan = harness.cycle();
        if let Some(fault) = scan.errors.first() {
            return print(&mut out, &format!("{next} => trap: {fault}"));
        }
        let mut values = Vec::new();
        for name in outputs {
            let value = read(&harness, name)?;
            let text = render(&value).ok_or_else(|| {
                Refusal::Other(format!("output {name}: no trace syntax for {value:?}"))
            })?;
            values.push(format!("{name}={text}"));
        }
        print(&mut out, &format!("{next} => {}", values.join(" ")))?;
        next += 1;
    }
    Ok(())
}

fn read(harness: &TestHarness, name: &str) -> Result<Value, Refusal> {
    harness
        .try_get_output(name)
        .map_err(|error| Refusal::Other(format!("variable {name}: {error}")))
}

fn print(out: &mut impl Write, line: &str) -> Result<(), Refusal> {
    writeln!(out, "{line}")
        .and_then(|()| out.flush())
        .map_err(|error| Refusal::Other(format!("standard output: {error}")))
}

/// The declared type of `value` as the trace header's types read it: `REAL` and `LREAL` take the
/// suite's tolerances, `STRING` its byte comparison.
fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Bool(_) => "BOOL",
        Value::SInt(_) => "SINT",
        Value::Int(_) => "INT",
        Value::DInt(_) => "DINT",
        Value::LInt(_) => "LINT",
        Value::USInt(_) => "USINT",
        Value::UInt(_) => "UINT",
        Value::UDInt(_) => "UDINT",
        Value::ULInt(_) => "ULINT",
        Value::Byte(_) => "BYTE",
        Value::Word(_) => "WORD",
        Value::DWord(_) => "DWORD",
        Value::LWord(_) => "LWORD",
        Value::Real(_) => "REAL",
        Value::LReal(_) => "LREAL",
        Value::Time(_) => "TIME",
        Value::LTime(_) => "LTIME",
        Value::String(_) => "STRING",
        _ => "OTHER",
    }
}

fn integer<T: TryFrom<i128>>(text: &str) -> Result<T, String> {
    let value = if let Some(hex) = text.strip_prefix("16#") {
        i128::from_str_radix(hex, 16).map_err(|_| format!("{text} is no number"))?
    } else {
        text.parse::<i128>()
            .map_err(|_| format!("{text} is no number"))?
    };
    T::try_from(value).map_err(|_| format!("{text} is out of range"))
}

fn real(text: &str) -> Result<f64, String> {
    match text {
        "inf" => Ok(f64::INFINITY),
        "-inf" => Ok(f64::NEG_INFINITY),
        "nan" => Ok(f64::NAN),
        _ => text
            .parse::<f64>()
            .map_err(|_| format!("{text} is no real")),
    }
}

fn millis(text: &str) -> Result<i64, String> {
    text.strip_prefix("T#")
        .and_then(|rest| rest.strip_suffix("ms"))
        .and_then(|ms| ms.parse::<i64>().ok())
        .ok_or_else(|| format!("{text} is no T#<n>ms"))
}

fn boolean(text: &str) -> Result<bool, String> {
    match text {
        "TRUE" => Ok(true),
        "FALSE" => Ok(false),
        _ => Err(format!("{text} is no BOOL")),
    }
}

/// The value `text` gives a variable whose value is now `current`, in the variable's type.
// A REAL input is written with the digits its trace gives, which an f32 holds as near as it can.
#[allow(clippy::cast_possible_truncation)]
fn input_value(current: &Value, text: &str) -> Result<Value, String> {
    Ok(match current {
        Value::Bool(_) => Value::Bool(boolean(text)?),
        Value::SInt(_) => Value::SInt(integer(text)?),
        Value::Int(_) => Value::Int(integer(text)?),
        Value::DInt(_) => Value::DInt(integer(text)?),
        Value::LInt(_) => Value::LInt(integer(text)?),
        Value::USInt(_) => Value::USInt(integer(text)?),
        Value::UInt(_) => Value::UInt(integer(text)?),
        Value::UDInt(_) => Value::UDInt(integer(text)?),
        Value::ULInt(_) => Value::ULInt(integer(text)?),
        Value::Byte(_) => Value::Byte(integer(text)?),
        Value::Word(_) => Value::Word(integer(text)?),
        Value::DWord(_) => Value::DWord(integer(text)?),
        Value::LWord(_) => Value::LWord(integer(text)?),
        Value::Real(_) => Value::Real(real(text)? as f32),
        Value::LReal(_) => Value::LReal(real(text)?),
        Value::Time(_) => Value::Time(Duration::from_millis(millis(text)?)),
        Value::LTime(_) => Value::LTime(Duration::from_millis(millis(text)?)),
        Value::String(_) => Value::String(
            String::from_utf8_lossy(&decode_string(text)?)
                .into_owned()
                .into(),
        ),
        other => return Err(format!("no input of the type of {other:?}")),
    })
}

/// The bytes of a trace's string literal: `'...'` with the escapes `$$ $' $L $N $P $R $T $xx`.
fn decode_string(text: &str) -> Result<Vec<u8>, String> {
    let inner = text
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .ok_or_else(|| format!("{text} is no string"))?;
    let bytes = inner.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'$' {
            out.push(bytes[index]);
            index += 1;
            continue;
        }
        let next = *bytes
            .get(index + 1)
            .ok_or_else(|| format!("{text}: a lone $"))?;
        let escaped = match next.to_ascii_uppercase() {
            b'$' => Some(b'$'),
            b'\'' => Some(b'\''),
            b'L' | b'N' => Some(b'\n'),
            b'P' => Some(0x0C),
            b'R' => Some(b'\r'),
            b'T' => Some(b'\t'),
            _ => None,
        };
        if let Some(byte) = escaped {
            out.push(byte);
            index += 2;
        } else {
            let hex = bytes
                .get(index + 1..index + 3)
                .and_then(|hex| std::str::from_utf8(hex).ok())
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
                .ok_or_else(|| format!("{text}: a bad $ escape"))?;
            out.push(hex);
            index += 3;
        }
    }
    Ok(out)
}

/// `value` in the trace's value syntax; `None` for a type the syntax has no form for.
fn render(value: &Value) -> Option<String> {
    Some(match value {
        Value::Bool(value) => if *value { "TRUE" } else { "FALSE" }.to_string(),
        Value::SInt(value) => value.to_string(),
        Value::Int(value) => value.to_string(),
        Value::DInt(value) => value.to_string(),
        Value::LInt(value) => value.to_string(),
        Value::USInt(value) => value.to_string(),
        Value::UInt(value) => value.to_string(),
        Value::UDInt(value) => value.to_string(),
        Value::ULInt(value) => value.to_string(),
        Value::Byte(value) => format!("16#{value:02X}"),
        Value::Word(value) => format!("16#{value:04X}"),
        Value::DWord(value) => format!("16#{value:08X}"),
        Value::LWord(value) => format!("16#{value:016X}"),
        Value::Real(value) => real_text(f64::from(*value), format!("{value:?}")),
        Value::LReal(value) => real_text(*value, format!("{value:?}")),
        Value::Time(value) | Value::LTime(value) => {
            let nanos = value.as_nanos();
            if nanos % 1_000_000 == 0 {
                format!("T#{}ms", nanos / 1_000_000)
            } else {
                format!("T#{nanos}ns")
            }
        }
        Value::String(value) => {
            let hex: String = value
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect();
            format!("#{hex}")
        }
        _ => return None,
    })
}

/// A real as the trace writes one: `inf`, `-inf`, `nan`, else Rust's shortest exact text.
fn real_text(value: f64, shortest: String) -> String {
    if value.is_nan() {
        "nan".to_string()
    } else if value.is_infinite() {
        if value > 0.0 { "inf" } else { "-inf" }.to_string()
    } else {
        shortest
    }
}

#[cfg(test)]
mod tests;

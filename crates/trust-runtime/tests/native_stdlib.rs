//! The standard functions of a compiled program compute what the runtime's compute (specification
//! 44 §6.8, §14.2): each family calls one function per scan over a dense set of inputs, edge values
//! and pseudo-random ones, compiled and in the runtime in lockstep, and every result and every error
//! of every scan is compared. The runtime's functions are pinned by their own oracle tests
//! (`stdlib_oracle_*`, `stdlib_*_decisions`), so a compiled function that gives what they give, bit
//! for bit and fault for fault, gives what specification 07 says.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

#[path = "support/native_lockstep.rs"]
#[allow(dead_code)]
mod native_lockstep;

use native_lockstep::compare_sources;
use trust_runtime::harness::SourceFile;

/// A deterministic sequence of pseudo-random numbers (Knuth's 64-bit LCG).
struct Numbers(u64);

impl Numbers {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }

    /// A number in `-span..span`.
    #[allow(clippy::cast_precision_loss)]
    fn real(&mut self, span: f64) -> f64 {
        (self.next() as f64 / (1u64 << 53) as f64 * 2.0 - 1.0) * span
    }

    /// An integer in `low..=high`.
    fn integer(&mut self, low: i128, high: i128) -> i128 {
        let span = u128::try_from(high - low + 1).expect("a range");
        low + i128::try_from(u128::from(self.next()) % span).expect("a span")
    }
}

/// One family: the declarations of its inputs and outputs, and its calls, each one statement or
/// more over input set `i`.
struct Family {
    name: &'static str,
    declarations: Vec<String>,
    calls: Vec<String>,
    /// The input sets.
    count: usize,
}

impl Family {
    fn new(name: &'static str, count: usize) -> Self {
        Self {
            name,
            declarations: Vec::new(),
            calls: Vec::new(),
            count,
        }
    }

    /// An input array `name` of `ty` holding `values`, ST literals.
    fn input(&mut self, name: &str, ty: &str, values: &[String]) {
        assert_eq!(values.len(), self.count, "{name}");
        self.declarations.push(format!(
            "    {name} : ARRAY[0..{}] OF {ty} := [{}];",
            self.count - 1,
            values.join(", ")
        ));
    }

    /// A call: `statement` writes output `o<n>` of `ty`, the next output's number.
    fn call(&mut self, ty: &str, statement: &str) {
        let output = format!("o{}", self.calls.len());
        self.declarations.push(format!("    {output} : {ty};"));
        self.calls.push(statement.replace("OUT", &output));
    }

    /// The program: scan `k` calls function `k MOD calls` on input set `k / calls`; `k` is retained,
    /// so that the warm restart after a fault goes on with the next call.
    fn program(&self) -> String {
        let functions = self.calls.len();
        let calls: Vec<String> = self
            .calls
            .iter()
            .enumerate()
            .map(|(number, call)| format!("    {number}: {call}"))
            .collect();
        format!(
            "PROGRAM Main\nVAR RETAIN\n    k : DINT := -1;\nEND_VAR\nVAR\n    i : DINT;\n{}\nEND_VAR\nk := k + 1;\ni := k / {functions};\nIF i < {} THEN\n    CASE k MOD {functions} OF\n{}\n    END_CASE;\nEND_IF;\nEND_PROGRAM\n",
            self.declarations.join("\n"),
            self.count,
            calls.join("\n")
        )
    }

    /// Runs the family compiled and in the runtime, one scan per call and input set.
    fn run(&self) {
        let source = self.program();
        let scans = i64::try_from(self.calls.len() * self.count).expect("scans");
        match compare_sources(self.name, &[SourceFile::new(source.clone())], 10, scans) {
            Ok(matched) => println!("{}: {matched} values matched over {scans} scans", self.name),
            Err(divergence) => panic!("{}: {divergence}\n{source}", self.name),
        }
    }
}

/// A `REAL` literal that reads back as `value` exactly.
fn real32(value: f32) -> String {
    format!("{value:.8e}")
}

/// An `LREAL` literal that reads back as `value` exactly.
fn real64(value: f64) -> String {
    format!("{value:.16e}")
}

/// The values a real function meets at its edges, then pseudo-random ones in `-span..span`.
fn reals(numbers: &mut Numbers, count: usize) -> Vec<f64> {
    let mut values = vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        -0.5,
        2.0,
        10.0,
        100.0,
        1.0e-30,
        -1.0e-30,
        1.0e30,
        3.4e38,
        -3.4e38,
        1.175_494_4e-38,
        88.7,
        89.0,
        709.7,
        710.0,
        std::f64::consts::PI,
        std::f64::consts::FRAC_PI_2,
        0.999_999,
        -0.999_999,
        1.000_001,
    ];
    while values.len() < count {
        let span = if values.len() % 2 == 0 { 1000.0 } else { 1.0 };
        values.push(numbers.real(span));
    }
    values.truncate(count);
    values
}

/// `SQRT`, `LN`, `LOG`, `EXP`, the trigonometric functions, `ATAN2`, `EXPT` and `ABS` of `REAL`s
/// and `LREAL`s (specification 07 §3): computed in binary64 by the same `libm`, a result that is
/// not finite an overflow, `SQRT` of a negative number and `LN` of 0 among them.
#[test]
fn real_functions_compute_as_the_runtime_computes_them() {
    let count = 40;
    let mut numbers = Numbers(7);
    let mut family = Family::new("reals", count);
    let x = reals(&mut numbers, count);
    let y: Vec<f64> = reals(&mut numbers, count).into_iter().rev().collect();
    #[allow(clippy::cast_possible_truncation)]
    let to32 = |values: &[f64]| -> Vec<String> {
        values.iter().map(|value| real32(*value as f32)).collect()
    };
    let to64 =
        |values: &[f64]| -> Vec<String> { values.iter().map(|value| real64(*value)).collect() };
    family.input("x", "REAL", &to32(&x));
    family.input("y", "REAL", &to32(&y));
    family.input("xl", "LREAL", &to64(&x));
    family.input("yl", "LREAL", &to64(&y));
    let exponents: Vec<String> = (0..count)
        .map(|index| (numbers.integer(-40, 40) + i128::try_from(index % 3).unwrap()).to_string())
        .collect();
    family.input("e", "INT", &exponents);
    for function in [
        "SQRT", "LN", "LOG", "EXP", "SIN", "COS", "TAN", "ASIN", "ACOS", "ATAN", "ABS",
    ] {
        family.call("REAL", &format!("OUT := {function}(x[i]);"));
        family.call("LREAL", &format!("OUT := {function}(xl[i]);"));
    }
    family.call("REAL", "OUT := ATAN2(x[i], y[i]);");
    family.call("LREAL", "OUT := ATAN2(xl[i], yl[i]);");
    family.call("REAL", "OUT := EXPT(x[i], y[i]);");
    family.call("REAL", "OUT := EXPT(x[i], e[i]);");
    family.call("LREAL", "OUT := EXPT(xl[i], yl[i]);");
    family.call("LREAL", "OUT := EXPT(xl[i], e[i]);");
    family.run();
}

/// The edges and pseudo-random values of an integer type of `bits` bits.
fn integers(numbers: &mut Numbers, count: usize, signed: bool, bits: u32) -> Vec<String> {
    let (low, high): (i128, i128) = if signed {
        (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
    } else if bits == 64 {
        // The runtime refuses a typed literal above the largest `LINT` ("integer literal out of
        // range").
        (0, i128::from(i64::MAX))
    } else {
        (0, (1i128 << bits) - 1)
    };
    // The smallest 64-bit value has no literal that reads as itself; one above it has.
    let low_literal = if bits == 64 && signed { low + 1 } else { low };
    let mut values = vec![
        0,
        1,
        2,
        3,
        7,
        10,
        100,
        high,
        high - 1,
        low_literal,
        low_literal + 1,
    ];
    if signed {
        values.extend([-1, -2, -3, -7, -100]);
    }
    while values.len() < count {
        let small = values.len() % 3 == 0;
        values.push(if small {
            numbers.integer(low.max(-50), high.min(50))
        } else {
            numbers.integer(low_literal, high)
        });
    }
    values.truncate(count);
    values.iter().map(ToString::to_string).collect()
}

/// The arithmetic, selection and comparison functions in their function forms on integers of five
/// widths (specification 07 §3, §5, §6): `ADD` of three inputs faults where an intermediate
/// overflows, `DIV` by zero faults, `MUX` refuses a selector outside its inputs naming the bounds,
/// and the comparisons chain. `MOD` is a keyword of the language: its function form is no ST.
#[test]
fn integer_functions_compute_as_the_runtime_computes_them() {
    let count = 24;
    let mut numbers = Numbers(11);
    let mut family = Family::new("integers", count);
    let selectors: Vec<String> = (0..count)
        .map(|_| numbers.integer(-1, 3).to_string())
        .collect();
    family.input("m", "INT", &selectors);
    let flags: Vec<String> = (0..count)
        .map(|index| if index % 2 == 0 { "TRUE" } else { "FALSE" }.to_string())
        .collect();
    family.input("g", "BOOL", &flags);
    for (ty, signed, bits) in [
        ("INT", true, 16),
        ("DINT", true, 32),
        ("LINT", true, 64),
        ("USINT", false, 8),
        ("UDINT", false, 32),
    ] {
        let lower = ty.to_ascii_lowercase();
        for suffix in ["a", "b", "c"] {
            let values = integers(&mut numbers, count, signed, bits);
            family.input(&format!("{lower}_{suffix}"), ty, &values);
        }
        let (a, b, c) = (
            format!("{lower}_a[i]"),
            format!("{lower}_b[i]"),
            format!("{lower}_c[i]"),
        );
        family.call(ty, &format!("OUT := ADD({a}, {b}, {c});"));
        family.call(ty, &format!("OUT := MUL({a}, {b});"));
        family.call(ty, &format!("OUT := SUB({a}, {b});"));
        family.call(ty, &format!("OUT := DIV({a}, {b});"));
        family.call(ty, &format!("OUT := MAX({a}, {b}, {c});"));
        family.call(ty, &format!("OUT := MIN({a}, {b}, {c});"));
        family.call(ty, &format!("OUT := LIMIT({a}, {b}, {c});"));
        family.call(
            ty,
            &format!("OUT := LIMIT(MX := {a}, IN := {b}, MN := {c});"),
        );
        family.call(ty, &format!("OUT := SEL(g[i], {a}, {b});"));
        family.call(ty, &format!("OUT := MUX(m[i], {a}, {b}, {c});"));
        family.call(ty, &format!("OUT := MOVE({a});"));
        family.call(ty, &format!("OUT := ABS({a});"));
        for comparison in ["GT", "GE", "EQ", "LE", "LT"] {
            family.call("BOOL", &format!("OUT := {comparison}({a}, {b}, {c});"));
        }
        family.call("BOOL", &format!("OUT := NE({a}, {b});"));
    }
    family.run();
}

/// `SHL`, `SHR`, `ROL` and `ROR` of the four bit strings and of a `BOOL` by signed and unsigned
/// counts (specification 07 §4): a count at or above the width shifts every bit out, a rotation
/// counts modulo the width, a negative count is a type mismatch.
#[test]
fn bit_functions_compute_as_the_runtime_computes_them() {
    let count = 24;
    let mut numbers = Numbers(13);
    let mut family = Family::new("bits", count);
    let counts: Vec<String> = (0..count)
        .map(|index| match index % 8 {
            0 => "-1".to_string(),
            1 => "0".to_string(),
            2 => "1".to_string(),
            3 => "8".to_string(),
            4 => "16".to_string(),
            5 => "64".to_string(),
            6 => "65".to_string(),
            _ => numbers.integer(2, 70).to_string(),
        })
        .collect();
    family.input("n", "INT", &counts);
    let unsigned_counts: Vec<String> = (0..count)
        .map(|_| numbers.integer(0, 130).to_string())
        .collect();
    family.input("u", "UINT", &unsigned_counts);
    for (ty, bits) in [("BYTE", 8), ("WORD", 16), ("DWORD", 32), ("LWORD", 64)] {
        let lower = ty.to_ascii_lowercase();
        for suffix in ["a", "b", "c"] {
            // Typed: an untyped literal for a bit string keeps the `DINT` range in the runtime.
            let values: Vec<String> = integers(&mut numbers, count, false, bits)
                .into_iter()
                .map(|value| format!("{ty}#{value}"))
                .collect();
            family.input(&format!("{lower}_{suffix}"), ty, &values);
        }
        let (a, b, c) = (
            format!("{lower}_a[i]"),
            format!("{lower}_b[i]"),
            format!("{lower}_c[i]"),
        );
        for shift in ["SHL", "SHR", "ROL", "ROR"] {
            family.call(ty, &format!("OUT := {shift}({a}, n[i]);"));
            family.call(ty, &format!("OUT := {shift}(IN := {a}, N := u[i]);"));
        }
        // `AND`, `OR`, `XOR` and `NOT` are keywords of the language: their function forms are
        // no ST, and the operators are compared by the examples.
        let _ = (b, c);
    }
    let booleans: Vec<String> = (0..count)
        .map(|index| if index % 3 == 0 { "TRUE" } else { "FALSE" }.to_string())
        .collect();
    family.input("p", "BOOL", &booleans);
    family.call("BOOL", "OUT := SHL(p[i], u[i]);");
    family.call("BOOL", "OUT := ROR(p[i], n[i]);");
    family.run();
}

/// `LEN`, `LEFT`, `RIGHT`, `MID`, `INSERT`, `DELETE`, `REPLACE`, `FIND`, `CONCAT` and the
/// characters of a string, read and written (specification 02 §6, 07 §8): positions and lengths
/// count characters in texts with characters of two and three bytes, from below one to past the
/// end, and a character above 255 read as a `CHAR` overflows.
#[test]
fn string_functions_compute_as_the_runtime_computes_them() {
    let count = 24;
    let mut numbers = Numbers(17);
    let mut family = Family::new("strings", count);
    let texts = [
        "",
        "A",
        "ABCDE",
        "Grüße",
        "xyz",
        "äöü",
        "Hello World",
        "€uro",
        "CA",
        "ABCABC",
        "ß",
        "  pad  ",
    ];
    let pick = |numbers: &mut Numbers| {
        let index = usize::try_from(numbers.next() % texts.len() as u64).unwrap();
        format!("'{}'", texts[index])
    };
    let first: Vec<String> = (0..count).map(|_| pick(&mut numbers)).collect();
    let second: Vec<String> = (0..count).map(|_| pick(&mut numbers)).collect();
    family.input("s", "STRING[20]", &first);
    family.input("t", "STRING[20]", &second);
    // Texts whose characters a `CHAR` holds, for reading characters: the runtime reports a
    // character above 255 read as a `CHAR` as a null dereference, where specification 02 §6 makes
    // it an overflow (specification 44 §14.2).
    let narrow: Vec<String> = first.iter().map(|text| text.replace('€', "E")).collect();
    family.input("cs", "STRING[20]", &narrow);
    let lengths: Vec<String> = (0..count)
        .map(|_| numbers.integer(-2, 14).to_string())
        .collect();
    let positions: Vec<String> = (0..count)
        .map(|_| numbers.integer(-1, 13).to_string())
        .collect();
    family.input("l", "INT", &lengths);
    family.input("p", "INT", &positions);
    // Printable ASCII characters, the literal's own `$` and quote aside: a `CHAR` initializer above
    // 127 is refused; characters of two bytes come from the texts.
    let characters: Vec<String> = (0..count)
        .map(|_| loop {
            let code = u32::try_from(numbers.integer(32, 126)).unwrap();
            if code == u32::from(b'$') || code == u32::from(b'\'') {
                continue;
            }
            break format!("CHAR#'{}'", char::from_u32(code).unwrap());
        })
        .collect();
    family.input("ch", "CHAR", &characters);
    family.declarations.push("    w : STRING[20];".into());
    family.declarations.push("    tight : STRING[5];".into());
    family.call("INT", "OUT := LEN(s[i]);");
    family.call("STRING[40]", "OUT := LEFT(s[i], l[i]);");
    family.call("STRING[40]", "OUT := RIGHT(s[i], l[i]);");
    family.call("STRING[40]", "OUT := MID(s[i], l[i], p[i]);");
    family.call(
        "STRING[40]",
        "OUT := MID(P := p[i], IN := s[i], L := l[i]);",
    );
    family.call("STRING[40]", "OUT := INSERT(s[i], t[i], p[i]);");
    family.call("STRING[40]", "OUT := DELETE(s[i], l[i], p[i]);");
    family.call("STRING[40]", "OUT := REPLACE(s[i], t[i], l[i], p[i]);");
    family.call("INT", "OUT := FIND(s[i], t[i]);");
    family.call("STRING[40]", "OUT := CONCAT(s[i], '-', t[i]);");
    family.call("STRING[8]", "OUT := INSERT(s[i], t[i], p[i]);");
    family.call("CHAR", "OUT := cs[i][p[i]];");
    family.call("BOOL", "w := s[i]; w[p[i]] := ch[i]; OUT := TRUE;");
    family.call("BOOL", "tight := s[i]; tight[p[i]] := ch[i]; OUT := TRUE;");
    family.call(
        "BOOL",
        "tight := t[i]; tight[p[i]] := cs[i][1]; OUT := TRUE;",
    );
    family.call("INT", "OUT := LEN(CONCAT(LEFT(s[i], 2), t[i]));");
    family.call("BOOL", "OUT := LEFT(s[i], l[i]) = t[i];");
    family.run();
}

/// The counters in their five count types, `SR`, `RS`, `R_TRIG`, `F_TRIG` and Mitsubishi's `DIFU`
/// and `DIFD` (specification 08): each instance is called every scan with that scan's inputs, the
/// count inputs toggling often, the reset and the load seldom, and a load near a type's limits so
/// that a counter saturates there instead of wrapping.
#[test]
fn standard_function_blocks_run_as_the_runtime_runs_them() {
    let count = 120;
    let mut numbers = Numbers(19);
    let mut family = Family::new("blocks", count);
    let flags = |numbers: &mut Numbers, percent: u64| -> Vec<String> {
        (0..count)
            .map(|_| {
                if numbers.next() % 100 < percent {
                    "TRUE"
                } else {
                    "FALSE"
                }
                .to_string()
            })
            .collect()
    };
    let up = flags(&mut numbers, 50);
    let down = flags(&mut numbers, 50);
    let reset = flags(&mut numbers, 5);
    let load = flags(&mut numbers, 8);
    family.input("cu", "BOOL", &up);
    family.input("cd", "BOOL", &down);
    family.input("r", "BOOL", &reset);
    family.input("ld", "BOOL", &load);
    let mut body = Vec::new();
    for (block, ty, signed, bits) in [
        ("INT", "INT", true, 16u32),
        ("DINT", "DINT", true, 32),
        ("LINT", "LINT", true, 64),
        ("UDINT", "UDINT", false, 32),
        ("ULINT", "ULINT", false, 64),
    ] {
        let lower = block.to_ascii_lowercase();
        // Presets of a few counts, and ones at a limit of the type, which a load brings the
        // count to.
        let (low, high): (i128, i128) = if signed {
            (-(1i128 << (bits - 1)) + 1, (1i128 << (bits - 1)) - 1)
        } else if bits == 64 {
            (0, i128::from(i64::MAX))
        } else {
            (0, (1i128 << bits) - 1)
        };
        let presets: Vec<String> = (0..count)
            .map(|index| match index % 5 {
                0 => (high - 1).to_string(),
                1 => (low + 1).to_string(),
                _ => numbers.integer(0, 6).to_string(),
            })
            .collect();
        family.input(&format!("pv_{lower}"), ty, &presets);
        let suffix = if block == "INT" {
            String::new()
        } else {
            format!("_{block}")
        };
        for (name, kind) in [("u", "CTU"), ("d", "CTD"), ("ud", "CTUD")] {
            family
                .declarations
                .push(format!("    {name}_{lower} : {kind}{suffix};"));
        }
        body.push(format!(
            "u_{lower}(CU := cu[i], R := r[i], PV := pv_{lower}[i]);"
        ));
        body.push(format!(
            "d_{lower}(CD := cd[i], LD := ld[i], PV := pv_{lower}[i]);"
        ));
        body.push(format!(
            "ud_{lower}(CU := cu[i], CD := cd[i], R := r[i], LD := ld[i], PV := pv_{lower}[i]);"
        ));
    }
    for declaration in [
        "    sr1 : SR;",
        "    rs1 : RS;",
        "    rising : R_TRIG;",
        "    falling : F_TRIG;",
        "    difu1 : DIFU;",
        "    difd1 : DIFD;",
    ] {
        family.declarations.push(declaration.into());
    }
    body.push("sr1(S1 := cu[i], R := cd[i]);".into());
    body.push("rs1(S := cu[i], R1 := cd[i]);".into());
    body.push("rising(CLK := cu[i]);".into());
    body.push("falling(CLK := cu[i]);".into());
    body.push("difu1(CLK := cd[i]);".into());
    body.push("difd1(CLK := cd[i]);".into());
    // One call: every block runs every scan, so that its state follows the inputs.
    family.call("BOOL", &format!("{} OUT := TRUE;", body.join(" ")));
    family.run();
}

/// A date-time literal `prefix#` of `millis` milliseconds since 1970-01-01: `DT#2026-10-23-12:34:56.789`.
fn date_time(prefix: &str, millis: i64) -> String {
    let days = millis.div_euclid(86_400_000);
    let of_day = millis.rem_euclid(86_400_000);
    // Hinnant's civil_from_days, as in the runtime.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + i64::from(m <= 2);
    let (hour, minute, second, milli) = (
        of_day / 3_600_000,
        of_day / 60_000 % 60,
        of_day / 1000 % 60,
        of_day % 1000,
    );
    match prefix {
        "DATE" | "LDATE" => format!("{prefix}#{year:04}-{m:02}-{d:02}"),
        "TOD" | "LTOD" => format!("{prefix}#{hour:02}:{minute:02}:{second:02}.{milli:03}"),
        _ => {
            format!("{prefix}#{year:04}-{m:02}-{d:02}-{hour:02}:{minute:02}:{second:02}.{milli:03}")
        }
    }
}

/// The date and time operators and functions (specification 07 §9; IEC Tables 36 and 37): a
/// duration added to a time of day or a date and time in whole milliseconds, dates and times taken
/// apart, durations scaled by integers and reals, the calendar built from components and split
/// into them, the day of the week, and the conversions between the date and time types, on dates
/// from 1900 to 2200, near midnight and with durations that cross days.
#[test]
fn date_and_time_functions_compute_as_the_runtime_computes_them() {
    let count = 48;
    let mut numbers = Numbers(23);
    let mut family = Family::new("dates", count);
    let millis: Vec<i64> = (0..count)
        .map(|index| match index % 6 {
            0 => 0,
            1 => 86_399_999,
            2 => -1,
            _ => i64::try_from(numbers.integer(-2_208_988_800_000, 7_258_118_400_000)).unwrap(),
        })
        .collect();
    let other: Vec<i64> = millis.iter().rev().copied().collect();
    let dt = |prefix: &str, values: &[i64]| -> Vec<String> {
        values
            .iter()
            .map(|value| date_time(prefix, *value))
            .collect()
    };
    family.input("stamp", "DT", &dt("DT", &millis));
    family.input("stamp2", "DT", &dt("DT", &other));
    family.input("lstamp", "LDT", &dt("LDT", &millis));
    family.input("day", "DATE", &dt("DATE", &millis));
    family.input("day2", "DATE", &dt("DATE", &other));
    family.input("clock", "TOD", &dt("TOD", &millis));
    family.input("clock2", "TOD", &dt("TOD", &other));
    family.input("lclock", "LTOD", &dt("LTOD", &millis));
    let durations: Vec<String> = (0..count)
        .map(|index| match index % 5 {
            0 => "T#0ms".to_string(),
            1 => "T#1d".to_string(),
            2 => "T#-1h".to_string(),
            _ => format!("T#{}ms", numbers.integer(-100_000_000, 100_000_000)),
        })
        .collect();
    family.input("span", "TIME", &durations);
    let long: Vec<String> = durations.iter().map(|text| format!("L{text}")).collect();
    family.input("lspan", "LTIME", &long);
    let factors: Vec<String> = (0..count)
        .map(|_| numbers.integer(-5, 5).to_string())
        .collect();
    family.input("n", "DINT", &factors);
    let unsigned: Vec<String> = (0..count)
        .map(|_| numbers.integer(0, 9).to_string())
        .collect();
    family.input("un", "UDINT", &unsigned);
    let reals: Vec<String> = reals(&mut numbers, count)
        .into_iter()
        .map(|value| real64(value.clamp(-1e6, 1e6)))
        .collect();
    family.input("r", "LREAL", &reals);
    for (name, ty, low, high) in [
        ("yr", "INT", 1899, 2201),
        ("mo", "INT", 0, 13),
        ("dy", "INT", 0, 32),
        ("hr", "INT", -1, 24),
        ("mi", "INT", -1, 60),
        ("se", "INT", -1, 60),
        ("ms", "INT", -1, 1000),
    ] {
        let values: Vec<String> = (0..count)
            .map(|_| numbers.integer(low, high).to_string())
            .collect();
        family.input(name, ty, &values);
    }
    family.call("DT", "OUT := stamp[i] + span[i];");
    family.call("DT", "OUT := stamp[i] - span[i];");
    family.call("TOD", "OUT := clock[i] + span[i];");
    family.call("TOD", "OUT := clock[i] - span[i];");
    family.call("LDT", "OUT := lstamp[i] + lspan[i];");
    family.call("LTOD", "OUT := lclock[i] - lspan[i];");
    family.call("TIME", "OUT := day[i] - day2[i];");
    family.call("TIME", "OUT := clock[i] - clock2[i];");
    family.call("TIME", "OUT := stamp[i] - stamp2[i];");
    family.call("TIME", "OUT := span[i] * n[i];");
    family.call("TIME", "OUT := span[i] / n[i];");
    family.call("TIME", "OUT := span[i] * un[i];");
    family.call("TIME", "OUT := span[i] * r[i];");
    family.call("TIME", "OUT := span[i] / r[i];");
    family.call("TIME", "OUT := ADD_TIME(span[i], span[i]);");
    family.call("TOD", "OUT := ADD_TOD_TIME(clock[i], span[i]);");
    family.call("DT", "OUT := SUB_DT_TIME(stamp[i], span[i]);");
    family.call("TIME", "OUT := SUB_DT_DT(stamp[i], stamp2[i]);");
    family.call("TIME", "OUT := MUL_TIME(span[i], n[i]);");
    family.call("TIME", "OUT := DIV_TIME(span[i], n[i]);");
    family.call("DT", "OUT := CONCAT_DATE_TOD(day[i], clock[i]);");
    family.call("LDT", "OUT := CONCAT_DATE_LTOD(day[i], lclock[i]);");
    family.call("DATE", "OUT := CONCAT_DATE(yr[i], mo[i], dy[i]);");
    family.call("TOD", "OUT := CONCAT_TOD(hr[i], mi[i], se[i], ms[i]);");
    family.call(
        "DT",
        "OUT := CONCAT_DT(yr[i], mo[i], dy[i], hr[i], mi[i], se[i], ms[i]);",
    );
    family.call("INT", "OUT := DAY_OF_WEEK(day[i]);");
    family.call("DATE", "OUT := DT_TO_DATE(stamp[i]);");
    family.call("TOD", "OUT := DT_TO_TOD(stamp[i]);");
    family.call("LDT", "OUT := DT_TO_LDT(stamp[i]);");
    family.call("DT", "OUT := LDT_TO_DT(lstamp[i]);");
    family.call("LTOD", "OUT := TOD_TO_LTOD(clock[i]);");
    family.call("TOD", "OUT := LTOD_TO_TOD(lclock[i]);");
    family.call("TIME", "OUT := span[i] * 3;");
    family.call("TIME", "OUT := span[i] / 2.5;");
    family.call("DATE", "OUT := LDT_TO_DATE(lstamp[i]);");
    family.call("TOD", "OUT := LDT_TO_TOD(lstamp[i]);");
    family.call("LTOD", "OUT := LDT_TO_LTOD(lstamp[i]);");
    family.call("LTOD", "OUT := DT_TO_LTOD(stamp[i]);");
    family.call("LTIME", "OUT := lstamp[i] - DT_TO_LDT(stamp2[i]);");
    family.call("LTOD", "OUT := CONCAT_LTOD(hr[i], mi[i], se[i], ms[i]);");
    family.call(
        "LDT",
        "OUT := CONCAT_LDT(yr[i], mo[i], dy[i], hr[i], mi[i], se[i], ms[i]);",
    );
    family.call(
        "DATE",
        "OUT := CONCAT_DATE(DAY := dy[i], MONTH := mo[i], YEAR := yr[i]);",
    );
    family.call("BOOL", "OUT := stamp[i] < stamp2[i];");
    family.call("BOOL", "OUT := clock[i] >= clock2[i];");
    family.call("DT", "OUT := DT#2026-10-23-12:00:00 + span[i];");
    family.call("TOD", "OUT := TOD#23:59:59.999 + span[i];");
    family
        .declarations
        .push("    y : INT; m : USINT; dd : DINT; h : INT; mm : INT; s : INT; f : INT;".into());
    family.call(
        "BOOL",
        "SPLIT_DATE(IN := day[i], YEAR => y, MONTH => m, DAY => dd); OUT := TRUE;",
    );
    family.call(
        "BOOL",
        "SPLIT_DT(stamp[i], y, m, dd, h, mm, s, f); OUT := TRUE;",
    );
    family.call(
        "BOOL",
        "SPLIT_TOD(IN := clock[i], HOUR => h, MINUTE => mm, SECOND => s, MILLISECOND => f); OUT := TRUE;",
    );
    family.call(
        "BOOL",
        "SPLIT_LDT(lstamp[i], y, m, dd, h, mm, s, f); OUT := TRUE;",
    );
    family.call(
        "BOOL",
        "SPLIT_LTOD(IN := lclock[i], HOUR => h, MINUTE => mm, SECOND => s, MILLISECOND => f); OUT := TRUE;",
    );
    family.run();
}

/// The conversions to and from texts, characters and BCD (specification 07 §2.8; IEC Tables 22
/// to 27): reals spelled as the runtime spells them, texts read with their white space, separators,
/// bases and signs or refused, characters by their codes, and BCD digits, compiled and in the
/// runtime in lockstep.
#[test]
fn text_conversions_compute_as_the_runtime_computes_them() {
    let spelled = [
        "' 16#FF '",
        "'1_000'",
        "'-128'",
        "'127'",
        "'128'",
        "'+5'",
        "'--5'",
        "'_1'",
        "'1e3'",
        "''",
        "'2#1010_1010'",
        "'8#-17'",
        "'36#Z'",
        "'37#1'",
        "'0'",
        "'-0'",
        "'99999999999999999999'",
        "'-9223372036854775808'",
        "'18446744073709551615'",
        "'1.5'",
        "' 2.5E2 '",
        "'1_000.25'",
        "'-0.0'",
        "'1e400'",
        "'abc'",
        "'3.5E38'",
        "'1.0E-50'",
        "'NaN'",
        "'inf'",
        "'A'",
        "'é'",
        "'AB'",
        "' '",
        "'0x10'",
        "'1 000'",
        "'$N42$T'",
        "'42 '",
        "'4_2'",
        "'16#_F'",
        "'16#F_'",
    ];
    let count = spelled.len();
    let mut numbers = Numbers(31);
    let mut family = Family::new("texts", count);
    family.input(
        "s",
        "STRING[40]",
        &spelled
            .iter()
            .map(|text| (*text).to_string())
            .collect::<Vec<_>>(),
    );
    let real_values = reals(&mut numbers, count);
    family.input(
        "r",
        "LREAL",
        &real_values
            .iter()
            .map(|value| real64(*value))
            .collect::<Vec<_>>(),
    );
    #[allow(clippy::cast_possible_truncation)]
    let narrow: Vec<String> = real_values
        .iter()
        .map(|value| real32(value.clamp(-3.0e38, 3.0e38) as f32))
        .collect();
    family.input("f", "REAL", &narrow);
    let codes: Vec<String> = (0..count)
        .map(|_| numbers.integer(-10, 300).to_string())
        .collect();
    family.input("n", "DINT", &codes);
    // ASCII only: the runtime refuses a `CHAR` initial value above 127 (checklist); the codes above
    // come from `DINT_TO_CHAR`.
    let characters: Vec<String> = ["'A'", "'z'", "'#'", "' '", "'$$'", "'0'", "'$''", "'~'"]
        .iter()
        .cycle()
        .take(count)
        .map(|text| (*text).to_string())
        .collect();
    family.input("c", "CHAR", &characters);
    let small: Vec<String> = (0..count)
        .map(|_| numbers.integer(0, 255).to_string())
        .collect();
    family.input("us", "USINT", &small);
    let bytes: Vec<String> = (0..count)
        .map(|_| numbers.integer(0, 255).to_string())
        .collect();
    family.input("b", "BYTE", &bytes);
    let words: Vec<String> = (0..count)
        .map(|_| numbers.integer(0, 65_535).to_string())
        .collect();
    family.input("ui", "UINT", &words);
    family.input("w", "WORD", &words);
    family.call("STRING[400]", "OUT := LREAL_TO_STRING(r[i]);");
    family.call("STRING[400]", "OUT := REAL_TO_STRING(f[i]);");
    family.call("STRING[8]", "OUT := LREAL_TO_STRING(r[i]);");
    family.call("DINT", "OUT := STRING_TO_DINT(s[i]);");
    family.call("SINT", "OUT := STRING_TO_SINT(s[i]);");
    family.call("ULINT", "OUT := STRING_TO_ULINT(s[i]);");
    family.call("LINT", "OUT := STRING_TO_LINT(s[i]);");
    family.call("LREAL", "OUT := STRING_TO_LREAL(s[i]);");
    family.call("REAL", "OUT := STRING_TO_REAL(s[i]);");
    family.call("CHAR", "OUT := STRING_TO_CHAR(s[i]);");
    family.call("CHAR", "OUT := DINT_TO_CHAR(n[i]);");
    family.call("DINT", "OUT := CHAR_TO_DINT(c[i]);");
    family.call("BYTE", "OUT := CHAR_TO_BYTE(c[i]);");
    family.call("STRING[4]", "OUT := CHAR_TO_STRING(c[i]);");
    family.call("STRING[4]", "OUT := CHAR_TO_STRING(DINT_TO_CHAR(n[i]));");
    family.call("BYTE", "OUT := USINT_TO_BCD_BYTE(us[i]);");
    family.call("USINT", "OUT := BYTE_BCD_TO_USINT(b[i]);");
    family.call("WORD", "OUT := UINT_TO_BCD_WORD(ui[i]);");
    family.call("UINT", "OUT := WORD_BCD_TO_UINT(w[i]);");
    family.run();
}

/// Literals with a sign take the type of where they go, as the runtime lowers them (specification
/// 05 §2): `REAL := -273.15` rounds the binary64 value -273.15 once to binary32, an integer literal
/// with a sign goes to a `REAL` or a narrower integer, and a signed literal as an operand takes the
/// operation's type.
#[test]
fn signed_literals_take_the_type_of_where_they_go() {
    let count = 6;
    let mut family = Family::new("signed literals", count);
    let values: Vec<String> = ["0.5", "-1.25", "3.0", "-0.0", "100.0", "-7.5"]
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    family.input("x", "REAL", &values);
    family.call("REAL", "OUT := -273.15;");
    family.call("LREAL", "OUT := -273.15;");
    family.call("REAL", "OUT := -1;");
    family.call("INT", "OUT := -5;");
    family.call("DINT", "OUT := +7;");
    family.call("SINT", "OUT := -128;");
    family.call("REAL", "OUT := x[i] * -2.5;");
    family.call("REAL", "OUT := x[i] - -1.0E-3;");
    family.call("LREAL", "OUT := -1.60217653E-19;");
    family.run();
}

/// A CASE whose selector is a text or a real compares it with each label in source order, as `=`
/// compares them, and runs the first branch that matches, else its ELSE, else nothing
/// (specification 06 §6; IEC §7.3.3.3.3): `'B'` takes the branch of `'B', 'C'`, `''` the empty
/// label, `'AB'` and `'b'` the ELSE; `-0.0` equals the label `0.0`, `2.5` and `3.25` take the
/// branch of `2.5, 3.25`, and `3.0` matches no label. The selector is read once, so a branch that
/// changes it runs alone. Catches a text compared by its length or its first byte, a branch taken
/// out of source order, an ELSE taken although a label matched or missing when none did, and a
/// real compared by its bits. (The runtime compiles no range of reals, which trust-hir accepts.)
#[test]
fn case_selects_by_texts_and_reals() {
    let count = 6;
    let mut family = Family::new("case selectors", count);
    let texts: Vec<String> = ["'A'", "'B'", "'C'", "''", "'AB'", "'b'"]
        .iter()
        .map(|text| (*text).to_string())
        .collect();
    family.input("s", "STRING[4]", &texts);
    let reals: Vec<String> = ["1.5", "2.5", "-0.0", "0.0", "3.25", "3.0"]
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    family.input("x", "REAL", &reals);
    family.call(
        "INT",
        "CASE s[i] OF 'A': OUT := 1; 'B', 'C': OUT := 2; '': OUT := 4; ELSE OUT := 3; END_CASE;",
    );
    family.call("INT", "OUT := 9; CASE s[i] OF 'B': OUT := 5; END_CASE;");
    family.call(
        "INT",
        "CASE s[i] OF 'A': s[i] := 'B'; OUT := 1; 'B': OUT := 2; ELSE OUT := 3; END_CASE;",
    );
    family.call(
        "INT",
        "CASE x[i] OF 1.5: OUT := 1; 2.5, 3.25: OUT := 2; 0.0: OUT := 3; ELSE OUT := 4; END_CASE;",
    );
    family.run();
}

/// Texts made of literals only compile and give the runtime's values: concatenations of literals,
/// nested too, a comparison of two literals, a text compared with a concatenation of literals, the
/// length of one, and one cut at its destination's capacity. A module holds no writable data
/// (specification 44 §5.3), so a text of literals is written by a call that takes its bytes; a
/// constant array of literal pieces was promoted to a static whose pointers needed relocations, and
/// the build refused the module for writable data. Catches that refusal, and literals joined in
/// the wrong order.
#[test]
fn texts_of_literals_only_compile_and_match_the_runtime() {
    let count = 4;
    let mut family = Family::new("literal texts", count);
    let texts: Vec<String> = ["'AB'", "'NE'", "''", "'B'"]
        .iter()
        .map(|text| (*text).to_string())
        .collect();
    family.input("s", "STRING[4]", &texts);
    family.call("STRING[4]", "OUT := CONCAT('N', 'E');");
    family.call("STRING[8]", "OUT := CONCAT('a', CONCAT('b', 'c'));");
    family.call("BOOL", "OUT := 'AB' > 'B';");
    family.call("BOOL", "OUT := s[i] = CONCAT('N', 'E');");
    family.call("INT", "OUT := LEN(CONCAT('xy', 'z'));");
    family.call("STRING[2]", "OUT := CONCAT('abc', 'def');");
    family.run();
}

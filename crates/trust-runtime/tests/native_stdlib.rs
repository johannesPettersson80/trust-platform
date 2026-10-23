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

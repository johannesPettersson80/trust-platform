//! The duration of an IEC 61131-3 duration literal's text (IEC §6.3.4, Table 8), for a value written outside the code, such as
//! a pragma's attribute (specification 22, `test_cycle`) or an HMI setting.

/// The nanoseconds of an IEC 61131-3 duration literal such as `T#1h30m` or `TIME#500ms`.
pub fn duration_nanos(text: &str) -> Option<i64> {
    let upper = text.trim().to_ascii_uppercase();
    let (prefix, body) = upper.split_once('#')?;
    if !matches!(prefix, "T" | "TIME" | "LT" | "LTIME") {
        return None;
    }
    let (sign, mut rest) = match body.strip_prefix('-') {
        Some(rest) => (-1.0, rest),
        None => (1.0, body.strip_prefix('+').unwrap_or(body)),
    };
    let mut total = 0.0;
    let mut parts = 0;
    while !rest.is_empty() {
        let digits = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '_'))
            .unwrap_or(rest.len());
        let number: f64 = rest[..digits].replace('_', "").parse().ok()?;
        rest = &rest[digits..];
        let letters = rest
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(rest.len());
        let nanos = match &rest[..letters] {
            "D" => 8.64e13,
            "H" => 3.6e12,
            "M" => 6.0e10,
            "S" => 1.0e9,
            "MS" => 1.0e6,
            "US" => 1.0e3,
            "NS" => 1.0,
            _ => return None,
        };
        total += number * nanos;
        parts += 1;
        rest = rest[letters..].trim_start_matches('_');
    }
    (parts > 0).then(|| (sign * total).round() as i64)
}

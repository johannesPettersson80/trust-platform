//! The standard functions (IEC §6.6.2.5; specification 07; specification 44 §6.8) by family:
//! numerical, bit-shift, bitwise, selection, comparison and arithmetic functions in their function
//! forms, `MOVE`, and the type conversions; the string functions are in `strings`. Each takes its
//! inputs as the runtime takes them, evaluated in their order, and gives its result and its faults
//! as the runtime's implementation does.

use trust_syntax::syntax::SyntaxNode;

use super::call::{Arg, Called, Want};
use super::expr::Text;
use super::{is_scalar, Lowerer};
use crate::native::types::{Elem, Kind, Ty};
use crate::native::NativeError;

/// The formal parameters of a standard function of fixed inputs, in their order
/// (`trust-runtime-core`'s registration).
fn fixed_formals(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "ABS" | "SQRT" | "LN" | "LOG" | "EXP" | "SIN" | "COS" | "TAN" | "ASIN" | "ACOS"
        | "ATAN" | "MOVE" | "NOT" => &["IN"],
        "ATAN2" => &["Y", "X"],
        "SUB" | "DIV" | "MOD" | "EXPT" | "NE" => &["IN1", "IN2"],
        "SHL" | "SHR" | "ROL" | "ROR" => &["IN", "N"],
        "SEL" => &["G", "IN0", "IN1"],
        "LIMIT" => &["MN", "IN", "MX"],
        _ if name.contains("_TO_") || name.starts_with("TO_") || name.starts_with("TRUNC") => {
            &["IN"]
        }
        _ => return None,
    })
}

/// The real function of `name`: its `lib::Math` variant.
fn math_function(name: &str) -> Option<&'static str> {
    Some(match name {
        "SQRT" => "Sqrt",
        "LN" => "Ln",
        "LOG" => "Log",
        "EXP" => "Exp",
        "SIN" => "Sin",
        "COS" => "Cos",
        "TAN" => "Tan",
        "ASIN" => "Asin",
        "ACOS" => "Acos",
        "ATAN" => "Atan",
        _ => return None,
    })
}

impl Lowerer<'_, '_> {
    /// A standard function by its name in capitals.
    pub(super) fn standard(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        args: Vec<Arg>,
        want: Want,
    ) -> Result<Called, NativeError> {
        if let Some(formals) = super::strings::text_function(name) {
            return self.call_text_function(node, name, formals, args, &want);
        }
        let values = self.standard_arguments(node, name, args)?;
        if name == "CONCAT" {
            let mut parts = Vec::new();
            for value in &values {
                parts.extend(self.text(value)?);
            }
            return Ok(Called::Text(parts));
        }
        if name == "TIME" && values.is_empty() {
            return Ok(Called::Scalar("cx.now_ns".into()));
        }
        if name.contains("_TO_") || name.starts_with("TO_") || name == "TRUNC" {
            let ty = self.type_of(node)?;
            let [value] = values.as_slice() else {
                return Err(self.unsupported(node, format!("{name} with other than one input")));
            };
            return self.conversion(node, name, value, ty);
        }
        if let Some(function) = math_function(name) {
            return self.real_function(node, function, &values, &want);
        }
        match name {
            "MOVE" => self.move_value(node, &values, &want),
            "EXPT" => self.expt(node, &values, &want),
            "ATAN2" => self.atan2(node, &values, &want),
            "SHL" | "SHR" | "ROL" | "ROR" => self.shift(node, name, &values, &want),
            "MUX" => self.mux(node, &values, &want),
            "GT" | "GE" | "EQ" | "LE" | "LT" | "NE" => {
                self.compare_function(node, name, &values, &want)
            }
            "ADD" | "MUL" | "SUB" | "DIV" | "MOD" => {
                self.arithmetic_function(node, name, &values, &want)
            }
            "AND" | "OR" | "XOR" | "NOT" => self.bitwise_function(node, name, &values, &want),
            _ => self.selection(node, name, &values, &want),
        }
    }

    /// The arguments of standard function `name` in the order of its formal parameters
    /// (specification 07 §1): positional ones as they come; formal ones by name, `IN1`, `IN2`, …
    /// of an extensible function by their numbers and `K` of `MUX` first.
    fn standard_arguments(
        &self,
        node: &SyntaxNode,
        name: &str,
        args: Vec<Arg>,
    ) -> Result<Vec<SyntaxNode>, NativeError> {
        if args.iter().all(|arg| arg.name.is_none()) {
            return Ok(args.into_iter().map(|arg| arg.value).collect());
        }
        if let Some(formals) = fixed_formals(name) {
            return self.formal_order(node, name, formals, args);
        }
        // An extensible function: `MUX`'s `K` first, then the inputs by their numbers.
        let mut keyed = Vec::with_capacity(args.len());
        for arg in args {
            let Some(formal) = arg.name.as_deref() else {
                return Err(self.unsupported(
                    &arg.value,
                    format!("{name} with formal and positional inputs"),
                ));
            };
            let upper = formal.to_ascii_uppercase();
            let key = if name == "MUX" && upper == "K" {
                Some(-1)
            } else {
                upper
                    .strip_prefix("IN")
                    .and_then(|number| number.parse::<i64>().ok())
            };
            let Some(key) = key else {
                return Err(self.unsupported(&arg.value, format!("the input {formal} of {name}")));
            };
            keyed.push((key, arg.value));
        }
        keyed.sort_by_key(|(key, _)| *key);
        Ok(keyed.into_iter().map(|(_, value)| value).collect())
    }

    /// The common type of `operands`, the widest, as the runtime's functions take their inputs
    /// (specification 07 §6).
    fn common_of(&mut self, node: &SyntaxNode, operands: &[SyntaxNode]) -> Result<Ty, NativeError> {
        let Some((first, rest)) = operands.split_first() else {
            return Err(self.unsupported(node, "a standard function without inputs"));
        };
        let mut ty = self.operand_type(first)?;
        for operand in rest {
            let other = self.operand_type(operand)?;
            ty = self.common(node, ty, other)?;
        }
        Ok(ty)
    }

    /// Each of `values` as a scalar of type `ty`, in order, each in a temporary.
    fn scalars_of(&mut self, values: &[SyntaxNode], ty: Ty) -> Result<Vec<String>, NativeError> {
        let mut scalars = Vec::with_capacity(values.len());
        for value in values {
            let scalar = self.scalar_to(value, ty)?;
            scalars.push(self.bind(&scalar));
        }
        Ok(scalars)
    }

    /// The scalar result `value` of type `ty`, as `want` needs it.
    fn result(
        &mut self,
        node: &SyntaxNode,
        value: &str,
        ty: Ty,
        want: &Want,
    ) -> Result<Called, NativeError> {
        let value = self.bind(value);
        match want {
            Want::Scalar(target) => Ok(Called::Scalar(self.convert(node, &value, ty, *target)?)),
            _ => Ok(Called::Scalar(value)),
        }
    }

    /// The elementary type of `ty`, refused for a type that is no scalar.
    fn elem_of(&self, node: &SyntaxNode, ty: Ty, name: &str) -> Result<Elem, NativeError> {
        self.info(ty)
            .scalar()
            .ok_or_else(|| self.unsupported(node, format!("{name} of values that are not scalars")))
    }

    /// `SQRT`, `LN`, `LOG`, `EXP` and the trigonometric functions of a `REAL` or an `LREAL`,
    /// computed in binary64 (specification 07 §3).
    fn real_function(
        &mut self,
        node: &SyntaxNode,
        function: &str,
        values: &[SyntaxNode],
        want: &Want,
    ) -> Result<Called, NativeError> {
        let ty = self.type_of(node)?;
        let elem = self.elem_of(node, ty, function)?;
        let [value] = values else {
            return Err(self.unsupported(node, format!("{function} with other than one input")));
        };
        let library = match elem {
            Elem::Real => "real32",
            Elem::LReal => "real64",
            _ => return Err(self.unsupported(node, format!("{function} of a {}", elem.name()))),
        };
        let value = self.scalar_to(value, ty)?;
        self.result(
            node,
            &format!("lib::{library}(cx, lib::Math::{function}, {value})?"),
            ty,
            want,
        )
    }

    /// `EXPT(IN1, IN2)`: a real base to any number's power, in binary64.
    fn expt(
        &mut self,
        node: &SyntaxNode,
        values: &[SyntaxNode],
        want: &Want,
    ) -> Result<Called, NativeError> {
        let [base, exponent] = values else {
            return Err(self.unsupported(node, "EXPT with other than two inputs"));
        };
        let ty = self.type_of(base)?;
        let library = match self.elem_of(node, ty, "EXPT")? {
            Elem::Real => "expt32",
            Elem::LReal => "expt64",
            other => {
                return Err(self.unsupported(node, format!("EXPT of a {} base", other.name())));
            }
        };
        let base = self.scalar(base)?;
        let base = self.bind(&base);
        let exponent_ty = self.type_of(exponent)?;
        let exponent_elem = self.elem_of(node, exponent_ty, "EXPT")?;
        let value = self.scalar(exponent)?;
        // The exponent in binary64, as the runtime's `to_f64` converts it.
        let exponent = match exponent_elem {
            Elem::LReal => value,
            Elem::Real => format!("f64::from({value})"),
            elem if elem.is_integer() => format!("({value} as f64)"),
            other => {
                return Err(self.unsupported(node, format!("EXPT to a {} power", other.name())));
            }
        };
        self.result(
            node,
            &format!("lib::{library}(cx, {base}, {exponent})?"),
            ty,
            want,
        )
    }

    /// `ATAN2(Y, X)` in the common type of its inputs.
    fn atan2(
        &mut self,
        node: &SyntaxNode,
        values: &[SyntaxNode],
        want: &Want,
    ) -> Result<Called, NativeError> {
        let ty = self.common_of(node, values)?;
        let library = match self.elem_of(node, ty, "ATAN2")? {
            Elem::Real => "atan2_32",
            Elem::LReal => "atan2_64",
            other => return Err(self.unsupported(node, format!("ATAN2 of {}", other.name()))),
        };
        let scalars = self.scalars_of(values, ty)?;
        let [y, x] = scalars.as_slice() else {
            return Err(self.unsupported(node, "ATAN2 with other than two inputs"));
        };
        self.result(node, &format!("lib::{library}({y}, {x})"), ty, want)
    }

    /// `SHL`, `SHR`, `ROL` and `ROR` of a bit string by a count (specification 07 §4).
    fn shift(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        values: &[SyntaxNode],
        want: &Want,
    ) -> Result<Called, NativeError> {
        let [value, count] = values else {
            return Err(self.unsupported(node, format!("{name} with other than two inputs")));
        };
        let ty = self.type_of(value)?;
        let elem = self.elem_of(node, ty, name)?;
        if !(elem.is_bits() || elem == Elem::Bool) {
            return Err(self.unsupported(node, format!("{name} of a {}", elem.name())));
        }
        let value = self.scalar(value)?;
        let value = self.bind(&value);
        let count_ty = self.type_of(count)?;
        let count_elem = self.elem_of(node, count_ty, name)?;
        let count = self.scalar(count)?;
        let count = if count_elem.is_signed() {
            self.bind(&format!("lib::shift_count(cx, i64::from({count}))?"))
        } else if count_elem.is_unsigned() {
            format!("u64::from({count})")
        } else {
            return Err(self.unsupported(node, format!("{name} by a {}", count_elem.name())));
        };
        let shifted = match (elem, name) {
            (Elem::Bool, "SHL" | "SHR") => format!("lib::shift_bool({value}, {count})"),
            (Elem::Bool, _) => value,
            _ => format!("lib::{}({value}, {count})", name.to_ascii_lowercase()),
        };
        self.result(node, &shifted, ty, want)
    }

    /// `MUX(K, IN0, …)`: every input evaluated, then the one `K` selects (specification 07 §6).
    fn mux(
        &mut self,
        node: &SyntaxNode,
        values: &[SyntaxNode],
        want: &Want,
    ) -> Result<Called, NativeError> {
        let Some((selector, inputs)) = values.split_first().filter(|(_, inputs)| inputs.len() >= 2)
        else {
            return Err(self.unsupported(node, "MUX with fewer than two inputs"));
        };
        let selector_ty = self.type_of(selector)?;
        let selector_elem = self.elem_of(node, selector_ty, "MUX")?;
        let selector = self.scalar(selector)?;
        let selector = self.bind(&selector);
        let ty = self.common_of(node, inputs)?;
        if !is_scalar(&self.info(ty).kind) {
            return Err(self.unsupported(node, "MUX of values that are not scalars"));
        }
        let scalars = self.scalars_of(inputs, ty)?;
        let wide = if selector_elem.is_signed() {
            format!("i64::from({selector})")
        } else if selector_elem == Elem::ULInt {
            self.bind(&format!("lib::uint_to::<i64>(cx, {selector})?"))
        } else if selector_elem.is_unsigned() {
            format!("i64::from({selector})")
        } else {
            return Err(self.unsupported(node, format!("MUX by a {}", selector_elem.name())));
        };
        let index = self.bind(&format!("lib::mux_index(cx, {wide}, {})?", scalars.len()));
        let mut arms: Vec<String> = scalars
            .iter()
            .enumerate()
            .map(|(position, value)| format!("{position} => {value}"))
            .collect();
        if let Some(last) = arms.last_mut() {
            *last = format!("_ => {}", scalars[scalars.len() - 1]);
        }
        self.result(
            node,
            &format!("match {index} {{ {} }}", arms.join(", ")),
            ty,
            want,
        )
    }

    /// `GT`, `GE`, `EQ`, `LE` and `LT` of their inputs in a chain, `NE` of two (specification 07 §5).
    fn compare_function(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        values: &[SyntaxNode],
        want: &Want,
    ) -> Result<Called, NativeError> {
        if values.len() < 2 || (name == "NE" && values.len() != 2) {
            return Err(self.unsupported(node, format!("{name} with {} inputs", values.len())));
        }
        let sign = match name {
            "GT" => ">",
            "GE" => ">=",
            "EQ" => "==",
            "LE" => "<=",
            "LT" => "<",
            _ => "!=",
        };
        let ty = self.common_of(node, values)?;
        let operands: Vec<String> = if matches!(self.info(ty).kind, Kind::Str(_)) {
            let mut texts = Vec::with_capacity(values.len());
            for value in values {
                texts.push(self.text_ref(value)?.0);
            }
            texts
        } else {
            self.scalars_of(values, ty)?
        };
        let text = matches!(self.info(ty).kind, Kind::Str(_));
        let links: Vec<String> = operands
            .windows(2)
            .map(|pair| {
                if text {
                    format!(
                        "(lib::compare(({}).view(), ({}).view()) {sign} core::cmp::Ordering::Equal)",
                        pair[0], pair[1]
                    )
                } else {
                    format!("({} {sign} {})", pair[0], pair[1])
                }
            })
            .collect();
        let bool_ty = self.model.types.elem(Elem::Bool);
        self.result(node, &format!("({})", links.join(" && ")), bool_ty, want)
    }

    /// `ADD` and `MUL` of their inputs from the left, `SUB`, `DIV` and `MOD` of two, as the
    /// operators compute them (specification 07 §3).
    fn arithmetic_function(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        values: &[SyntaxNode],
        want: &Want,
    ) -> Result<Called, NativeError> {
        let variadic = matches!(name, "ADD" | "MUL");
        if values.len() < 2 || (!variadic && values.len() != 2) {
            return Err(self.unsupported(node, format!("{name} with {} inputs", values.len())));
        }
        let ty = self.common_of(node, values)?;
        let elem = self.elem_of(node, ty, name)?;
        let function = match (name, elem.is_real()) {
            ("ADD", false) if elem.is_integer() || elem.is_duration() => "add",
            ("SUB", false) if elem.is_integer() || elem.is_duration() => "sub",
            ("MUL", false) if elem.is_integer() => "mul",
            ("DIV", false) if elem.is_integer() => "div",
            ("MOD", false) if elem.is_integer() => "rem",
            ("ADD", true) => "fadd",
            ("SUB", true) => "fsub",
            ("MUL", true) => "fmul",
            ("DIV", true) => "fdiv",
            _ => return Err(self.unsupported(node, format!("{name} of {}", elem.name()))),
        };
        let scalars = self.scalars_of(values, ty)?;
        let mut acc = scalars[0].clone();
        for next in &scalars[1..] {
            acc = self.bind(&format!("lib::{function}(cx, {acc}, {next})?"));
        }
        self.result(node, &acc, ty, want)
    }

    /// `AND`, `OR` and `XOR` of their inputs, `NOT` of one, on bit strings and `BOOL`
    /// (specification 07 §4).
    fn bitwise_function(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        values: &[SyntaxNode],
        want: &Want,
    ) -> Result<Called, NativeError> {
        let ty = self.common_of(node, values)?;
        let elem = self.elem_of(node, ty, name)?;
        if !(elem.is_bits() || elem == Elem::Bool) {
            return Err(self.unsupported(node, format!("{name} of {}", elem.name())));
        }
        let scalars = self.scalars_of(values, ty)?;
        let value = match (name, scalars.as_slice()) {
            ("NOT", [value]) => format!("(!{value})"),
            ("AND" | "OR" | "XOR", [_, _, ..]) => {
                let sign = match name {
                    "AND" => " & ",
                    "OR" => " | ",
                    _ => " ^ ",
                };
                format!("({})", scalars.join(sign))
            }
            _ => {
                return Err(self.unsupported(node, format!("{name} with {} inputs", scalars.len())));
            }
        };
        self.result(node, &value, ty, want)
    }

    /// `MOVE(IN)`: the input's value.
    fn move_value(
        &mut self,
        node: &SyntaxNode,
        values: &[SyntaxNode],
        want: &Want,
    ) -> Result<Called, NativeError> {
        let [value] = values else {
            return Err(self.unsupported(node, "MOVE with other than one input"));
        };
        let ty = self.type_of(node)?;
        match self.info(ty).kind.clone() {
            kind if is_scalar(&kind) => {
                let scalar = self.scalar_to(value, ty)?;
                self.result(node, &scalar, ty, want)
            }
            Kind::Str(_) => Ok(Called::Text(self.text(value)?)),
            _ => Ok(Called::Place(self.aggregate(value, ty)?)),
        }
    }

    /// `LIMIT`, `MAX`, `MIN`, `ABS` and `SEL` in the common type of their inputs, the selector of
    /// `SEL` aside (specification 07 §6).
    fn selection(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        values: &[SyntaxNode],
        want: &Want,
    ) -> Result<Called, NativeError> {
        let operands = if name == "SEL" {
            values.get(1..).unwrap_or(&[])
        } else {
            values
        };
        let ty = self.common_of(node, operands)?;
        let mut scalars = Vec::with_capacity(values.len());
        for value in values {
            let target = if name == "SEL" && scalars.is_empty() {
                self.model.types.elem(Elem::Bool)
            } else {
                ty
            };
            scalars.push(self.scalar_to(value, target)?);
        }
        let elem = self.elem_of(node, ty, name)?;
        let value = match (name, scalars.as_slice()) {
            ("LIMIT", [mn, value, mx]) => format!("lib::limit({mn}, {value}, {mx})"),
            ("MAX" | "MIN", [first, rest @ ..]) if !rest.is_empty() => {
                let function = if name == "MAX" {
                    "lib::max"
                } else {
                    "lib::min"
                };
                rest.iter().fold(first.clone(), |acc, next| {
                    format!("{function}({acc}, {next})")
                })
            }
            ("ABS", [value]) if elem.is_signed() => self.bind(&format!("lib::abs(cx, {value})?")),
            ("ABS", [value]) if elem.is_real() => format!("lib::fabs({value})"),
            ("ABS", [value]) => value.clone(),
            ("SEL", [select, first, second]) => {
                format!("(if {select} {{ {second} }} else {{ {first} }})")
            }
            _ => return Err(self.unsupported(node, format!("the standard function {name}"))),
        };
        self.result(node, &value, ty, want)
    }

    /// `SRC_TO_DST(value)` of the result type `ty` (IEC §6.6.2.5.2 to §6.6.2.5.7; specification
    /// 07 §2), with the runtime's rounding and faults through the library.
    fn conversion(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        value: &SyntaxNode,
        ty: Ty,
    ) -> Result<Called, NativeError> {
        let from = self.type_of(value)?;
        let source = self.scalar(value)?;
        let Some(src) = self.info(from).scalar() else {
            return Err(self.unsupported(node, format!("{name} of a value that is no scalar")));
        };
        if let Kind::Str(_) = self.info(ty).kind {
            let temp = self.temp();
            self.line(&format!("let {temp} = Str::<20>::new();"));
            if src.is_signed() {
                self.line(&format!(
                    "lib::write_int({temp}.view(), i64::from({source}));"
                ));
            } else if src.is_unsigned() || src.is_bits() {
                self.line(&format!(
                    "lib::write_uint({temp}.view(), u64::from({source}));"
                ));
            } else {
                return Err(self.unsupported(node, format!("{name}: {} as text", src.name())));
            }
            return Ok(Called::Text(vec![Text::Str {
                reference: format!("&{temp}"),
                capacity: 20,
            }]));
        }
        let Some(dst) = self.info(ty).scalar() else {
            return Err(self.unsupported(node, format!("{name} to a type that is no scalar")));
        };
        let rust = dst.rust();
        let truncate = name == "TRUNC";
        let converted = if src == dst || (src.is_duration() && dst.is_duration()) {
            source
        } else if src == Elem::Bool && (dst.is_integer() || dst.is_bits()) {
            format!("({source} as {rust})")
        } else if src.is_integer() && dst.is_integer() {
            if src.is_signed() {
                self.bind(&format!("lib::sint_to::<{rust}>(cx, i64::from({source}))?"))
            } else {
                self.bind(&format!("lib::uint_to::<{rust}>(cx, u64::from({source}))?"))
            }
        } else if src.is_integer() && dst.is_real() {
            let (function, wide) = if src.is_signed() {
                ("sint", "i64")
            } else {
                ("uint", "u64")
            };
            let bits = if dst == Elem::Real { 32 } else { 64 };
            format!("lib::{function}_to_real{bits}({wide}::from({source}))")
        } else if src.is_real() && dst.is_integer() {
            let rounding = if truncate { "Truncate" } else { "Nearest" };
            self.bind(&format!(
                "lib::real_to_int::<{rust}>(cx, f64::from({source}), lib::Rounding::{rounding})?"
            ))
        } else if src == Elem::Real && dst == Elem::LReal {
            self.bind(&format!("lib::real32_to_real::<f64>(cx, {source})?"))
        } else if src == Elem::LReal && dst == Elem::Real {
            self.bind(&format!("lib::real64_to_real32::<f32>(cx, {source})?"))
        } else if (src.is_bits() || src.is_integer()) && (dst.is_bits() || dst.is_integer()) {
            // A bit string converts by its bits: cut to the destination's width or extended with
            // zeros, read with the destination's sign (specification 07 §2).
            format!("({source} as {rust})")
        } else if src == Elem::DWord && dst == Elem::Real {
            self.bind(&format!("lib::bits_to_real32(cx, {source})?"))
        } else if src == Elem::LWord && dst == Elem::LReal {
            self.bind(&format!("lib::bits_to_real64(cx, {source})?"))
        } else if src == Elem::Real && dst == Elem::DWord
            || src == Elem::LReal && dst == Elem::LWord
        {
            format!("{source}.to_bits()")
        } else if src.is_duration() && dst == Elem::DWord {
            self.bind(&format!("lib::time_to_dword(cx, {source})?"))
        } else if src == Elem::DWord && dst.is_duration() {
            format!("lib::dword_to_time({source})")
        } else {
            return Err(self.unsupported(node, format!("{name}: {} to {}", src.name(), dst.name())));
        };
        Ok(Called::Scalar(converted))
    }
}

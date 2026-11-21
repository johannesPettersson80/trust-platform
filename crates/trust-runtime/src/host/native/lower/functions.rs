//! The standard functions (IEC §6.6.2.5; specification 07; specification 44 §6.8) by family:
//! numerical, bit-shift, bitwise, selection, comparison and arithmetic functions in their function
//! forms and `MOVE`; the type conversions are in `conversions`, the string functions in `strings`
//! and the date and time functions in `dates`. Each takes its inputs as the runtime takes them,
//! evaluated in their order, and gives its result and its faults as the runtime's implementation
//! does.

use trust_syntax::syntax::SyntaxNode;

use super::call::{Arg, Called, Want};
use super::{is_scalar, Lowerer, Need};
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
        if super::dates::is_date_function(name) {
            return self.call_date_function(node, name, args, &want);
        }
        if super::assertions::assertion(name).is_some() {
            return self.call_assertion(node, name, args);
        }
        if name == "TEST_FINISHED" {
            return self.call_test_finished(node, &args);
        }
        // `__ISVALIDREF(ref)` is `ref <> NULL`: whether the reference is bound (specification 02
        // §4.1).
        if name == "__ISVALIDREF" {
            let [arg] = args.as_slice() else {
                return Err(self.unsupported(node, "__ISVALIDREF with other than one argument"));
            };
            let reference = self.reference(&arg.value)?;
            return Ok(Called::Scalar(format!(
                "(!same_target({reference}, abi::RefValue::NULL_REF))"
            )));
        }
        // `__QUERYINTERFACE(source, target)` is `target ?= source`, TRUE when the target then names
        // an instance (specification 04).
        if name == "__QUERYINTERFACE" {
            let [source, target] = args.as_slice() else {
                return Err(
                    self.unsupported(node, "__QUERYINTERFACE with other than two arguments")
                );
            };
            let place = self.place(&target.value)?;
            let source = self.interface_value(&source.value)?;
            Need::Cast(place.ty).add_to(&mut self.needs);
            let cast = crate::native::dispatch::cast_function(place.ty);
            let answer = self.bind(&format!("{cast}(cx, w, {source})?"));
            self.line(&format!("{}.set({answer});", place.expr));
            return Ok(Called::Scalar(format!(
                "(!same_target({answer}, abi::RefValue::NULL_REF))"
            )));
        }
        let operands = self.standard_operands(name, args)?;
        // The families whose functions of fixed inputs a formal call may leave incomplete, an
        // omitted input taking the default value of its type (IEC 61131-3 Ed.3 §6.6.1.4.2;
        // specification 07 §1), with their extensible functions.
        match name {
            "EXPT" => return self.expt(node, &operands, &want),
            "ATAN2" => return self.atan2(node, &operands, &want),
            "SHL" | "SHR" | "ROL" | "ROR" => return self.shift(node, name, &operands, &want),
            "GT" | "GE" | "EQ" | "LE" | "LT" | "NE" => {
                return self.compare_function(node, name, &operands, &want)
            }
            "ADD" | "MUL" | "SUB" | "DIV" | "MOD" => {
                return self.arithmetic_function(node, name, &operands, &want)
            }
            "SEL" | "LIMIT" | "MAX" | "MIN" | "ABS" => {
                return self.selection(node, name, &operands, &want)
            }
            _ => {}
        }
        let values = self.given(node, name, operands)?;
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
        // `LOWER_BOUND` and `UPPER_BOUND` of a fixed array are constants, of an open array its
        // view's (specification 07, "Array bound").
        if name == "LOWER_BOUND" || name == "UPPER_BOUND" {
            let bound = self.array_bound(node, name == "UPPER_BOUND", &values)?;
            let ty = self.type_of(node)?;
            let rust = self.info(ty).scalar().map_or("i64", Elem::rust);
            if rust == "i64" {
                return Ok(Called::Scalar(bound));
            }
            return Ok(Called::Scalar(
                self.bind(&format!("lib::sint_to::<{rust}>(cx, {bound})?")),
            ));
        }
        // `CURRENT_DT()` reads the platform's wall clock: a `DT` of Unix milliseconds, which the
        // runtime's clock does not move (specification 07, "Runtime Clock Sources").
        if name == "CURRENT_DT" && values.is_empty() {
            self.model.wall_clock = true;
            return Ok(Called::Scalar(self.bind("abi::module::wall_clock(cx)?")));
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
            "MUX" => self.mux(node, &values, &want),
            "AND" | "OR" | "XOR" | "NOT" => self.bitwise_function(node, name, &values, &want),
            "IS_VALID" | "IS_VALID_BCD" => self.validation(node, name, &values, &want),
            _ => Err(self.unsupported(node, format!("the standard function {name}"))),
        }
    }

    /// The inputs of `name`, a function whose inputs a formal call may not omit, each of which the
    /// call gives.
    fn given(
        &self,
        node: &SyntaxNode,
        name: &str,
        operands: Vec<Option<SyntaxNode>>,
    ) -> Result<Vec<SyntaxNode>, NativeError> {
        let formals = fixed_formals(name).unwrap_or(&[]);
        operands
            .into_iter()
            .enumerate()
            .map(|(index, operand)| {
                operand.ok_or_else(|| {
                    let formal = formals.get(index).copied().unwrap_or("input");
                    self.unsupported(node, format!("{name} without its {formal}"))
                })
            })
            .collect()
    }

    /// `IS_VALID(IN)`, whether a `REAL` or an `LREAL` is finite, and `IS_VALID_BCD(IN)`, whether
    /// every four bits of a bit string are at most 9 (IEC Table 39; specification 07 §9).
    fn validation(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        values: &[SyntaxNode],
        want: &Want,
    ) -> Result<Called, NativeError> {
        let [value] = values else {
            return Err(self.unsupported(node, format!("{name} with other than one input")));
        };
        let ty = self.type_of(value)?;
        let elem = self.elem_of(node, ty, name)?;
        let input = self.scalar_to(value, ty)?;
        let valid = match (name, elem) {
            ("IS_VALID", Elem::Real | Elem::LReal) => format!("({input}).is_finite()"),
            ("IS_VALID_BCD", Elem::Byte | Elem::Word | Elem::DWord | Elem::LWord) => {
                let digits = elem.size() * 2;
                format!(
                    "{{ let bits = u64::from({input}); (0..{digits}u32).all(|digit| (bits >> (4 * digit)) & 0xF <= 9) }}"
                )
            }
            _ => {
                return Err(self.unsupported(node, format!("{name} of a {}", elem.name())));
            }
        };
        let bool_ty = self.model.types.elem(Elem::Bool);
        self.result(node, &valid, bool_ty, want)
    }

    /// The arguments of standard function `name` in the order of its formal parameters
    /// (specification 07 §1): positional ones as they come; formal ones by name, `None` for an
    /// input of fixed inputs that a formal call omits, and `IN1`, `IN2`, … of an extensible
    /// function by their numbers, `K` of `MUX` first.
    fn standard_operands(
        &self,
        name: &str,
        args: Vec<Arg>,
    ) -> Result<Vec<Option<SyntaxNode>>, NativeError> {
        if args.iter().all(|arg| arg.name.is_none()) {
            return Ok(args.into_iter().map(|arg| Some(arg.value)).collect());
        }
        if let Some(formals) = fixed_formals(name) {
            return self.formal_slots(name, formals, args);
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
        Ok(keyed.into_iter().map(|(_, value)| Some(value)).collect())
    }

    /// The common type of `operands`, the widest, as the runtime's functions take their inputs
    /// (specification 07 §6).
    pub(super) fn common_of(
        &mut self,
        node: &SyntaxNode,
        operands: &[SyntaxNode],
    ) -> Result<Ty, NativeError> {
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

    /// `EXPT(IN1, IN2)`: a real base to any number's power, in binary64; an exponent that a formal
    /// call omits is 0.
    fn expt(
        &mut self,
        node: &SyntaxNode,
        operands: &[Option<SyntaxNode>],
        want: &Want,
    ) -> Result<Called, NativeError> {
        let [base, exponent] = operands else {
            return Err(self.unsupported(node, "EXPT with other than two inputs"));
        };
        let Some(base) = base else {
            return Err(self.unsupported(node, "EXPT without its IN1"));
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
        let Some(exponent) = exponent else {
            return self.result(
                node,
                &format!("lib::{library}(cx, {base}, 0f64)?"),
                ty,
                want,
            );
        };
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

    /// `ATAN2(Y, X)` in the common type of its inputs, an omitted one 0.
    fn atan2(
        &mut self,
        node: &SyntaxNode,
        operands: &[Option<SyntaxNode>],
        want: &Want,
    ) -> Result<Called, NativeError> {
        let ty = self.common_of_given(node, operands)?;
        let library = match self.elem_of(node, ty, "ATAN2")? {
            Elem::Real => "atan2_32",
            Elem::LReal => "atan2_64",
            other => return Err(self.unsupported(node, format!("ATAN2 of {}", other.name()))),
        };
        let scalars = self.operand_scalars(node, operands, ty)?;
        let [y, x] = scalars.as_slice() else {
            return Err(self.unsupported(node, "ATAN2 with other than two inputs"));
        };
        self.result(node, &format!("lib::{library}({y}, {x})"), ty, want)
    }

    /// `SHL`, `SHR`, `ROL` and `ROR` of a bit string by a count (specification 07 §4); a count that
    /// a formal call omits is 0.
    fn shift(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        operands: &[Option<SyntaxNode>],
        want: &Want,
    ) -> Result<Called, NativeError> {
        let [value, count] = operands else {
            return Err(self.unsupported(node, format!("{name} with other than two inputs")));
        };
        let Some(value) = value else {
            return Err(self.unsupported(node, format!("{name} without its IN")));
        };
        let ty = self.type_of(value)?;
        let elem = self.elem_of(node, ty, name)?;
        if !(elem.is_bits() || elem == Elem::Bool) {
            return Err(self.unsupported(node, format!("{name} of a {}", elem.name())));
        }
        let value = self.scalar(value)?;
        let value = self.bind(&value);
        let count = match count {
            Some(count) => {
                let count_ty = self.type_of(count)?;
                let count_elem = self.elem_of(node, count_ty, name)?;
                let count = self.scalar(count)?;
                if count_elem.is_signed() {
                    self.bind(&format!("lib::shift_count(cx, i64::from({count}))?"))
                } else if count_elem.is_unsigned() {
                    format!("u64::from({count})")
                } else {
                    return Err(
                        self.unsupported(node, format!("{name} by a {}", count_elem.name()))
                    );
                }
            }
            None => "0u64".to_string(),
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
        let selector = self.mux_selector(node, selector)?;
        if let Some(called) = self.text_mux(node, &selector, inputs)? {
            return Ok(called);
        }
        let ty = self.common_of(node, inputs)?;
        if !is_scalar(&self.info(ty).kind) {
            return Err(self.unsupported(node, "MUX of values that are not scalars"));
        }
        let scalars = self.scalars_of(inputs, ty)?;
        let index = self.mux_index(node, &selector, scalars.len())?;
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

    /// `MUX`'s selector `K`, read before its inputs, and its elementary type.
    pub(super) fn mux_selector(
        &mut self,
        node: &SyntaxNode,
        selector: &SyntaxNode,
    ) -> Result<(String, Elem), NativeError> {
        let selector_ty = self.type_of(selector)?;
        let elem = self.elem_of(node, selector_ty, "MUX")?;
        let value = self.scalar(selector)?;
        Ok((self.bind(&value), elem))
    }

    /// The input of `count` that `MUX`'s selector, read by `mux_selector`, chooses, from 0: a
    /// selector outside them ends the scan with `IndexOutOfBounds` (specification 07 §6.1).
    pub(super) fn mux_index(
        &mut self,
        node: &SyntaxNode,
        (selector, elem): &(String, Elem),
        count: usize,
    ) -> Result<String, NativeError> {
        let wide = if elem.is_signed() {
            format!("i64::from({selector})")
        } else if *elem == Elem::ULInt {
            self.bind(&format!("lib::uint_to::<i64>(cx, {selector})?"))
        } else if elem.is_unsigned() {
            format!("i64::from({selector})")
        } else {
            return Err(self.unsupported(node, format!("MUX by a {}", elem.name())));
        };
        Ok(self.bind(&format!("lib::mux_index(cx, {wide}, {count})?")))
    }

    /// `GT`, `GE`, `EQ`, `LE` and `LT` of their inputs in a chain, `NE` of two (specification 07 §5).
    fn compare_function(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        operands: &[Option<SyntaxNode>],
        want: &Want,
    ) -> Result<Called, NativeError> {
        if operands.len() < 2 || (name == "NE" && operands.len() != 2) {
            return Err(self.unsupported(node, format!("{name} with {} inputs", operands.len())));
        }
        let sign = match name {
            "GT" => ">",
            "GE" => ">=",
            "EQ" => "==",
            "LE" => "<=",
            "LT" => "<",
            _ => "!=",
        };
        let ty = self.common_of_given(node, operands)?;
        let wide = matches!(self.info(ty).kind, Kind::WStr(_));
        let compared: Vec<String> = if matches!(self.info(ty).kind, Kind::Str(_) | Kind::WStr(_)) {
            self.operand_texts(operands, wide)?
                .into_iter()
                .map(|(text, _)| text)
                .collect()
        } else {
            self.operand_scalars(node, operands, ty)?
        };
        let text = matches!(self.info(ty).kind, Kind::Str(_) | Kind::WStr(_));
        let compare = if wide { "compare_wide" } else { "compare" };
        let links: Vec<String> = compared
            .windows(2)
            .map(|pair| {
                if text {
                    format!(
                        "(lib::{compare}(({}).view(), ({}).view()) {sign} core::cmp::Ordering::Equal)",
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
        operands: &[Option<SyntaxNode>],
        want: &Want,
    ) -> Result<Called, NativeError> {
        let variadic = matches!(name, "ADD" | "MUL");
        if operands.len() < 2 || (!variadic && operands.len() != 2) {
            return Err(self.unsupported(node, format!("{name} with {} inputs", operands.len())));
        }
        let ty = self.common_of_given(node, operands)?;
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
        let scalars = self.operand_scalars(node, operands, ty)?;
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
            Kind::Str(_) | Kind::WStr(_) => Ok(Called::Text(self.text(value)?)),
            _ => Ok(Called::Place(self.aggregate(value, ty)?)),
        }
    }

    /// `LIMIT`, `MAX`, `MIN`, `ABS` and `SEL` in the common type of their inputs, the selector of
    /// `SEL` aside (specification 07 §6); an omitted input is the default value of its type, an
    /// omitted selector `FALSE`.
    fn selection(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        operands: &[Option<SyntaxNode>],
        want: &Want,
    ) -> Result<Called, NativeError> {
        if let Some(called) = self.text_selection(node, name, operands)? {
            return Ok(called);
        }
        let inputs = if name == "SEL" {
            operands.get(1..).unwrap_or(&[])
        } else {
            operands
        };
        let ty = self.common_of_given(node, inputs)?;
        let mut scalars = Vec::with_capacity(operands.len());
        for operand in operands {
            let target = if name == "SEL" && scalars.is_empty() {
                self.model.types.elem(Elem::Bool)
            } else {
                ty
            };
            scalars.push(match operand {
                Some(value) => self.scalar_to(value, target)?,
                None => self.default_scalar(node, target)?,
            });
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
}

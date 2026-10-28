//! Dates and times (IEC §6.4.2, Tables 36 and 37; specification 07 §9; specification 44 §6.8):
//! the operators that add a duration to a time of day or a date and time or take two apart, the
//! scaling of a duration, the date and time functions, and the conversions between the date and
//! time types, each as the runtime computes it on its millisecond and nanosecond values.

use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::call::{Arg, Called, Want};
use super::Lowerer;
use crate::native::types::Elem;
use crate::native::NativeError;

/// Whether `elem` holds milliseconds since 1970-01-01: `DATE`, `TIME_OF_DAY`, `DATE_AND_TIME`.
fn is_ticks(elem: Elem) -> bool {
    matches!(elem, Elem::Date | Elem::Tod | Elem::Dt)
}

/// Whether `elem` is a long date or time, in nanoseconds: `LDATE`, `LTIME_OF_DAY`,
/// `LDATE_AND_TIME`.
fn is_long(elem: Elem) -> bool {
    matches!(elem, Elem::LDate | Elem::LTod | Elem::Ldt)
}

/// The inputs `IN1` and `IN2`.
const IN1_IN2: &[&str] = &["IN1", "IN2"];
/// The components of a date.
const DATE: &[&str] = &["YEAR", "MONTH", "DAY"];
/// The components of a time of day.
const CLOCK: &[&str] = &["HOUR", "MINUTE", "SECOND", "MILLISECOND"];
/// The components of a date and time.
const DATE_AND_CLOCK: &[&str] = &[
    "YEAR",
    "MONTH",
    "DAY",
    "HOUR",
    "MINUTE",
    "SECOND",
    "MILLISECOND",
];

/// How a date and time function computes, by its name.
enum DateFunction {
    /// A duration and a duration of one type: `ADD_TIME`, `SUB_LTIME`.
    Durations { add: bool },
    /// A millisecond value and a `TIME`: `ADD_TOD_TIME`, `SUB_DT_TIME`.
    TicksAndTime { add: bool },
    /// A nanosecond value and an `LTIME`: `ADD_LTOD_LTIME`, `SUB_LDT_LTIME`.
    NanosAndTime { add: bool },
    /// Two millisecond values apart: `SUB_DATE_DATE`, `SUB_TOD_TOD`, `SUB_DT_DT`.
    TicksApart,
    /// Two nanosecond values apart: `SUB_LDATE_LDATE`, `SUB_LTOD_LTOD`, `SUB_LDT_LDT`.
    NanosApart,
    /// `MUL_TIME`, `DIV_TIME`, `MUL_LTIME`, `DIV_LTIME`.
    Scale { multiply: bool },
    /// The library function of the name over the integer components of its formals:
    /// `CONCAT_DATE`, `CONCAT_TOD`.
    Components(&'static str, &'static [&'static str]),
    /// `CONCAT_DATE_TOD`, `CONCAT_DATE_LTOD`: the library function and its formals.
    Join(&'static str, &'static [&'static str]),
    /// `DAY_OF_WEEK`.
    DayOfWeek,
    /// `SPLIT_DATE` and the others: the library function, its formals and whether it can fault.
    Split(&'static str, &'static [&'static str], bool),
}

impl DateFunction {
    /// The formal parameters, in their order (`trust-runtime-core`'s registration).
    fn formals(&self) -> &'static [&'static str] {
        match self {
            Self::Durations { .. }
            | Self::TicksAndTime { .. }
            | Self::NanosAndTime { .. }
            | Self::TicksApart
            | Self::NanosApart
            | Self::Scale { .. } => IN1_IN2,
            Self::Components(_, formals) | Self::Join(_, formals) | Self::Split(_, formals, _) => {
                formals
            }
            Self::DayOfWeek => &["IN"],
        }
    }
}

/// Whether `name` is a date and time function this module lowers.
pub(super) fn is_date_function(name: &str) -> bool {
    date_function(name).is_some()
}

fn date_function(name: &str) -> Option<DateFunction> {
    use DateFunction as F;
    const SPLIT_CLOCK: &[&str] = &["IN", "HOUR", "MINUTE", "SECOND", "MILLISECOND"];
    const SPLIT_DATE_AND_CLOCK: &[&str] = &[
        "IN",
        "YEAR",
        "MONTH",
        "DAY",
        "HOUR",
        "MINUTE",
        "SECOND",
        "MILLISECOND",
    ];
    Some(match name {
        "ADD_TIME" | "ADD_LTIME" => F::Durations { add: true },
        "SUB_TIME" | "SUB_LTIME" => F::Durations { add: false },
        "ADD_TOD_TIME" | "ADD_DT_TIME" => F::TicksAndTime { add: true },
        "SUB_TOD_TIME" | "SUB_DT_TIME" => F::TicksAndTime { add: false },
        "ADD_LTOD_LTIME" | "ADD_LDT_LTIME" => F::NanosAndTime { add: true },
        "SUB_LTOD_LTIME" | "SUB_LDT_LTIME" => F::NanosAndTime { add: false },
        "SUB_DATE_DATE" | "SUB_TOD_TOD" | "SUB_DT_DT" => F::TicksApart,
        "SUB_LDATE_LDATE" | "SUB_LTOD_LTOD" | "SUB_LDT_LDT" => F::NanosApart,
        "MUL_TIME" | "MUL_LTIME" => F::Scale { multiply: true },
        "DIV_TIME" | "DIV_LTIME" => F::Scale { multiply: false },
        "CONCAT_DATE" => F::Components("concat_date", DATE),
        "CONCAT_TOD" => F::Components("concat_tod", CLOCK),
        "CONCAT_LTOD" => F::Components("concat_ltod", CLOCK),
        "CONCAT_DT" => F::Components("concat_dt", DATE_AND_CLOCK),
        "CONCAT_LDT" => F::Components("concat_ldt", DATE_AND_CLOCK),
        "CONCAT_DATE_TOD" => F::Join("concat_date_tod", &["DATE", "TOD"]),
        "CONCAT_DATE_LTOD" => F::Join("concat_date_ltod", &["DATE", "LTOD"]),
        "DAY_OF_WEEK" => F::DayOfWeek,
        "SPLIT_DATE" => F::Split("split_date", &["IN", "YEAR", "MONTH", "DAY"], false),
        "SPLIT_TOD" => F::Split("split_tod", SPLIT_CLOCK, true),
        "SPLIT_LTOD" => F::Split("split_ltod", SPLIT_CLOCK, false),
        "SPLIT_DT" => F::Split("split_dt", SPLIT_DATE_AND_CLOCK, false),
        "SPLIT_LDT" => F::Split("split_ldt", SPLIT_DATE_AND_CLOCK, false),
        _ => return None,
    })
}

impl Lowerer<'_, '_> {
    /// The scalar of `node` in its own type, in a temporary, with its type's element.
    fn operand(&mut self, node: &SyntaxNode) -> Result<(String, Elem), NativeError> {
        let ty = self.type_of(node)?;
        let Some(elem) = self.info(ty).scalar() else {
            return Err(self.unsupported(node, "a date or time operand that is no scalar"));
        };
        let value = self.scalar(node)?;
        Ok((self.bind(&value), elem))
    }

    /// `left ± right` when a date, a time of day or a date and time takes part, `left * right` and
    /// `left / right` of a duration and a number (specification 07 §9): the value, or `None` for
    /// operands of other types.
    pub(super) fn date_time_operator(
        &mut self,
        node: &SyntaxNode,
        left: &SyntaxNode,
        right: &SyntaxNode,
        op: SyntaxKind,
    ) -> Result<Option<String>, NativeError> {
        let (Some(left_elem), Some(right_elem)) =
            (self.scalar_elem(left)?, self.scalar_elem(right)?)
        else {
            return Ok(None);
        };
        use Elem as E;
        use SyntaxKind as K;
        let add = op == K::Plus;
        let plus_or_minus = matches!(op, K::Plus | K::Minus);
        let call = match (left_elem, right_elem) {
            (E::Tod | E::Dt, E::Time) if plus_or_minus => {
                let (a, _) = self.operand(left)?;
                let (b, _) = self.operand(right)?;
                format!("lib::ticks_add(cx, {a}, {b}, {add})?")
            }
            (E::LTod | E::Ldt, E::LTime) if plus_or_minus => {
                let (a, _) = self.operand(left)?;
                let (b, _) = self.operand(right)?;
                format!("lib::nanos_add(cx, {a}, {b}, {add})?")
            }
            (a, b) if a == b && is_ticks(a) && op == K::Minus => {
                let (a, _) = self.operand(left)?;
                let (b, _) = self.operand(right)?;
                format!("lib::ticks_diff(cx, {a}, {b})?")
            }
            (a, b) if a == b && is_long(a) && op == K::Minus => {
                let (a, _) = self.operand(left)?;
                let (b, _) = self.operand(right)?;
                format!("lib::nanos_add(cx, {a}, {b}, false)?")
            }
            (duration, factor)
                if duration.is_duration()
                    && !factor.is_duration()
                    && matches!(op, K::Star | K::Slash) =>
            {
                let (a, _) = self.operand(left)?;
                let factor_value = self.operand(right)?;
                self.scale(node, &a, factor_value, op == K::Star)?
            }
            _ => return Ok(None),
        };
        Ok(Some(self.bind(&call)))
    }

    /// The element of `node`'s type, when it is a scalar.
    fn scalar_elem(&mut self, node: &SyntaxNode) -> Result<Option<Elem>, NativeError> {
        let ty = self.type_of(node)?;
        Ok(self.info(ty).scalar())
    }

    /// The call that scales the duration `duration` by `factor` of its own type.
    fn scale(
        &self,
        node: &SyntaxNode,
        duration: &str,
        (factor, elem): (String, Elem),
        multiply: bool,
    ) -> Result<String, NativeError> {
        Ok(if elem.is_signed() {
            format!("lib::scale_signed(cx, {duration}, i64::from({factor}), {multiply})?")
        } else if elem.is_unsigned() {
            format!("lib::scale_unsigned(cx, {duration}, u64::from({factor}), {multiply})?")
        } else if elem.is_real() {
            format!("lib::scale_real(cx, {duration}, f64::from({factor}), {multiply})?")
        } else {
            return Err(self.unsupported(node, format!("a duration scaled by a {}", elem.name())));
        })
    }

    /// An integer component as an `i64`, as the runtime's `to_i64` takes it: a `ULINT` above the
    /// largest `LINT` overflows.
    fn component(&mut self, node: &SyntaxNode) -> Result<String, NativeError> {
        let (value, elem) = self.operand(node)?;
        Ok(if elem == Elem::ULInt {
            self.bind(&format!("lib::uint_to::<i64>(cx, {value})?"))
        } else if elem.is_integer() {
            format!("i64::from({value})")
        } else {
            return Err(self.unsupported(node, format!("a date component of {}", elem.name())));
        })
    }

    /// The date and time function `name` with `args`; `name` is one (`is_date_function`).
    pub(super) fn call_date_function(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        args: Vec<Arg>,
        want: &Want,
    ) -> Result<Called, NativeError> {
        let Some(function) = date_function(name) else {
            return Err(self.unsupported(node, format!("the standard function {name}")));
        };
        use DateFunction as F;
        let values = self.formal_order(node, name, function.formals(), args)?;
        if let F::Split(library, _, faults) = function {
            self.split(name, library, faults, &values)?;
            return Ok(Called::Nothing);
        }
        let ty = self.type_of(node)?;
        let value = match (function, values.as_slice()) {
            (F::Durations { add }, [a, b]) => {
                let (a, _) = self.operand(a)?;
                let (b, _) = self.operand(b)?;
                let function = if add { "add" } else { "sub" };
                format!("lib::{function}(cx, {a}, {b})?")
            }
            (F::TicksAndTime { add }, [a, b]) => {
                let (a, _) = self.operand(a)?;
                let (b, _) = self.operand(b)?;
                format!("lib::ticks_add(cx, {a}, {b}, {add})?")
            }
            (F::NanosAndTime { add }, [a, b]) => {
                let (a, _) = self.operand(a)?;
                let (b, _) = self.operand(b)?;
                format!("lib::nanos_add(cx, {a}, {b}, {add})?")
            }
            (F::TicksApart, [a, b]) => {
                let (a, _) = self.operand(a)?;
                let (b, _) = self.operand(b)?;
                format!("lib::ticks_diff(cx, {a}, {b})?")
            }
            (F::NanosApart, [a, b]) => {
                let (a, _) = self.operand(a)?;
                let (b, _) = self.operand(b)?;
                format!("lib::nanos_add(cx, {a}, {b}, false)?")
            }
            (F::Scale { multiply }, [duration, factor]) => {
                let (duration, _) = self.operand(duration)?;
                let factor = self.operand(factor)?;
                self.scale(node, &duration, factor, multiply)?
            }
            (F::Components(library, _), components) => {
                let mut wide = Vec::with_capacity(components.len());
                for component in components {
                    wide.push(self.component(component)?);
                }
                format!("lib::{library}(cx, {})?", wide.join(", "))
            }
            (F::Join(library, _), [a, b]) => {
                let (a, _) = self.operand(a)?;
                let (b, _) = self.operand(b)?;
                format!("lib::{library}(cx, {a}, {b})?")
            }
            (F::DayOfWeek, [date]) => {
                let (date, _) = self.operand(date)?;
                format!("lib::day_of_week({date})")
            }
            _ => return Err(self.unsupported(node, format!("{name} with these inputs"))),
        };
        let value = self.bind(&value);
        Ok(match want {
            Want::Scalar(target) => Called::Scalar(self.convert(node, &value, ty, *target)?),
            _ => Called::Scalar(value),
        })
    }

    /// `SPLIT_DATE(IN, YEAR, MONTH, DAY)` and the others, of `values` in their formals' order:
    /// the components of the input, each converted to its output's integer type before any is
    /// written, as the runtime writes them in one transaction.
    fn split(
        &mut self,
        name: &str,
        library: &str,
        faults: bool,
        values: &[SyntaxNode],
    ) -> Result<(), NativeError> {
        let Some((input, outputs)) = values.split_first() else {
            return Ok(());
        };
        let (input, _) = self.operand(input)?;
        let components = if faults {
            self.bind(&format!("lib::{library}(cx, {input})?"))
        } else {
            self.bind(&format!("lib::{library}({input})"))
        };
        let lint = self.model.types.elem(Elem::LInt);
        let mut converted = Vec::with_capacity(outputs.len());
        for (index, output) in outputs.iter().enumerate() {
            let place = self.place(output)?;
            if !self
                .info(place.ty)
                .scalar()
                .is_some_and(|elem| elem.is_integer())
            {
                return Err(
                    self.unsupported(output, format!("an output of {name} that is no integer"))
                );
            }
            let value = self.convert(output, &format!("{components}[{index}]"), lint, place.ty)?;
            converted.push((place, self.bind(&value)));
        }
        for (place, value) in converted {
            self.store_scalar(&place, &value);
        }
        Ok(())
    }
}

/// `SRC_TO_DST` between the date and time types (specification 07 §2): the call and whether it can
/// fault, or `None` for another pair.
pub(super) fn date_conversion(src: Elem, dst: Elem, value: &str) -> Option<(String, bool)> {
    use Elem as E;
    Some(match (src, dst) {
        (E::Dt, E::Date) => (format!("lib::dt_to_date(cx, {value})?"), true),
        (E::Ldt, E::Date) => (format!("lib::ldt_to_date({value})"), false),
        (E::Dt, E::Tod) => (format!("lib::dt_to_tod({value})"), false),
        (E::Dt, E::LTod) => (format!("lib::dt_to_ltod({value})"), false),
        (E::Ldt, E::Tod) => (format!("lib::ldt_to_tod({value})"), false),
        (E::Ldt, E::LTod) => (format!("lib::ldt_to_ltod({value})"), false),
        (E::LTod, E::Tod) | (E::Ldt, E::Dt) => (format!("lib::nanos_to_ticks({value})"), false),
        (E::Tod, E::LTod) | (E::Dt, E::Ldt) => (format!("lib::ticks_to_nanos(cx, {value})?"), true),
        _ => return None,
    })
}

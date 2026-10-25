//! The string functions and the characters of a `STRING` (IEC §6.6.2.5.11, Table 34;
//! specification 02 §6; specification 07 §8; specification 44 §6.8): each function's result is
//! written into a temporary large enough for every result its inputs can give, the sum of their
//! capacities, so that only the assignment that takes it cuts it, as in the runtime.

use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::call::{Arg, Called, Want};
use super::text::Text;
use super::Lowerer;
use crate::native::types::{Elem, Kind};
use crate::native::NativeError;

/// The longest text a string function may give, in bytes (specification 07 §8.1).
const MAX_TEXT: u32 = 65_535;

/// The formal parameters of string function `name`, in their order, when it is one this module
/// lowers.
pub(super) fn text_function(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "LEN" => &["IN"],
        "LEFT" | "RIGHT" => &["IN", "L"],
        "MID" | "DELETE" => &["IN", "L", "P"],
        "INSERT" => &["IN1", "IN2", "P"],
        "REPLACE" => &["IN1", "IN2", "L", "P"],
        "FIND" => &["IN1", "IN2"],
        _ => return None,
    })
}

impl Lowerer<'_, '_> {
    /// A reference to a `Str` that holds the text of `node`, and its capacity: the string itself,
    /// or a temporary its parts are joined into.
    pub(super) fn text_ref(&mut self, node: &SyntaxNode) -> Result<(String, u32), NativeError> {
        let parts = self.text(node)?;
        if let [Text::Str {
            reference,
            capacity,
        }] = parts.as_slice()
        {
            return Ok((reference.clone(), *capacity));
        }
        let capacity = parts
            .iter()
            .map(Text::capacity)
            .fold(0u32, u32::saturating_add)
            .min(MAX_TEXT);
        Ok((self.text_temp(&parts, capacity), capacity))
    }

    /// The arguments of standard function `name` in the order of `formals`: positional ones in
    /// order, formal ones by name (IEC §6.6.1.4).
    pub(super) fn formal_order(
        &self,
        node: &SyntaxNode,
        name: &str,
        formals: &[&str],
        args: Vec<Arg>,
    ) -> Result<Vec<SyntaxNode>, NativeError> {
        let mut slots: Vec<Option<SyntaxNode>> = vec![None; formals.len()];
        let mut next = 0;
        for arg in args {
            let slot = match &arg.name {
                Some(formal) => formals
                    .iter()
                    .position(|candidate| candidate.eq_ignore_ascii_case(formal)),
                None => {
                    next += 1;
                    Some(next - 1).filter(|slot| *slot < formals.len())
                }
            };
            let Some(slot) = slot else {
                return Err(self.unsupported(&arg.value, format!("an argument {name} has no")));
            };
            slots[slot] = Some(arg.value);
        }
        slots
            .into_iter()
            .zip(formals)
            .map(|(slot, formal)| {
                slot.ok_or_else(|| self.unsupported(node, format!("{name} without its {formal}")))
            })
            .collect()
    }

    /// An integer argument of a string function as an `i64`, as the runtime takes it.
    fn count(&mut self, node: &SyntaxNode) -> Result<String, NativeError> {
        let lint = self.model.types.elem(Elem::LInt);
        self.scalar_to(node, lint)
    }

    /// The call of string function `name` with `args`; its arguments are evaluated in their
    /// order. A function of `WSTRING`s calls the library's wide function of the same rule.
    pub(super) fn call_text_function(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        formals: &[&str],
        args: Vec<Arg>,
        want: &Want,
    ) -> Result<Called, NativeError> {
        let values = self.formal_order(node, name, formals, args)?;
        let wide = values.first().is_some_and(|text| self.is_wide_text(text));
        let function = |base: &str| {
            if wide {
                format!("{base}_wide")
            } else {
                base.to_string()
            }
        };
        match (name, values.as_slice()) {
            ("LEN", [text]) => {
                let (text, _) = self.text_ref(text)?;
                let len = if wide { "len_wide_int" } else { "len_int" };
                let value = self.bind(&format!("lib::{len}(cx, ({text}).view())?"));
                self.int_result(node, value, want)
            }
            ("FIND", [text, wanted]) => {
                let (text, _) = self.text_ref(text)?;
                let (wanted, _) = self.text_ref(wanted)?;
                let value = self.bind(&format!(
                    "lib::{}(cx, ({text}).view(), ({wanted}).view())?",
                    function("find")
                ));
                self.int_result(node, value, want)
            }
            ("LEFT" | "RIGHT", [text, count]) => {
                let (text, capacity) = self.text_ref(text)?;
                let count = self.count(count)?;
                let function = function(&name.to_ascii_lowercase());
                let temp = self.result_temp(capacity, wide);
                self.line(&format!(
                    "lib::{function}(cx, {temp}.view(), ({text}).view(), {count})?;"
                ));
                Ok(text_result(temp, capacity, wide))
            }
            ("MID" | "DELETE", [text, length, position]) => {
                let (text, capacity) = self.text_ref(text)?;
                let length = self.count(length)?;
                let position = self.count(position)?;
                let function = function(&name.to_ascii_lowercase());
                let temp = self.result_temp(capacity, wide);
                self.line(&format!(
                    "lib::{function}(cx, {temp}.view(), ({text}).view(), {length}, {position})?;"
                ));
                Ok(text_result(temp, capacity, wide))
            }
            ("INSERT", [text, inserted, position]) => {
                let (text, first) = self.text_ref(text)?;
                let (inserted, second) = self.text_ref(inserted)?;
                let position = self.count(position)?;
                let capacity = first.saturating_add(second).min(MAX_TEXT);
                let temp = self.result_temp(capacity, wide);
                self.line(&format!(
                    "lib::{}(cx, {temp}.view(), ({text}).view(), ({inserted}).view(), {position})?;",
                    function("insert")
                ));
                Ok(text_result(temp, capacity, wide))
            }
            ("REPLACE", [text, replacement, length, position]) => {
                let (text, first) = self.text_ref(text)?;
                let (replacement, second) = self.text_ref(replacement)?;
                let length = self.count(length)?;
                let position = self.count(position)?;
                let capacity = first.saturating_add(second).min(MAX_TEXT);
                let temp = self.result_temp(capacity, wide);
                self.line(&format!(
                    "lib::{}(cx, {temp}.view(), ({text}).view(), ({replacement}).view(), {length}, {position})?;",
                    function("replace")
                ));
                Ok(text_result(temp, capacity, wide))
            }
            _ => Err(self.unsupported(node, format!("the standard function {name}"))),
        }
    }

    /// A fresh empty `STRING[capacity]`, or `WSTRING[capacity]` when `wide`, for a function's
    /// result.
    fn result_temp(&mut self, capacity: u32, wide: bool) -> String {
        let temp = self.temp();
        let ty = if wide { "WStr" } else { "Str" };
        self.line(&format!("let {temp} = {ty}::<{capacity}>::new();"));
        temp
    }

    /// The `INT` result `value` of `LEN` or `FIND`, as `want` needs it.
    fn int_result(
        &mut self,
        node: &SyntaxNode,
        value: String,
        want: &Want,
    ) -> Result<Called, NativeError> {
        let int = self.model.types.elem(Elem::Int);
        match want {
            Want::Scalar(target) => Ok(Called::Scalar(self.convert(node, &value, int, *target)?)),
            _ => Ok(Called::Scalar(value)),
        }
    }

    /// The string, the index expression of `node`, `text[index]`, and whether the string is a
    /// `WSTRING`, when it indexes a string.
    fn string_index(
        &mut self,
        node: &SyntaxNode,
    ) -> Result<Option<(SyntaxNode, SyntaxNode, bool)>, NativeError> {
        if node.kind() != SyntaxKind::IndexExpr {
            return Ok(None);
        }
        let mut children = node.children();
        let (Some(base), Some(index)) = (children.next(), children.next()) else {
            return Ok(None);
        };
        let ty = self.type_of(&base)?;
        match self.info(ty).kind {
            Kind::Str(_) => Ok(Some((base, index, false))),
            Kind::WStr(_) => Ok(Some((base, index, true))),
            _ => Ok(None),
        }
    }

    /// `text[index]` as a `CHAR`, or a `WCHAR` of a `WSTRING`, when `node` indexes a string
    /// (specification 02 §6).
    pub(super) fn string_element(
        &mut self,
        node: &SyntaxNode,
    ) -> Result<Option<String>, NativeError> {
        let Some((base, index, wide)) = self.string_index(node)? else {
            return Ok(None);
        };
        let text = self.place(&base)?.reference();
        let index = self.count(&index)?;
        let function = if wide { "char_at_wide" } else { "char_at" };
        Ok(Some(self.bind(&format!(
            "lib::{function}(cx, ({text}).view(), {index})?"
        ))))
    }

    /// `text[index] := value`, when `target` indexes a string; whether it does.
    pub(super) fn string_element_write(
        &mut self,
        target: &SyntaxNode,
        value: &SyntaxNode,
    ) -> Result<bool, NativeError> {
        let Some((base, index, wide)) = self.string_index(target)? else {
            return Ok(false);
        };
        let text = self.place(&base)?.reference();
        let index = self.count(&index)?;
        let (elem, function) = if wide {
            (Elem::WChar, "set_char_wide")
        } else {
            (Elem::Char, "set_char")
        };
        let char_ty = self.model.types.elem(elem);
        let character = self.scalar_to(value, char_ty)?;
        self.line(&format!(
            "lib::{function}(cx, ({text}).view(), {index}, {character})?;"
        ));
        Ok(true)
    }
}

/// A function's result in the temporary `temp` of `capacity` bytes, or code units when `wide`.
fn text_result(temp: String, capacity: u32, wide: bool) -> Called {
    let reference = format!("&{temp}");
    Called::Text(vec![if wide {
        Text::WStr {
            reference,
            capacity,
        }
    } else {
        Text::Str {
            reference,
            capacity,
        }
    }])
}

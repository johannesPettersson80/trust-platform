//! The selection functions of texts (IEC 61131-3 Ed.3 §6.6.2.5.10, Table 32; specification 07 §6):
//! `MAX`, `MIN` and `LIMIT` of `STRING`s or of `WSTRING`s, which compare as two texts compare
//! (specification 07 §7), and `SEL` and `MUX` of them. Every input is read once, in order, a
//! selector first; the selected text is copied into a result of the inputs' greatest capacity, as a
//! string function returns its text.

use trust_syntax::syntax::SyntaxNode;

use super::call::Called;
use super::text::Text;
use super::Lowerer;
use crate::native::types::Elem;
use crate::native::NativeError;

impl Lowerer<'_, '_> {
    /// `name`, `MAX`, `MIN`, `LIMIT` or `SEL`, of `values` when its inputs are texts: the selected
    /// text. `MAX` and `MIN` keep the first of equal texts, and `LIMIT` raises its input to `MN`
    /// before it caps it at `MX`, as they treat other values (specification 07 §6.1). `None` for
    /// any other function, and for inputs that are no texts.
    pub(super) fn text_selection(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        values: &[SyntaxNode],
    ) -> Result<Option<Called>, NativeError> {
        let (select, inputs) = match (name, values) {
            ("SEL", [select, inputs @ ..]) => (Some(select), inputs),
            ("MAX" | "MIN" | "LIMIT", inputs) => (None, inputs),
            _ => return Ok(None),
        };
        let Some(wide) = self.text_family(node, inputs)? else {
            return Ok(None);
        };
        let select = match select {
            Some(select) => {
                let boolean = self.model.types.elem(Elem::Bool);
                Some(self.scalar_to(select, boolean)?)
            }
            None => None,
        };
        let texts = self.texts_of(inputs)?;
        let views: Vec<String> = texts
            .iter()
            .map(|(text, _)| format!("({text}).view()"))
            .collect();
        let compare = if wide { "compare_wide" } else { "compare" };
        let chosen = self.temp();
        match (name, views.as_slice(), select) {
            ("SEL", [first, second], Some(select)) => {
                self.line(&format!(
                    "let {chosen} = if {select} {{ {second} }} else {{ {first} }};"
                ));
            }
            ("LIMIT", [low, value, high], None) => {
                self.line(&format!("let mut {chosen} = {value};"));
                self.line(&format!(
                    "if lib::{compare}({chosen}, {low}) == core::cmp::Ordering::Less {{ {chosen} = {low}; }}"
                ));
                self.line(&format!(
                    "if lib::{compare}({chosen}, {high}) == core::cmp::Ordering::Greater {{ {chosen} = {high}; }}"
                ));
            }
            ("MAX" | "MIN", [first, rest @ ..], None) if !rest.is_empty() => {
                let better = if name == "MAX" { "Greater" } else { "Less" };
                self.line(&format!("let mut {chosen} = {first};"));
                for next in rest {
                    self.line(&format!(
                        "if lib::{compare}({next}, {chosen}) == core::cmp::Ordering::{better} {{ {chosen} = {next}; }}"
                    ));
                }
            }
            _ => {
                return Err(self.unsupported(node, format!("{name} of {} texts", texts.len())));
            }
        }
        Ok(Some(self.chosen_text(&chosen, &texts, wide)))
    }

    /// `MUX` of the texts `inputs`, its selector read before them by `mux_selector`: the text the
    /// selector chooses, from 0 (specification 07 §6.1). `None` for inputs that are no texts.
    pub(super) fn text_mux(
        &mut self,
        node: &SyntaxNode,
        selector: &(String, Elem),
        inputs: &[SyntaxNode],
    ) -> Result<Option<Called>, NativeError> {
        let Some(wide) = self.text_family(node, inputs)? else {
            return Ok(None);
        };
        let texts = self.texts_of(inputs)?;
        let index = self.mux_index(node, selector, texts.len())?;
        let last = texts.len().saturating_sub(1);
        let arms: Vec<String> = texts
            .iter()
            .enumerate()
            .map(|(position, (text, _))| {
                let pattern = if position == last {
                    "_".to_string()
                } else {
                    position.to_string()
                };
                format!("{pattern} => ({text}).view()")
            })
            .collect();
        let chosen = self.temp();
        self.line(&format!(
            "let {chosen} = match {index} {{ {} }};",
            arms.join(", ")
        ));
        Ok(Some(self.chosen_text(&chosen, &texts, wide)))
    }

    /// Whether `inputs` are texts of the `WSTRING` family, `false` for `STRING`s; `None` when one
    /// of them is no text. A `STRING` with a `WSTRING`, which trust-hir refuses, is refused.
    fn text_family(
        &mut self,
        node: &SyntaxNode,
        inputs: &[SyntaxNode],
    ) -> Result<Option<bool>, NativeError> {
        let mut family = None;
        for input in inputs {
            if !self.is_text(input) {
                return Ok(None);
            }
            let wide = self.is_wide_text(input);
            match family {
                Some(other) if other != wide => {
                    return Err(self.unsupported(node, "a selection of a STRING and a WSTRING"));
                }
                _ => family = Some(wide),
            }
        }
        Ok(family)
    }

    /// Each of `inputs` as a reference to a string and its capacity, in order.
    fn texts_of(&mut self, inputs: &[SyntaxNode]) -> Result<Vec<(String, u32)>, NativeError> {
        inputs.iter().map(|input| self.text_ref(input)).collect()
    }

    /// The text `chosen`, a view of one of `texts`, copied into a temporary of their greatest
    /// capacity, as the function's result.
    fn chosen_text(&mut self, chosen: &str, texts: &[(String, u32)], wide: bool) -> Called {
        let capacity = texts
            .iter()
            .map(|(_, capacity)| *capacity)
            .max()
            .unwrap_or(0);
        let temp = self.temp();
        let (storage, assign) = if wide {
            ("WStr", "assign_wide")
        } else {
            ("Str", "assign")
        };
        self.line(&format!("let {temp} = {storage}::<{capacity}>::new();"));
        self.line(&format!("lib::{assign}({temp}.view(), {chosen});"));
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
}

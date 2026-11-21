//! The cases with which `trust-dev rust bench` measures a Rust POU, `#[bench_case(x = 1.0, n = 3)]`
//! (specification 48 RU-48).

use syn::ext::IdentExt as _;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Error, MetaNameValue, Token};

use crate::literal::initial_of;
use crate::{Section, Value, Variable};

/// One case of a POU's bench: the values it gives to the POU's inputs and in-outs, in the order the
/// case names them. Every input and in-out it does not name keeps its declared initial value.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// Each named variable's name, as [`Variable::name`] holds it, with its value.
    pub values: Vec<(String, Value)>,
}

/// The cases of the item with the attributes `attributes`, whose variables are `variables`: one for
/// each `#[bench_case(...)]`, none for an item without one. An in-out takes a value only where
/// `in_outs` is set: a program's instances are the configuration's, so no bench provides its
/// in-out.
pub(crate) fn cases(
    attributes: &[Attribute],
    variables: &[Variable],
    in_outs: bool,
) -> Result<Vec<Case>, Error> {
    attributes
        .iter()
        .filter(|attribute| crate::is_bench_case(attribute))
        .map(|attribute| case(attribute, variables, in_outs))
        .collect()
}

fn case(attribute: &Attribute, variables: &[Variable], in_outs: bool) -> Result<Case, Error> {
    // `#[bench_case]` alone is the case of the declared initial values.
    if matches!(attribute.meta, syn::Meta::Path(_)) {
        return Ok(Case { values: Vec::new() });
    }
    let named =
        attribute.parse_args_with(Punctuated::<MetaNameValue, Token![,]>::parse_terminated)?;
    let mut values: Vec<(String, Value)> = Vec::with_capacity(named.len());
    for pair in &named {
        let Some(ident) = pair.path.get_ident() else {
            return Err(Error::new(
                pair.path.span(),
                "a case names an input of the POU: `#[bench_case(x = 1.0)]` (specification 48 \
                 RU-48)",
            ));
        };
        let name = ident.unraw().to_string();
        let Some(variable) = variables.iter().find(|variable| variable.name == name) else {
            return Err(Error::new(
                ident.span(),
                format!(
                    "`{name}` is no variable of the POU: a case gives values to its inputs{} \
                     (specification 48 RU-48)",
                    if in_outs { " and in-outs" } else { "" }
                ),
            ));
        };
        match variable.section {
            Section::Input => {}
            Section::InOut if in_outs => {}
            _ => {
                return Err(Error::new(
                    ident.span(),
                    format!(
                        "`{name}` is no input{} of the POU: a case gives values only to them, the \
                         others start as declared (specification 48 RU-48)",
                        if in_outs { " or in-out" } else { "" }
                    ),
                ))
            }
        }
        if values.iter().any(|(given, _)| *given == name) {
            return Err(Error::new(
                ident.span(),
                format!("the case names `{name}` twice"),
            ));
        }
        let value = initial_of(&pair.value, &variable.ty).map_err(|error| {
            Error::new(
                error.span(),
                format!(
                    "the case's value of `{name}` is written as `#[init(...)]` writes it: {error}"
                ),
            )
        })?;
        values.push((name, value));
    }
    Ok(Case { values })
}

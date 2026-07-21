use syn::punctuated::Punctuated;
use syn::{Attribute, Meta, Token};

const CAN_BE_FALSE: u8 = 1;
const CAN_BE_TRUE: u8 = 2;

/// Return true only when a `cfg` predicate cannot be enabled outside tests.
///
/// Unknown target and feature predicates remain conservatively both true and
/// false. This prevents `cfg(not(test))` and `cfg(any(test, feature = ...))`
/// production code from disappearing from the denominator.
pub fn is_test_only(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        if !attribute.path().is_ident("cfg") {
            return false;
        }
        attribute
            .parse_args::<Meta>()
            .is_ok_and(|predicate| possible_values_without_test(&predicate) == CAN_BE_FALSE)
    })
}

fn possible_values_without_test(predicate: &Meta) -> u8 {
    match predicate {
        Meta::Path(path) if path.is_ident("test") => CAN_BE_FALSE,
        Meta::Path(_) | Meta::NameValue(_) => CAN_BE_FALSE | CAN_BE_TRUE,
        Meta::List(list) if list.path.is_ident("all") => {
            let Some(children) = parse_children(list) else {
                return CAN_BE_FALSE | CAN_BE_TRUE;
            };
            combine_all(&children)
        }
        Meta::List(list) if list.path.is_ident("any") => {
            let Some(children) = parse_children(list) else {
                return CAN_BE_FALSE | CAN_BE_TRUE;
            };
            combine_any(&children)
        }
        Meta::List(list) if list.path.is_ident("not") => {
            let Some(children) = parse_children(list) else {
                return CAN_BE_FALSE | CAN_BE_TRUE;
            };
            if children.len() != 1 {
                return CAN_BE_FALSE | CAN_BE_TRUE;
            }
            invert(possible_values_without_test(&children[0]))
        }
        Meta::List(_) => CAN_BE_FALSE | CAN_BE_TRUE,
    }
}

fn parse_children(list: &syn::MetaList) -> Option<Vec<Meta>> {
    list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        .ok()
        .map(|items| items.into_iter().collect())
}

fn combine_all(children: &[Meta]) -> u8 {
    let mut result = CAN_BE_TRUE;
    for child in children {
        result = combine(result, possible_values_without_test(child), |left, right| left && right);
    }
    result
}

fn combine_any(children: &[Meta]) -> u8 {
    let mut result = CAN_BE_FALSE;
    for child in children {
        result = combine(result, possible_values_without_test(child), |left, right| left || right);
    }
    result
}

fn combine(left: u8, right: u8, operation: impl Fn(bool, bool) -> bool) -> u8 {
    let mut result = 0;
    for left_value in values(left) {
        for right_value in values(right) {
            result |= if operation(left_value, right_value) {
                CAN_BE_TRUE
            } else {
                CAN_BE_FALSE
            };
        }
    }
    result
}

fn values(mask: u8) -> impl Iterator<Item = bool> {
    [
        (mask & CAN_BE_FALSE != 0).then_some(false),
        (mask & CAN_BE_TRUE != 0).then_some(true),
    ]
    .into_iter()
    .flatten()
}

fn invert(mask: u8) -> u8 {
    let mut result = 0;
    if mask & CAN_BE_FALSE != 0 {
        result |= CAN_BE_TRUE;
    }
    if mask & CAN_BE_TRUE != 0 {
        result |= CAN_BE_FALSE;
    }
    result
}

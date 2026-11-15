//! The budget of a Rust POU, `#[budget("15us")]` (specification 48 RU-46).

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::{Attribute, Error, LitStr};

/// The budget in nanoseconds of the item with the attributes `attributes`, whose name is at `item`.
pub(crate) fn of(attributes: &[Attribute], item: Span) -> Result<u64, Error> {
    let mut budgets = attributes
        .iter()
        .filter(|attribute| crate::is_budget(attribute));
    let Some(budget) = budgets.next() else {
        return Err(Error::new(
            item,
            "a Rust POU declares its budget, the longest one call may take on the slowest target \
             of its project: `#[budget(\"20us\")]` (specification 48 RU-46)",
        ));
    };
    if let Some(second) = budgets.next() {
        return Err(Error::new(second.span(), "a Rust POU declares one budget"));
    }
    let text: LitStr = budget.parse_args()?;
    nanoseconds(&text.value()).ok_or_else(|| {
        Error::new(
            text.span(),
            "a budget is a duration above zero in whole nanoseconds: `500ns`, `15us`, `1.5ms` or \
             `2s`",
        )
    })
}

/// The nanoseconds of `text`, a decimal number with the unit `ns`, `us` (also `µs`), `ms` or `s`;
/// `None` for another text, for zero and for a fraction of a nanosecond.
pub(crate) fn nanoseconds(text: &str) -> Option<u64> {
    let (number, scale) = [
        ("ns", 1_u64),
        ("us", 1_000),
        ("µs", 1_000),
        ("ms", 1_000_000),
        ("s", 1_000_000_000),
    ]
    .into_iter()
    .find_map(|(unit, scale)| text.strip_suffix(unit).map(|number| (number, scale)))?;
    let (whole, fraction) = match number.split_once('.') {
        Some((whole, fraction)) if !fraction.is_empty() => (whole, fraction),
        Some(_) => return None,
        None => (number, ""),
    };
    let digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    if whole.is_empty() || !digits(whole) || !digits(fraction) {
        return None;
    }
    let mut total = whole.parse::<u64>().ok()?.checked_mul(scale)?;
    if !fraction.is_empty() {
        let places = u32::try_from(fraction.len()).ok()?;
        let denominator = 10_u64.checked_pow(places)?;
        let numerator = fraction.parse::<u64>().ok()?.checked_mul(scale)?;
        if numerator % denominator != 0 {
            return None;
        }
        total = total.checked_add(numerator / denominator)?;
    }
    (total > 0).then_some(total)
}

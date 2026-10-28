//! The chips of truST's chip files (specification 34, TARGET-03): a runtime whose `status`
//! reports a chip's name as its platform (decision O5) is the truST firmware of that chip, and
//! its host is a chip (specification 25 §5.12).

use std::sync::OnceLock;

use trust_chips::{Catalog, ChipFile};

/// The chip files that this build embeds, read once.
fn catalog() -> Option<&'static Catalog> {
    static CATALOG: OnceLock<Option<Catalog>> = OnceLock::new();
    CATALOG.get_or_init(|| Catalog::embedded().ok()).as_ref()
}

/// The manufacturer's name of the chip that `platform` names, such as `STM32U5A9` for
/// `stm32u5a9`; `None` for the platform of a computer.
pub(crate) fn model(platform: &str) -> Option<String> {
    catalog()?
        .chip(platform.trim())
        .map(|chip| chip.identity.model.clone())
}

/// Every chip of the chip files, in the catalog's order: the `Runtimes` group of `+ Add` offers
/// truST for each (specification 25 §5.14).
pub(crate) fn all() -> &'static [ChipFile] {
    catalog().map(Catalog::chips).unwrap_or_default()
}

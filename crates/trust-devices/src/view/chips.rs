//! The chips of truST's chip files (specification 34, TARGET-03): a runtime whose `status`
//! reports a chip's name as its platform (decision O5) is the truST firmware of that chip, and
//! its host is a chip (specification 25 §5.12).

use std::sync::OnceLock;

use trust_chips::Catalog;

/// The chip files that this build embeds, read once.
fn catalog() -> Option<&'static Catalog> {
    static CATALOG: OnceLock<Option<Catalog>> = OnceLock::new();
    CATALOG.get_or_init(|| Catalog::embedded().ok()).as_ref()
}

/// The manufacturer's name of the chip that `platform` names, such as `STM32F407` for
/// `stm32f407`; `None` for the platform of a computer.
pub(crate) fn model(platform: &str) -> Option<String> {
    catalog()?
        .chip(platform.trim())
        .map(|chip| chip.identity.model.clone())
}

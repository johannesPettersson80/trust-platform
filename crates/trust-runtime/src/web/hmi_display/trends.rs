//! The values the active bundle's trends show (specification 39, §5.9 item 1), which the runtime
//! keeps in its own history: every trend pen of the pages and popups, and of the panels their
//! objects open or place, bound to each object's instance; limit bands are no pens.

use std::collections::{BTreeMap, BTreeSet};

use super::faceplates::FACEPLATE_PREFIX;
use super::files::HmiFiles;
use crate::hmi::bundle::store::BundleStore;
use crate::hmi::exchange::HmiExchange;
use crate::hmi::page_files::{HmiClick, HmiPage, PLACED_FACEPLATE};

/// A trend's bindings that are no pens: its limit bands (§5.7) and its visibility (§18 phase 5).
const NOT_PENS: [&str; 3] = ["high-limit", "low-limit", "visible"];

/// The paths the trends of the pages and popups show, each once, with those of the panels their
/// objects open or place, so a panel's trend opens filled for every instance it is shown for. An
/// object's `block` chooses among a panel's component definitions: no instance is read from the
/// running program.
pub(in crate::web) fn trended_paths(files: &HmiFiles) -> Vec<String> {
    let pages = files
        .page_names()
        .into_iter()
        .filter_map(|name| Some((name.clone(), files.page(&name)?.ok()?)));
    let popups = files
        .popup_names()
        .into_iter()
        .filter_map(|name| Some((format!("popup:{name}"), files.popup(&name)?.ok()?.page)));
    let sources: Vec<(String, HmiPage)> = pages.chain(popups).collect();
    let blocks: BTreeMap<String, String> = sources
        .iter()
        .flat_map(|(_, page)| &page.objects)
        .filter_map(|object| {
            Some((
                object.instance.as_ref()?.to_ascii_lowercase(),
                object.block.clone()?,
            ))
        })
        .collect();
    let type_of = |path: &str| blocks.get(&path.to_ascii_lowercase()).cloned();
    let mut paths = BTreeSet::new();
    for (source, page) in &sources {
        add_pens(page, &mut paths);
        for object in page.objects.iter().filter(|object| {
            object.kind == PLACED_FACEPLATE
                || matches!(object.effective_click(), Some(HmiClick::Faceplate { .. }))
        }) {
            let address = format!("{FACEPLATE_PREFIX}{source}/{}", object.id);
            if let Some(Ok((faceplate, _))) = files.object_faceplate(&address, object, &type_of) {
                add_pens(&faceplate.popup.page, &mut paths);
            }
        }
    }
    paths.into_iter().collect()
}

/// Adds the pens of a page's trends; a field placeholder (`$Level`) names no variable.
fn add_pens(page: &HmiPage, paths: &mut BTreeSet<String>) {
    for object in page.objects.iter().filter(|object| object.kind == "trend") {
        for (property, binding) in &object.bind {
            if !NOT_PENS.contains(&property.as_str()) && !binding.path.starts_with('$') {
                paths.insert(binding.path.clone());
            }
        }
    }
}

/// Gives the runtime the active bundle's trended values to keep: at the start and after each
/// activation of a bundle.
pub(in crate::web) fn share_trended_paths(store: &BundleStore, exchange: &HmiExchange) {
    let paths = store.read(|root| trended_paths(&HmiFiles::new(root.to_path_buf())));
    exchange.keep_trends(&paths);
}

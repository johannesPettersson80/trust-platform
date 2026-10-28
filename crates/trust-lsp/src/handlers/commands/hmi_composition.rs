//! The editor's view of a page with the panels docked on it (specification 39, §21.1 item 3),
//! composed by the runtime's loader exactly as the display composes it.
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tower_lsp::lsp_types::Url;
use trust_runtime::hmi::page_files as native;

use super::hmi_files::{file_errors, refused};

pub(super) fn compose(
    root: Option<Url>,
    file: &str,
    text: &str,
    open: BTreeMap<String, String>,
) -> Value {
    let Some(root) = root.and_then(|root| root.to_file_path().ok()) else {
        return refused("composition needs a file root_uri".to_owned());
    };
    let name = file.replace('\\', "/");
    let Some(name) = name
        .strip_prefix("hmi/pages/")
        .and_then(|file| file.strip_suffix(".toml"))
    else {
        return refused("composition needs a page under hmi/pages".to_owned());
    };
    if name.contains('/') {
        return refused("invalid page name".to_owned());
    }
    let snapshot = match super::hmi_snapshot::paths(&root, &open) {
        Ok(snapshot) => snapshot,
        Err(error) => return refused(error),
    };
    let bundle = match super::hmi_authoring::bundle::read_edited(&root, &snapshot) {
        Ok(bundle) => bundle,
        Err(error) => return refused(error.to_string()),
    };
    let page = match native::parse_page(file, text) {
        Ok(page) => page,
        Err(errors) => return file_errors(&errors),
    };
    let panels = bundle.docked_panels();
    let page = if panels.is_empty() {
        page
    } else {
        let Some(screen) = bundle.files.get("hmi/screen.toml") else {
            return refused("hmi/screen.toml is missing".to_owned());
        };
        let screen = match native::parse_screen("hmi/screen.toml", &String::from_utf8_lossy(screen))
        {
            Ok(screen) => screen,
            Err(errors) => return file_errors(&errors),
        };
        match native::compose_docked(page, name, screen.size, &panels) {
            Ok(page) => page,
            Err(message) => return refused(message),
        }
    };
    let text = native::write_page(&page);
    match super::hmi_authoring::documents::parse("page", file, &text) {
        Ok(model) => json!({"ok": true, "model": model}),
        Err(errors) => file_errors(&errors),
    }
}

//! What the runtime display loads (specification 39, §4.6, §4.9, §11.1): the screen layout, the
//! page list and one page, or one popup.

use serde_json::{json, Value};

use crate::hmi::access::config::WriteRule;
use crate::hmi::access::policy::control_roles;
use crate::hmi::catalogue::Catalogue;
use crate::hmi::page_files::HmiFaceplate;

use super::files::{errors_json, is_page_name, HmiFiles};

/// The screen layout, the page list and one page: the one named, or the first of the list, with
/// the role each of its controls needs under the write policy (§5.9 item 5), and the faceplates
/// placed on it, each bound to its instance as its popup is (§8.9); `type_of` gives an instance's
/// type, which chooses a faceplate's pair.
pub(super) fn page_data(
    files: &HmiFiles,
    name: Option<&str>,
    language: Option<&str>,
    policy: &[WriteRule],
    type_of: &dyn Fn(&str) -> Option<String>,
) -> (u16, Value) {
    let mut errors = Vec::new();
    if !files.exists() {
        return (
            200,
            json!({ "screen": null, "pages": [], "navigation": [], "page": null, "errors": [] }),
        );
    }
    let (screen, languages) = match files.screen() {
        Ok(screen) => {
            let languages = screen.languages.clone();
            (json!(screen), languages)
        }
        Err(screen_errors) => {
            errors.extend(screen_errors);
            (Value::Null, vec!["en".to_string()])
        }
    };
    // The language asked for when the project has it, else its first (§14.5).
    let language = language
        .filter(|asked| languages.iter().any(|tag| tag == asked))
        .map(str::to_string)
        .unwrap_or_else(|| languages[0].clone());
    let project_texts = match files.texts(&language) {
        Some(Ok(texts)) => Some(texts),
        Some(Err(text_errors)) => {
            errors.extend(text_errors);
            None
        }
        None => None,
    };
    let texts = crate::hmi::texts::merged_texts(&language, project_texts.as_ref());
    let language_list: Vec<Value> = languages
        .iter()
        .map(|tag| json!({ "tag": tag, "name": crate::hmi::texts::language_name(tag) }))
        .collect();
    let pages = files.page_names();
    let navigation = files.navigation();
    let name = match name {
        Some(name) if !is_page_name(name) => {
            return (
                400,
                json!({ "error": "a page name has letters, digits, - and _ only" }),
            );
        }
        Some(name) => Some(name.to_string()),
        // `/hmi` opens the first page of the list, or the first by name when every page is hidden.
        None => screen["start_page"]
            .as_str()
            .filter(|name| pages.iter().any(|page| page.as_str() == *name))
            .map(str::to_string)
            .or_else(|| {
                navigation
                    .first()
                    .and_then(|entry| entry["name"].as_str().map(str::to_string))
                    .or_else(|| pages.first().cloned())
            }),
    };
    let mut symbols = Vec::new();
    let mut roles = json!({});
    let mut placed_faceplates = serde_json::Map::new();
    let page = match name.as_deref().map(|name| (name, files.page(name))) {
        None => Value::Null,
        Some((name, None)) => {
            return (
                404,
                json!({ "error": format!("the project has no page `{name}`") }),
            )
        }
        Some((name, Some(Ok(page)))) => {
            // The project's own symbols the page places come with it (§14.4).
            let (placed, symbol_errors) =
                files.symbols_for(page.objects.iter().map(|object| object.kind.as_str()));
            symbols = placed;
            errors.extend(symbol_errors);
            roles = json!(control_roles(&page, Catalogue::shipped(), policy));
            for (id, faceplate) in files.placed_faceplates(name, &page, type_of) {
                match faceplate {
                    Some(Ok(faceplate)) => {
                        let entry = served(name, &id, &faceplate, policy);
                        placed_faceplates.insert(id, entry);
                    }
                    Some(Err(faceplate_errors)) => errors.extend(faceplate_errors),
                    // The display says the project lacks it (§8.9).
                    None => {
                        placed_faceplates.insert(id, Value::Null);
                    }
                }
            }
            json!({ "name": name, "model": page })
        }
        Some((_, Some(Err(page_errors)))) => {
            errors.extend(page_errors);
            Value::Null
        }
    };
    (
        200,
        json!({
            "screen": screen,
            "pages": pages,
            "navigation": navigation,
            "page": page,
            "roles": roles,
            "faceplates": placed_faceplates,
            "symbols": symbols,
            "errors": errors_json(&errors),
            "bundle": files.version(),
            "language": language,
            "languages": language_list,
            "texts": texts,
            "formats": files.formats(),
        }),
    )
}

/// The faceplate an object opens, bound to its instance (specification 39, §8.7), served as a
/// popup named by its address, `faceplate:<page>/<object id>`.
pub(super) fn faceplate_data(
    files: &HmiFiles,
    source: &str,
    type_of: &dyn Fn(&str) -> Option<String>,
    policy: &[WriteRule],
) -> (u16, Value) {
    let address = format!("{}{source}", super::faceplates::FACEPLATE_PREFIX);
    match files.faceplate_at(&address, type_of) {
        None => (
            404,
            json!({ "error": format!("no object `{source}` opens a faceplate the project has") }),
        ),
        Some(Err(errors)) => (
            200,
            json!({ "popup": null, "errors": errors_json(&errors), "bundle": files.version() }),
        ),
        Some(Ok(faceplate)) => (
            200,
            json!({
                "popup": served_at(address, &faceplate, policy),
                "formats": files.formats(),
                "errors": [],
                "bundle": files.version(),
            }),
        ),
    }
}

/// A faceplate as the display takes it: its address, the role each of its controls needs and
/// its bound model.
fn served_at(address: String, faceplate: &HmiFaceplate, policy: &[WriteRule]) -> Value {
    json!({
        "name": address,
        "roles": control_roles(&faceplate.popup.page, Catalogue::shipped(), policy),
        "model": faceplate,
    })
}

/// A faceplate placed on a page, named by its object as its popup is.
fn served(page: &str, id: &str, faceplate: &HmiFaceplate, policy: &[WriteRule]) -> Value {
    served_at(
        format!("{}{page}/{id}", super::faceplates::FACEPLATE_PREFIX),
        faceplate,
        policy,
    )
}

/// One popup with its design size and pin, and the version of the files it is drawn from.
pub(super) fn popup_data(
    files: &HmiFiles,
    name: Option<&str>,
    policy: &[WriteRule],
) -> (u16, Value) {
    let Some(name) = name.filter(|name| is_page_name(name)) else {
        return (
            400,
            json!({ "error": "name a popup: /api/hmi/popup?name=<popup>" }),
        );
    };
    match files.popup(name) {
        None => (
            404,
            json!({ "error": format!("the project has no popup `{name}`") }),
        ),
        Some(Err(errors)) => (
            200,
            json!({ "popup": null, "errors": errors_json(&errors), "bundle": files.version() }),
        ),
        Some(Ok(popup)) => {
            let (symbols, errors) =
                files.symbols_for(popup.page.objects.iter().map(|object| object.kind.as_str()));
            (
                200,
                json!({
                    "popup": {
                        "name": name,
                        "roles": control_roles(&popup.page, Catalogue::shipped(), policy),
                        "model": popup,
                    },
                    "symbols": symbols,
                    "formats": files.formats(),
                    "errors": errors_json(&errors),
                    "bundle": files.version(),
                }),
            )
        }
    }
}

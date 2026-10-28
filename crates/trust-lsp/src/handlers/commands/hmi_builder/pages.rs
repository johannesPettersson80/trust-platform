//! The Pages panel: navigation order, names and visibility (specification 39, §3.3 item 8, §4.6;
//! row H3).
//!
//! The extension passes the text of every page file `hmi/pages/<name>.toml` by its name (an open
//! editor's text, saved or not, else the file's) and applies the files the answer gives:
//!
//! - `list { pages }` → `{ pages: [{ name, title, hidden }] }`: the pages in the navigation's
//!   order (§4 item 6), hidden ones included: by `order`, the pages without one last, then by
//!   name. A page that does not load is listed by its name.
//! - `add { pages }` → `{ name, write, create }`: an empty page `page-<n>`, the first free number
//!   from 1, titled `Page <n>`, last in the navigation.
//! - `rename { pages, name, title }` → `{ write }`: the page's title, trimmed; an empty title
//!   changes nothing.
//! - `duplicate { pages, name }` → `{ name, write, create }`: a copy `<name>-copy` (then
//!   `<name>-copy-2`, …) titled `<title> (copy)`, right after the page; the pages after it move one
//!   place on.
//! - `move { pages, name, before }` → `{ write }`: the page before the page `before`, or last when
//!   `before` is null or names no page.
//! - `hide { pages, name, hidden }` → `{ write }`: the page left out of the navigation, or back in.
//!
//! `write` holds the pages whose file changes and `create` the new page, each `{ name, text }`
//! with the canonical text of the runtime's page writer (§15.2). Adding, copying and moving number
//! the pages from 0 in their new order, writing only the pages whose order changes; a page that
//! does not load is left as it is. Deleting a page needs no rule: the extension asks and deletes.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{json, Value};
use trust_runtime::hmi::page_files::{parse_page, write_page, HmiPage};

use super::decode;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Args {
    List {
        pages: BTreeMap<String, String>,
    },
    Add {
        pages: BTreeMap<String, String>,
    },
    Rename {
        pages: BTreeMap<String, String>,
        name: String,
        title: String,
    },
    Duplicate {
        pages: BTreeMap<String, String>,
        name: String,
    },
    Move {
        pages: BTreeMap<String, String>,
        name: String,
        #[serde(default)]
        before: Option<String>,
    },
    Hide {
        pages: BTreeMap<String, String>,
        name: String,
        hidden: bool,
    },
}

/// The answer of `trust-lsp.hmiPages`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    match decode::<Args>(argument)? {
        Args::List { pages } => {
            Ok(json!({ "ok": true, "pages": Navigation::new(&pages).entries() }))
        }
        Args::Add { pages } => add(&pages),
        Args::Rename { pages, name, title } => rename(&pages, &name, &title),
        Args::Duplicate { pages, name } => duplicate(&pages, &name),
        Args::Move {
            pages,
            name,
            before,
        } => Ok(move_page(&pages, &name, before.as_deref())),
        Args::Hide {
            pages,
            name,
            hidden,
        } => hide(&pages, &name, hidden),
    }
}

/// The first free name from `stem`: `stem` itself, then `stem-2`, `stem-3`, … A copy's name and a
/// page made from the program take the first free one (§3.3 item 8, §9 item 3).
pub(super) fn free_name(stem: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(stem) {
        return stem.to_string();
    }
    let mut number = 2_u32;
    loop {
        let candidate = format!("{stem}-{number}");
        if !taken(&candidate) {
            return candidate;
        }
        number += 1;
    }
}

/// The project's pages: those that load, and every page's name in the navigation's order.
struct Navigation {
    loaded: BTreeMap<String, HmiPage>,
    names: Vec<String>,
}

impl Navigation {
    fn new(pages: &BTreeMap<String, String>) -> Self {
        let loaded: BTreeMap<String, HmiPage> = pages
            .iter()
            .filter_map(|(name, text)| Some((name.clone(), parse_page(&file_of(name), text).ok()?)))
            .collect();
        let order = |name: &String| loaded.get(name).and_then(|page| page.order);
        let mut names: Vec<String> = pages.keys().cloned().collect();
        names.sort_by(|a, b| {
            let (first, second) = (order(a), order(b));
            (first.is_none(), first)
                .cmp(&(second.is_none(), second))
                .then_with(|| a.cmp(b))
        });
        Self { loaded, names }
    }

    /// The Pages panel's entries: name, title (the name for a page that does not load) and
    /// whether the navigation hides the page.
    fn entries(&self) -> Vec<Value> {
        self.names
            .iter()
            .map(|name| {
                let page = self.loaded.get(name);
                json!({
                    "name": name,
                    "title": page.map_or(name.as_str(), |page| page.title.as_str()),
                    "hidden": page.is_some_and(|page| page.hidden),
                })
            })
            .collect()
    }

    /// The names in the navigation's order.
    fn sequence(&self) -> Vec<&str> {
        self.names.iter().map(String::as_str).collect()
    }

    /// The files that give each page of `sequence` its place as its order, for the pages whose
    /// order is another; a name that is not a loaded page (a new one, or one that does not load)
    /// is passed over.
    fn renumbered(&self, sequence: &[&str]) -> Vec<Value> {
        sequence
            .iter()
            .enumerate()
            .filter_map(|(place, name)| {
                let page = self.loaded.get(*name)?;
                let order = place_order(place);
                (page.order != Some(order)).then(|| {
                    let mut page = page.clone();
                    page.order = Some(order);
                    file(name, &page)
                })
            })
            .collect()
    }
}

/// The page file of `name`, as the loader's errors name it.
fn file_of(name: &str) -> String {
    format!("hmi/pages/{name}.toml")
}

/// A place in the navigation as a page's `order`.
fn place_order(place: usize) -> u32 {
    u32::try_from(place).unwrap_or(u32::MAX)
}

/// A page file to write: its name and canonical text.
fn file(name: &str, page: &HmiPage) -> Value {
    json!({ "name": name, "text": write_page(page) })
}

/// The page `name` when it loads; an error when the project has no such page.
fn page_named<'a>(
    navigation: &'a Navigation,
    pages: &BTreeMap<String, String>,
    name: &str,
) -> Result<Option<&'a HmiPage>, String> {
    if pages.contains_key(name) {
        Ok(navigation.loaded.get(name))
    } else {
        Err(format!("The project has no page \"{name}\"."))
    }
}

fn add(pages: &BTreeMap<String, String>) -> Result<Value, String> {
    let navigation = Navigation::new(pages);
    let mut number = 1_u32;
    while pages.contains_key(&format!("page-{number}")) {
        number += 1;
    }
    let name = format!("page-{number}");
    let sequence = navigation.sequence();
    let text = super::files::written(
        "page",
        json!({
            "title": format!("Page {number}"),
            "order": place_order(sequence.len()),
            "object": [],
            "pipe": [],
        }),
    )?;
    Ok(json!({
        "ok": true,
        "name": name,
        "write": navigation.renumbered(&sequence),
        "create": [{ "name": name, "text": text }],
    }))
}

fn rename(pages: &BTreeMap<String, String>, name: &str, title: &str) -> Result<Value, String> {
    let title = title.trim();
    if title.is_empty() {
        return Ok(json!({ "ok": true, "write": [] }));
    }
    let navigation = Navigation::new(pages);
    let write: Vec<Value> = page_named(&navigation, pages, name)?
        .filter(|page| page.title != title)
        .map(|page| {
            let mut page = page.clone();
            page.title = title.to_string();
            file(name, &page)
        })
        .into_iter()
        .collect();
    Ok(json!({ "ok": true, "write": write }))
}

fn duplicate(pages: &BTreeMap<String, String>, name: &str) -> Result<Value, String> {
    let navigation = Navigation::new(pages);
    let Some(page) = page_named(&navigation, pages, name)? else {
        return Ok(json!({ "ok": true, "write": [], "create": [] }));
    };
    let copy_name = free_name(&format!("{name}-copy"), |candidate| {
        pages.contains_key(candidate)
    });
    let mut sequence = navigation.sequence();
    let at = sequence
        .iter()
        .position(|listed| *listed == name)
        .map_or(sequence.len(), |at| at + 1);
    sequence.insert(at, &copy_name);
    let write = navigation.renumbered(&sequence);
    let mut copy = page.clone();
    copy.title = format!("{} (copy)", page.title);
    copy.order = Some(place_order(at));
    Ok(json!({
        "ok": true,
        "name": copy_name,
        "write": write,
        "create": [file(&copy_name, &copy)],
    }))
}

fn move_page(pages: &BTreeMap<String, String>, name: &str, before: Option<&str>) -> Value {
    let navigation = Navigation::new(pages);
    let mut sequence = navigation.sequence();
    let Some(from) = sequence.iter().position(|listed| *listed == name) else {
        return json!({ "ok": true, "write": [] });
    };
    if before == Some(name) {
        return json!({ "ok": true, "write": [] });
    }
    let moved = sequence.remove(from);
    let at = before
        .and_then(|before| sequence.iter().position(|listed| *listed == before))
        .unwrap_or(sequence.len());
    sequence.insert(at, moved);
    json!({ "ok": true, "write": navigation.renumbered(&sequence) })
}

fn hide(pages: &BTreeMap<String, String>, name: &str, hidden: bool) -> Result<Value, String> {
    let navigation = Navigation::new(pages);
    let write: Vec<Value> = page_named(&navigation, pages, name)?
        .filter(|page| page.hidden != hidden)
        .map(|page| {
            let mut page = page.clone();
            page.hidden = hidden;
            file(name, &page)
        })
        .into_iter()
        .collect();
    Ok(json!({ "ok": true, "write": write }))
}

#[cfg(test)]
#[path = "pages_tests.rs"]
mod tests;

//! The project's HMI files as the runtime display reads them (specification 39, §4.6, §4.9,
//! §11.1): the screen layout, the page list, one page and one popup, read when they are opened.
//! The bundle's version and what derives from all its files come from its snapshot (§21.5 item 4).

mod snapshot;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use serde_json::{json, Value};

use crate::hmi::page_files::{
    parse_page, parse_popup, parse_screen, HmiPage, HmiPopup, HmiScreen, PageFileError,
};

const SCREEN_FILE: &str = "hmi/screen.toml";
const PAGES_FOLDER: &str = "hmi/pages";
const POPUPS_FOLDER: &str = "hmi/popups";
const PANELS_FOLDER: &str = "hmi/panels";
/// How a command or stream names a popup instead of a page (§4.9).
const POPUP_PREFIX: &str = "popup:";
const MAX_PAGE_NAME: usize = 64;

/// Whether a name can be a page address: letters, digits, `-` and `_`.
pub(in crate::web) fn is_page_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_PAGE_NAME
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

pub(in crate::web) struct HmiFiles {
    root: PathBuf,
    /// The active bundle's snapshot, taken once for whatever this request asks.
    snapshot: OnceLock<Arc<snapshot::Snapshot>>,
}

impl HmiFiles {
    pub(in crate::web) fn new(root: PathBuf) -> Self {
        Self {
            root,
            snapshot: OnceLock::new(),
        }
    }

    /// The active bundle as a snapshot: read and hashed when it was activated or its files last
    /// changed, never again for a request (§21.5 item 4).
    fn snapshot(&self) -> &snapshot::Snapshot {
        self.snapshot.get_or_init(|| snapshot::current(&self.root))
    }

    /// The folder the files are read from.
    pub(super) fn root(&self) -> &std::path::Path {
        &self.root
    }

    /// Whether the project has an HMI: its screen file exists.
    pub(in crate::web) fn exists(&self) -> bool {
        self.root.join(SCREEN_FILE).is_file()
    }

    /// The project's texts in a language, when it has a file for it (§14.5).
    pub(super) fn texts(
        &self,
        language: &str,
    ) -> Option<Result<crate::hmi::page_files::HmiTexts, Vec<PageFileError>>> {
        // Only a language tag names a file, so nothing outside `hmi/texts/` is read.
        if !crate::hmi::page_files::is_language_tag(language) {
            return None;
        }
        let file = format!("hmi/texts/{language}.toml");
        let text = std::fs::read_to_string(self.root.join(&file)).ok()?;
        Some(crate::hmi::page_files::parse_texts(&file, &text))
    }

    /// The project's symbols that objects of these types place (§14.4), and the problems of the
    /// symbol files among them.
    pub(super) fn symbols_for<'a>(
        &self,
        kinds: impl Iterator<Item = &'a str>,
    ) -> (Vec<Value>, Vec<PageFileError>) {
        let wanted: std::collections::BTreeSet<&str> =
            kinds.filter_map(|kind| kind.strip_prefix("my:")).collect();
        if wanted.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let (symbols, errors) = crate::hmi::my_symbols::project_symbols(&self.root);
        let symbols = symbols
            .iter()
            .filter(|symbol| wanted.contains(symbol.name.as_str()))
            .map(|symbol| serde_json::to_value(symbol).unwrap_or(Value::Null))
            .collect();
        let errors = errors
            .into_iter()
            .filter(|error| {
                wanted.iter().any(|name| {
                    error.file.starts_with(&format!(
                        "{}{name}.",
                        crate::hmi::my_symbols::SYMBOLS_FOLDER
                    ))
                })
            })
            .collect();
        (symbols, errors)
    }

    pub(super) fn screen(&self) -> Result<HmiScreen, Vec<PageFileError>> {
        let text = read(&self.root.join(SCREEN_FILE), SCREEN_FILE)?;
        parse_screen(SCREEN_FILE, &text)
    }

    /// The names of the pages, sorted; files whose name cannot be an address are left out.
    pub(super) fn page_names(&self) -> Vec<String> {
        self.names(PAGES_FOLDER)
    }

    /// The names of the popups, sorted, as [`Self::page_names`] (§4.9).
    pub(super) fn popup_names(&self) -> Vec<String> {
        self.names(POPUPS_FOLDER)
    }

    fn names(&self, folder: &str) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(self.root.join(folder)) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_file())
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let stem = name.strip_suffix(".toml")?;
                is_page_name(stem).then(|| stem.to_string())
            })
            .collect();
        names.sort();
        names
    }

    /// The version of the HMI files a page was drawn from (§12.8, §16.4): the bundle's short
    /// version; empty when the files cannot be read.
    pub(in crate::web) fn version(&self) -> String {
        self.snapshot().version.clone()
    }

    /// Project presentation defaults do not alter authored page or popup properties (§5.9).
    pub(super) fn formats(
        &self,
    ) -> std::collections::BTreeMap<String, crate::hmi::presentation::ValueFormat> {
        self.snapshot().formats.clone()
    }

    /// One page, with the project's panels docked on it composed in (§21.1 item 3), or `None`
    /// when the project has no page of that name.
    pub(in crate::web) fn page(&self, name: &str) -> Option<Result<HmiPage, Vec<PageFileError>>> {
        let page = self.file(PAGES_FOLDER, name, parse_page)?;
        Some(page.and_then(|page| self.with_docked_panels(name, page)))
    }

    /// The page with the docked panels that apply to it; unchanged when none does.
    fn with_docked_panels(&self, name: &str, page: HmiPage) -> Result<HmiPage, Vec<PageFileError>> {
        let panels = self.docked_panels();
        if panels.is_empty() {
            return Ok(page);
        }
        let screen = self.screen()?;
        crate::hmi::page_files::compose_docked(page, name, screen.size, &panels).map_err(
            |message| {
                vec![PageFileError {
                    file: format!("{PAGES_FOLDER}/{name}.toml"),
                    line: 1,
                    message,
                }]
            },
        )
    }

    /// The project's panels docked on pages, by name; a panel file that does not load is left
    /// out, its own check reports it.
    fn docked_panels(&self) -> Vec<(String, crate::hmi::page_files::HmiFaceplate)> {
        let folder = self.root.join(PANELS_FOLDER);
        let Ok(entries) = std::fs::read_dir(&folder) else {
            return Vec::new();
        };
        let mut panels: Vec<(String, crate::hmi::page_files::HmiFaceplate)> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let stem = name.strip_suffix(".toml")?;
                if !is_page_name(stem) {
                    return None;
                }
                let file = format!("{PANELS_FOLDER}/{name}");
                let text = std::fs::read_to_string(entry.path()).ok()?;
                let panel = crate::hmi::page_files::parse_faceplate(&file, &text).ok()?;
                let docked = panel
                    .panel
                    .as_ref()
                    .is_some_and(|dock| dock.scope != crate::hmi::page_files::HmiDockScope::Manual);
                docked.then(|| (stem.to_string(), panel))
            })
            .collect();
        panels.sort_by(|a, b| a.0.cmp(&b.0));
        panels
    }

    /// One popup, or `None` when the project has no popup of that name (§4.9).
    pub(in crate::web) fn popup(&self, name: &str) -> Option<Result<HmiPopup, Vec<PageFileError>>> {
        self.file(POPUPS_FOLDER, name, parse_popup)
    }

    /// The page a command names: a page by its name, or a popup as `popup:<name>`.
    pub(in crate::web) fn page_or_popup(
        &self,
        address: &str,
    ) -> Option<Result<HmiPage, Vec<PageFileError>>> {
        self.page_or_popup_with(address, &|_| None)
    }

    /// The page a command or a stream names, a faceplate as `faceplate:<page>/<object id>` too,
    /// bound to its instance; `type_of` gives an instance's type (§8.7).
    pub(in crate::web) fn page_or_popup_with(
        &self,
        address: &str,
        type_of: &dyn Fn(&str) -> Option<String>,
    ) -> Option<Result<HmiPage, Vec<PageFileError>>> {
        if address.starts_with(super::faceplates::FACEPLATE_PREFIX) {
            return self
                .faceplate_at(address, type_of)
                .map(|faceplate| faceplate.map(|faceplate| faceplate.popup.page));
        }
        match address.strip_prefix(POPUP_PREFIX) {
            Some(name) => self.popup(name).map(|popup| popup.map(|popup| popup.page)),
            None => self.page(address),
        }
    }

    /// The pages the navigation lists (§4.6): by order, then by name, without the hidden ones.
    /// A page that does not load is listed by its name, so its errors can be reached.
    pub(super) fn navigation(&self) -> Vec<Value> {
        let mut listed: Vec<(Option<u32>, String, Value)> = Vec::new();
        for name in self.page_names() {
            let (order, entry) = match self.page(&name) {
                Some(Ok(page)) if page.hidden => continue,
                Some(Ok(page)) => {
                    let mut entry = json!({ "name": name, "title": page.title });
                    if let Some(icon) = &page.icon {
                        entry["icon"] = json!(icon);
                    }
                    if let Some(group) = &page.group {
                        entry["group"] = json!(group);
                    }
                    (page.order, entry)
                }
                _ => (None, json!({ "name": name, "title": name })),
            };
            listed.push((order, name, entry));
        }
        listed.sort_by(|a, b| (a.0.is_none(), a.0, &a.1).cmp(&(b.0.is_none(), b.0, &b.1)));
        listed.into_iter().map(|(_, _, entry)| entry).collect()
    }

    fn file<T>(
        &self,
        folder: &str,
        name: &str,
        parse: fn(&str, &str) -> Result<T, Vec<PageFileError>>,
    ) -> Option<Result<T, Vec<PageFileError>>> {
        if !is_page_name(name) {
            return None;
        }
        let file = format!("{folder}/{name}.toml");
        let path = self.root.join(&file);
        if !path.is_file() {
            return None;
        }
        Some(read(&path, &file).and_then(|text| parse(&file, &text)))
    }
}

/// The distinct paths a page's objects and pipes bind, in order of first use.
pub(super) fn bound_paths(page: &HmiPage) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    let objects = page.objects.iter().flat_map(|object| object.bind.values());
    for binding in objects.chain(page.pipes.iter().flat_map(|pipe| pipe.bind.values())) {
        for path in std::iter::once(&binding.path).chain(binding.quality.iter()) {
            if !paths.contains(path) {
                paths.push(path.clone());
            }
        }
    }
    paths
}

pub(super) fn errors_json(errors: &[PageFileError]) -> Value {
    Value::Array(
        errors
            .iter()
            .map(
                |error| json!({ "file": error.file, "line": error.line, "message": error.message }),
            )
            .collect(),
    )
}

fn read(path: &std::path::Path, file: &str) -> Result<String, Vec<PageFileError>> {
    std::fs::read_to_string(path).map_err(|error| {
        vec![PageFileError {
            file: file.to_string(),
            line: 1,
            message: format!("the file cannot be read: {error}"),
        }]
    })
}

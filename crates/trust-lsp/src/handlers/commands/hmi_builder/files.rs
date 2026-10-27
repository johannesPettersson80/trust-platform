//! New page and panel files and the references a renamed file keeps (specification 39, §3.3
//! item 8, §4 item 1, §8.7; row H4).
//!
//! - `name { name }` → `{ problem? }`: why `name` cannot name a page or panel file, for the name
//!   box while it is typed: a name has 1 to 64 letters, digits, `-` and `_`.
//! - `create { kind, name, hasScreen }` → `{ file, text, screen? }`: a new page or panel file,
//!   titled with its name in words; a panel is 240 × 720, without fields and docked nowhere.
//!   `screen` is the screen layout a project without one gets (§4 item 1).
//! - `renames { renames: [{ from, to }] }` → `{ projects: [{ path, index, renames }], pattern }`:
//!   which renamed files (paths `<project>/hmi/<folder>/<name>.toml`) are pages, panels or popups
//!   staying in their folder, by project (`index` is the place of the project's first rename in
//!   the request), and `pattern`, the project's files that may link to them.
//! - `references { renames: [{ folder, before, after }], files: { path: text } }` →
//!   `{ write: [{ file, text }] }`: the files whose links to the renamed files change, with their
//!   canonical text. A rename changes only the links to its own kind of file: a page's in
//!   `start_page`, a docked panel's `pages` and `[object.click] page`; a panel's in an object's
//!   `faceplate`, `[object.click] faceplate` and a definition's `faceplate`; a popup's in
//!   `[object.click] popup`. A file that does not load is left as it is.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use trust_runtime::hmi::page_files::{
    parse_faceplate, parse_page, parse_pair, parse_popup, parse_screen, write_faceplate,
    write_page, write_pair, write_popup, write_screen, HmiClick, HmiFaceplate, HmiObject, HmiPage,
    HmiPair, HmiPopup, HmiScreen, PageFileError,
};

use super::decode;

/// What a name that cannot name a file is told.
const NAME_RULE: &str = "Use 1–64 letters, digits, - or _.";

/// The files of a project that may link to a page, panel or popup, relative to the project, as a
/// VS Code glob: one level of braces, since VS Code's globs find nothing for nested ones.
const LINKING_FILES: &str =
    "hmi/{screen.toml,pages/*.toml,panels/*.toml,popups/*.toml,pairs/*.toml}";

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Args {
    Name {
        name: String,
    },
    Create {
        kind: Kind,
        name: String,
        #[serde(rename = "hasScreen")]
        has_screen: bool,
    },
    Renames {
        renames: Vec<Moved>,
    },
    References {
        renames: Vec<Rename>,
        files: BTreeMap<String, String>,
    },
}

/// The kind of file New HMI Page and New HMI Panel create.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Page,
    Panel,
}

/// A file the Explorer renames, by its paths before and after.
#[derive(Deserialize)]
struct Moved {
    from: String,
    to: String,
}

/// The folder of a renamed file, which says what links to it.
#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Folder {
    Pages,
    Panels,
    Popups,
}

/// A renamed page, panel or popup: its folder and its names before and after.
#[derive(Deserialize, Serialize)]
struct Rename {
    folder: Folder,
    before: String,
    after: String,
}

/// The answer of `trust-lsp.hmiFiles`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    match decode::<Args>(argument)? {
        Args::Name { name } => Ok(if is_file_name(&name) {
            json!({ "ok": true })
        } else {
            json!({ "ok": true, "problem": NAME_RULE })
        }),
        Args::Create {
            kind,
            name,
            has_screen,
        } => create(kind, &name, has_screen),
        Args::Renames { renames } => Ok(renamed_files(&renames)),
        Args::References { renames, files } => references(&renames, &files),
    }
}

/// Whether `name` can name a page, panel or popup file: 1 to 64 letters, digits, `-` and `_`, as
/// the runtime addresses pages (§3.3 item 8, §15.1).
pub(super) fn is_file_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// A name in words (§4 item 8): each run of `-` and `_` is one space, without spaces around, and
/// the first letter is a capital, so `room_heating-2` is "Room heating 2".
pub(super) fn in_words(name: &str) -> String {
    let mut spaced = String::with_capacity(name.len());
    let mut separating = false;
    for character in name.chars() {
        let separator = character == '-' || character == '_';
        if !separator {
            spaced.push(character);
        } else if !separating {
            spaced.push(' ');
        }
        separating = separator;
    }
    let spaced = spaced.trim();
    let mut characters = spaced.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

/// The canonical text of a new file's JSON model, written and read back by the editor's format
/// (`trust-lsp.hmiFileFormat`, §15.2).
pub(super) fn written(kind: &str, model: Value) -> Result<String, String> {
    super::super::hmi_authoring::documents::write(kind, model).map_err(|errors| {
        format!(
            "The new {kind} could not be written: {}",
            first_problem(&errors)
        )
    })
}

fn first_problem(errors: &[PageFileError]) -> String {
    errors.first().map_or_else(
        || "no reason given".to_string(),
        |error| error.message.clone(),
    )
}

fn create(kind: Kind, name: &str, has_screen: bool) -> Result<Value, String> {
    if !is_file_name(name) {
        return Err("Invalid HMI file name.".into());
    }
    let title = in_words(name);
    let (file, text) = match kind {
        Kind::Page => (
            format!("hmi/pages/{name}.toml"),
            written("page", json!({ "title": title, "object": [], "pipe": [] }))?,
        ),
        Kind::Panel => (
            format!("hmi/panels/{name}.toml"),
            written(
                "panel",
                json!({
                    "title": title,
                    "object": [],
                    "pipe": [],
                    "size": [240, 720],
                    "fields": {},
                    "panel": { "scope": "manual", "pages": [], "dock": "left", "order": 0 },
                }),
            )?,
        ),
    };
    let mut answer = json!({ "ok": true, "file": file, "text": text });
    if !has_screen {
        answer["screen"] = json!(super::screens::default_screen());
    }
    Ok(answer)
}

/// The project, folder and name of a page, panel or popup file from its path,
/// `<project>/hmi/<folder>/<name>.toml`.
fn hmi_file(path: &str) -> Option<(&str, Folder, &str)> {
    let (rest, name) = path.strip_suffix(".toml")?.rsplit_once('/')?;
    let (rest, folder) = rest.rsplit_once('/')?;
    let folder = match folder {
        "pages" => Folder::Pages,
        "panels" => Folder::Panels,
        "popups" => Folder::Popups,
        _ => return None,
    };
    let (project, hmi) = rest.rsplit_once('/')?;
    (hmi == "hmi" && !name.is_empty()).then_some((project, folder, name))
}

fn renamed_files(moved: &[Moved]) -> Value {
    let mut projects: Vec<(&str, usize, Vec<Rename>)> = Vec::new();
    for (index, Moved { from, to }) in moved.iter().enumerate() {
        let (Some((project, folder, before)), Some((target, folder_after, after))) =
            (hmi_file(from), hmi_file(to))
        else {
            continue;
        };
        if project != target || folder != folder_after {
            continue;
        }
        let rename = Rename {
            folder,
            before: before.to_string(),
            after: after.to_string(),
        };
        match projects.iter_mut().find(|(path, ..)| *path == project) {
            Some((_, _, renames)) => renames.push(rename),
            None => projects.push((project, index, vec![rename])),
        }
    }
    let projects: Vec<Value> = projects
        .into_iter()
        .map(|(path, index, renames)| json!({ "path": path, "index": index, "renames": renames }))
        .collect();
    json!({ "ok": true, "projects": projects, "pattern": LINKING_FILES })
}

/// A file that may link to a page, panel or popup, loaded by the runtime's one loader.
enum Linking {
    Screen(HmiScreen),
    Page(HmiPage),
    Popup(HmiPopup),
    Panel(HmiFaceplate),
    Pair(HmiPair),
}

impl Linking {
    /// The file at `path` (`…/hmi/screen.toml` or `…/hmi/<folder>/<name>.toml`), or `None` for a
    /// file that holds no links or does not load.
    fn load(path: &str, text: &str) -> Option<Self> {
        let mut parts = path.rsplit('/');
        let file = parts.next()?;
        let folder = parts.next()?;
        if folder == "hmi" && file == "screen.toml" {
            return parse_screen(path, text).ok().map(Self::Screen);
        }
        let named = file
            .strip_suffix(".toml")
            .is_some_and(|name| !name.is_empty());
        if !named || parts.next()? != "hmi" {
            return None;
        }
        match folder {
            "pages" => parse_page(path, text).ok().map(Self::Page),
            "popups" => parse_popup(path, text).ok().map(Self::Popup),
            "panels" => parse_faceplate(path, text).ok().map(Self::Panel),
            "pairs" => parse_pair(path, text).ok().map(Self::Pair),
            _ => None,
        }
    }

    /// Points the file's links to `rename.before` at `rename.after`; whether any changed.
    fn relink(&mut self, rename: &Rename) -> bool {
        match self {
            Self::Screen(screen) => {
                rename.folder == Folder::Pages
                    && screen
                        .start_page
                        .as_mut()
                        .is_some_and(|page| relinked(page, rename))
            }
            Self::Page(page) => relink_objects(&mut page.objects, rename),
            Self::Popup(popup) => relink_objects(&mut popup.page.objects, rename),
            Self::Panel(panel) => {
                let docked = rename.folder == Folder::Pages
                    && panel
                        .panel
                        .as_mut()
                        .is_some_and(|dock| relink_all(&mut dock.pages, rename));
                relink_objects(&mut panel.popup.page.objects, rename) || docked
            }
            Self::Pair(pair) => {
                rename.folder == Folder::Panels && relinked(&mut pair.faceplate, rename)
            }
        }
    }

    /// The file's canonical text.
    fn text(&self) -> String {
        match self {
            Self::Screen(screen) => write_screen(screen),
            Self::Page(page) => write_page(page),
            Self::Popup(popup) => write_popup(popup),
            Self::Panel(panel) => write_faceplate(panel),
            Self::Pair(pair) => write_pair(pair),
        }
    }
}

/// Points `name` at `rename.after` when it names `rename.before`; whether it did.
fn relinked(name: &mut String, rename: &Rename) -> bool {
    let linked = *name == rename.before;
    if linked {
        name.clone_from(&rename.after);
    }
    linked
}

/// Points each of `names` that names `rename.before` at `rename.after`; whether any changed.
fn relink_all(names: &mut [String], rename: &Rename) -> bool {
    let mut changed = false;
    for name in names {
        changed |= relinked(name, rename);
    }
    changed
}

/// Points the objects' links to the renamed file at its new name: an equipment object's or a
/// placed panel's `faceplate` for a panel, and the click that opens the file's kind.
fn relink_objects(objects: &mut [HmiObject], rename: &Rename) -> bool {
    let mut changed = false;
    for object in objects {
        changed |= rename.folder == Folder::Panels
            && object
                .faceplate
                .as_mut()
                .is_some_and(|faceplate| relinked(faceplate, rename));
        changed |= match (object.click.as_mut(), rename.folder) {
            (Some(HmiClick::Page(name)), Folder::Pages)
            | (Some(HmiClick::Popup(name)), Folder::Popups)
            | (Some(HmiClick::Faceplate { name, .. }), Folder::Panels) => relinked(name, rename),
            _ => false,
        };
    }
    changed
}

fn references(renames: &[Rename], files: &BTreeMap<String, String>) -> Result<Value, String> {
    let mut write = Vec::new();
    for (path, text) in files {
        let Some(mut file) = Linking::load(path, text) else {
            continue;
        };
        let mut changed = false;
        for rename in renames {
            changed |= file.relink(rename);
        }
        if !changed {
            continue;
        }
        let text = file.text();
        // What is written loads, as every file the editor writes (§21.1 item 1).
        if Linking::load(path, &text).is_none() {
            return Err(format!(
                "{path} would not load with its links renamed, so no link is changed."
            ));
        }
        write.push(json!({ "file": path, "text": text }));
    }
    Ok(json!({ "ok": true, "write": write }))
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;

//! A project's texts for the editor (specification 39, §14.5 items 4, 7 and 8): the languages its
//! screen lists, every text its screen, pages, popups and the panels they show (its own and its
//! libraries') show and where, its text files, and truST's own words in those languages.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;

use super::{builtin, language_name};
use crate::hmi::page_files::{
    parse_faceplate, parse_page, parse_popup, parse_screen, parse_texts, HmiPage, PageFileError,
};

const SCREEN_FILE: &str = "hmi/screen.toml";

/// A language of the project and its own name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LanguageEntry {
    /// The BCP 47 tag, such as `sv`.
    pub tag: String,
    /// The language's own name, such as `Svenska`.
    pub name: String,
}

/// A text the project shows, as written, and where: `screen`, a page's name, or `popup:<name>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectText {
    /// The text as written in the project's first language.
    pub text: String,
    /// Where it shows: `screen`, a page's name, or `popup:<name>`.
    pub places: Vec<String>,
}

/// What the editor is given of a project's texts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ProjectTexts {
    /// The screen's languages, the first being that of the project's texts.
    pub languages: Vec<LanguageEntry>,
    /// Every text the screen, pages and popups show, once, in the order the files give them.
    pub texts: Vec<ProjectText>,
    /// truST's own words, by their English wording.
    pub words: Vec<String>,
    /// The project's text files by language.
    pub project: BTreeMap<String, BTreeMap<String, String>>,
    /// truST's words in each project language truST ships.
    pub builtin: BTreeMap<String, BTreeMap<String, String>>,
    /// The files that do not load; their texts are left out.
    pub errors: Vec<PageFileError>,
}

/// Reads the texts of the project at `root`; `open` holds the text of files the editor has
/// unsaved, by project-relative path.
pub fn read_project_texts(root: &Path, open: &BTreeMap<String, String>) -> ProjectTexts {
    let read = |file: &str| {
        open.get(file)
            .cloned()
            .or_else(|| std::fs::read_to_string(root.join(file)).ok())
    };
    let mut errors = Vec::new();
    let screen = match read(SCREEN_FILE).map(|text| parse_screen(SCREEN_FILE, &text)) {
        Some(Ok(screen)) => Some(screen),
        Some(Err(problems)) => {
            errors.extend(problems);
            None
        }
        None => None,
    };
    let mut found = Found::default();
    if let Some(top_bar) = screen.as_ref().and_then(|screen| screen.top_bar.as_ref()) {
        found.add(&top_bar.title, "screen");
    }
    for (folder, prefix) in [("hmi/pages", ""), ("hmi/popups", "popup:")] {
        for name in file_names(&root.join(folder)) {
            let file = format!("{folder}/{name}.toml");
            let Some(text) = read(&file) else { continue };
            let loaded = if prefix.is_empty() {
                parse_page(&file, &text)
            } else {
                parse_popup(&file, &text).map(|popup| popup.page)
            };
            match loaded {
                Ok(page) => found.page(&page, &format!("{prefix}{name}")),
                Err(problems) => errors.extend(problems),
            }
        }
    }
    // The panels the pages show, placed on them or opened from their equipment, and the
    // project's own: their labels and buttons are texts as a page's are (§14.5 item 1). A
    // library's panel is read as the bundle holds it.
    let mut panels: BTreeSet<String> = found.panels.clone();
    panels.extend(file_names(&root.join("hmi/panels")));
    let bundle = crate::hmi::bundle::HmiBundle::read(root).ok();
    let mut done = BTreeSet::new();
    while let Some(name) = panels.iter().find(|name| !done.contains(*name)).cloned() {
        done.insert(name.clone());
        let file = format!("hmi/panels/{name}.toml");
        let text = read(&file).or_else(|| {
            bundle
                .as_ref()
                .and_then(|bundle| bundle.files.get(&file))
                .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        });
        let Some(text) = text else { continue };
        match parse_faceplate(&file, &text) {
            Ok(panel) => {
                let place = format!("panel:{name}");
                found.page(&panel.popup.page, &place);
                if let Some(description) = &panel.description {
                    found.add(description, &place);
                }
                panels.extend(found.panels.iter().cloned());
            }
            Err(problems) => errors.extend(problems),
        }
    }
    let languages = screen
        .map(|screen| screen.languages)
        .unwrap_or_else(|| vec!["en".to_string()]);
    let mut project = BTreeMap::new();
    for language in &languages {
        let file = format!("hmi/texts/{language}.toml");
        match read(&file).map(|text| parse_texts(&file, &text)) {
            Some(Ok(texts)) => {
                project.insert(language.clone(), texts.texts);
            }
            Some(Err(problems)) => errors.extend(problems),
            None => {}
        }
    }
    ProjectTexts {
        languages: languages
            .iter()
            .map(|tag| LanguageEntry {
                tag: tag.clone(),
                name: language_name(tag).to_string(),
            })
            .collect(),
        texts: found.texts,
        words: builtin("en")
            .map(|english| english.texts.keys().cloned().collect())
            .unwrap_or_default(),
        project,
        builtin: languages
            .iter()
            .filter_map(|tag| builtin(tag).map(|table| (tag.clone(), table.texts.clone())))
            .collect(),
        errors,
    }
}

/// The names of a folder's `.toml` files without the extension, sorted.
fn file_names(folder: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            name.strip_suffix(".toml").map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

/// The texts found so far, each once, with the places it shows in.
#[derive(Default)]
struct Found {
    texts: Vec<ProjectText>,
    index: BTreeMap<String, usize>,
    /// The panels the objects found so far show, by name.
    panels: BTreeSet<String>,
}

impl Found {
    fn add(&mut self, text: &str, place: &str) {
        if text.trim().is_empty() {
            return;
        }
        match self.index.get(text) {
            Some(&at) => {
                let places = &mut self.texts[at].places;
                if !places.iter().any(|known| known == place) {
                    places.push(place.to_string());
                }
            }
            None => {
                self.index.insert(text.to_string(), self.texts.len());
                self.texts.push(ProjectText {
                    text: text.to_string(),
                    places: vec![place.to_string()],
                });
            }
        }
    }

    /// A page's texts: its title and group, then each object's label, text, question and
    /// choices, in z-order. A tag is a name and is not a text (§14.5 item 1).
    fn page(&mut self, page: &HmiPage, place: &str) {
        self.add(&page.title, place);
        if let Some(group) = &page.group {
            self.add(group, place);
        }
        for object in &page.objects {
            for text in [&object.label, &object.text, &object.confirm]
                .into_iter()
                .flatten()
            {
                self.add(text, place);
            }
            for option in object.options.iter().flatten() {
                self.add(option, place);
            }
            if let Some(panel) = &object.faceplate {
                self.panels.insert(panel.clone());
            }
        }
    }
}

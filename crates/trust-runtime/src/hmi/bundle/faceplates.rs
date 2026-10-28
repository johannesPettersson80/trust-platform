//! The faceplates and pairs of a bundle (specification 39, §8.7 item 3): the project's own and
//! its libraries', as the bundle holds them.

use super::HmiBundle;
use crate::hmi::page_files::{
    parse_faceplate, parse_pair, HmiDockScope, HmiFaceplate, HmiPair, PageFileError,
};

impl HmiBundle {
    /// A faceplate by its name, or `None` when the bundle has none of that name.
    pub fn faceplate(&self, name: &str) -> Option<Result<HmiFaceplate, Vec<PageFileError>>> {
        let file = format!("hmi/panels/{name}.toml");
        let bytes = self.files.get(&file)?;
        Some(parse_faceplate(&file, &String::from_utf8_lossy(bytes)))
    }

    /// The pairs of a faceplate, in file order; a pair file that does not load is left out, the
    /// language server reports it.
    pub fn pairs_of(&self, faceplate: &str) -> Vec<HmiPair> {
        self.files
            .iter()
            .filter(|(path, _)| path.starts_with("hmi/pairs/") && path.ends_with(".toml"))
            .filter_map(|(path, bytes)| parse_pair(path, &String::from_utf8_lossy(bytes)).ok())
            .filter(|pair| pair.faceplate == faceplate)
            .collect()
    }

    /// The project's panels docked on pages (specification 39, §21.1 item 3), by name; a panel file
    /// that does not load is left out, its own check reports it.
    pub fn docked_panels(&self) -> Vec<(String, HmiFaceplate)> {
        self.files
            .iter()
            .filter_map(|(path, bytes)| {
                let name = path.strip_prefix("hmi/panels/")?.strip_suffix(".toml")?;
                let panel = parse_faceplate(path, &String::from_utf8_lossy(bytes)).ok()?;
                let docked = panel
                    .panel
                    .as_ref()
                    .is_some_and(|dock| dock.scope != HmiDockScope::Manual);
                docked.then(|| (name.to_string(), panel))
            })
            .collect()
    }
}

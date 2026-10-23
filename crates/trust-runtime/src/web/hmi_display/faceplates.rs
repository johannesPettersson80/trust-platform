//! Faceplates in the runtime display (specification 39, §8.7): a faceplate is named by the object
//! that opens it, `faceplate:<page>/<object id>` (or `faceplate:popup:<name>/<object id>`), and is
//! served bound to that object's instance: each `$Field` becomes the instance's member, by the
//! faceplate's pair for the instance's type, else its only pair, else the field's own name. A
//! faceplate file is the project's own, else a library's. A faceplate placed on a page (§8.9) is
//! named and bound the same way, by its object, and comes with the page. A popup's title is the
//! equipment's tag with the component's title after it, "P101 — Pump" (§8.4 item 1).

use std::path::PathBuf;

use crate::hmi::faceplate_fit::{bind_fields, choose_pair, own_name, placed_faceplate};
use crate::hmi::page_files::{
    parse_faceplate, parse_pair, HmiClick, HmiFaceplate, HmiObject, HmiPage, HmiPair,
    PageFileError, PLACED_FACEPLATE,
};

use super::files::{is_page_name, HmiFiles};

/// A panel bound to an instance, with the component definition chosen for its type, if any.
pub(in crate::web) type BoundFaceplate = (HmiFaceplate, Option<HmiPair>);

/// A faceplate file as found: `None` when there is none, else it loaded or its errors.
pub(in crate::web) type FoundFaceplate = Option<Result<HmiFaceplate, Vec<PageFileError>>>;

/// How a request, a stream or a command names a faceplate.
pub(in crate::web) const FACEPLATE_PREFIX: &str = "faceplate:";
const FACEPLATES_FOLDER: &str = "hmi/panels";
const PAIRS_FOLDER: &str = "hmi/pairs";

/// Whether `faceplate:<source>/<object id>` names a faceplate: a page or `popup:<name>`, and an
/// object id.
pub(in crate::web) fn is_faceplate_address(address: &str) -> bool {
    let Some((source, object)) = address
        .strip_prefix(FACEPLATE_PREFIX)
        .and_then(|rest| rest.rsplit_once('/'))
    else {
        return false;
    };
    is_page_name(source.strip_prefix("popup:").unwrap_or(source)) && is_page_name(object)
}

impl HmiFiles {
    /// The folders of the project's libraries, whose `hmi/` holds the shipped faceplates and pairs.
    fn library_roots(&self) -> Vec<PathBuf> {
        crate::bundle_builder::dependency_roots(self.root()).unwrap_or_default()
    }

    /// A faceplate file by its name: the project's own, else the first library's that has it.
    pub(in crate::web) fn faceplate_file(
        &self,
        name: &str,
    ) -> Option<Result<HmiFaceplate, Vec<PageFileError>>> {
        if !is_page_name(name) {
            return None;
        }
        let file = format!("{FACEPLATES_FOLDER}/{name}.toml");
        std::iter::once(self.root().to_path_buf())
            .chain(self.library_roots())
            .find_map(|root| crate::hmi::bundle::read_hmi_file(&root, &file))
            .map(|bytes| match String::from_utf8(bytes) {
                Ok(text) => parse_faceplate(&file, &text),
                Err(_) => Err(vec![PageFileError {
                    file: file.clone(),
                    line: 1,
                    message: "cannot be read: the file is not UTF-8 text".to_string(),
                }]),
            })
    }

    /// The pairs of a faceplate: the project's, then the libraries' not of a name the project's
    /// already have. A pair file that does not load is left out; the language server reports it.
    fn pairs_of(&self, faceplate: &str) -> Vec<HmiPair> {
        let mut seen = std::collections::BTreeSet::new();
        let mut pairs = Vec::new();
        for root in std::iter::once(self.root().to_path_buf()).chain(self.library_roots()) {
            let Ok(entries) = std::fs::read_dir(root.join(PAIRS_FOLDER)) else {
                continue;
            };
            let mut files: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
                .collect();
            files.sort();
            for path in files {
                let Some(name) = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_string)
                else {
                    continue;
                };
                if !seen.insert(name.clone()) {
                    continue;
                }
                let Some(text) =
                    crate::hmi::bundle::read_hmi_file(&root, &format!("{PAIRS_FOLDER}/{name}"))
                        .and_then(|bytes| String::from_utf8(bytes).ok())
                else {
                    continue;
                };
                if let Ok(pair) = parse_pair(&format!("{PAIRS_FOLDER}/{name}"), &text)
                    && pair.faceplate == faceplate
                {
                    pairs.push(pair);
                }
            }
        }
        pairs
    }

    /// The faceplate an object opens, bound to its instance (§8.7, items 4 and 5); `None` when the
    /// address names no object that opens a faceplate, or the faceplate does not exist.
    /// `type_of` gives an instance's function block type, which chooses among several pairs.
    pub(in crate::web) fn faceplate_at(
        &self,
        address: &str,
        type_of: &dyn Fn(&str) -> Option<String>,
    ) -> Option<Result<HmiFaceplate, Vec<PageFileError>>> {
        self.faceplate_with_definition(address, type_of)
            .map(|found| found.map(|(faceplate, _)| faceplate))
    }

    /// Like [`Self::faceplate_at`], with the component definition chosen for the instance's type
    /// (§8.5, §21.4): the one whose block is the instance's type, else the panel's only one.
    pub(in crate::web) fn faceplate_with_definition(
        &self,
        address: &str,
        type_of: &dyn Fn(&str) -> Option<String>,
    ) -> Option<Result<BoundFaceplate, Vec<PageFileError>>> {
        if !is_faceplate_address(address) {
            return None;
        }
        let (source, object_id) = address.strip_prefix(FACEPLATE_PREFIX)?.rsplit_once('/')?;
        let page = self.page_or_popup(source)?.ok()?;
        let object = page.objects.iter().find(|object| object.id == object_id)?;
        self.object_faceplate(address, object, type_of)
    }

    /// The faceplate `object` opens or places, bound to its instance as
    /// [`Self::faceplate_with_definition`] binds it; `address` names it in its errors.
    pub(in crate::web) fn object_faceplate(
        &self,
        address: &str,
        object: &HmiObject,
        type_of: &dyn Fn(&str) -> Option<String>,
    ) -> Option<Result<BoundFaceplate, Vec<PageFileError>>> {
        let placed = object.kind == PLACED_FACEPLATE;
        let click = object.effective_click();
        let generic_name = if placed && object.instance.is_none() {
            object.faceplate.as_deref()
        } else {
            match &click {
                Some(HmiClick::Faceplate {
                    name,
                    instance: None,
                }) if crate::hmi::faceplate_fit::common_instance(object).is_none() => {
                    Some(name.as_str())
                }
                _ => None,
            }
        };
        if let Some(name) = generic_name {
            return match self.faceplate_file(name)? {
                Ok(faceplate) if !crate::hmi::faceplate_fit::requires_instance(&faceplate) => {
                    Some(Ok((faceplate, None)))
                }
                Ok(_) => Some(Err(vec![PageFileError {
                    file: address.into(),
                    line: 1,
                    message: "choose an equipment instance for this panel".into(),
                }])),
                Err(errors) => Some(Err(errors)),
            };
        }
        let (name, instance) = match &click {
            _ if placed => (object.faceplate.as_ref()?, object.instance.clone()?),
            Some(HmiClick::Faceplate { name, instance }) => (
                name,
                instance
                    .clone()
                    .or_else(|| crate::hmi::faceplate_fit::common_instance(object))?,
            ),
            _ => return None,
        };
        let faceplate = match self.faceplate_file(name)? {
            Ok(faceplate) => faceplate,
            Err(errors) => return Some(Err(errors)),
        };
        let pairs = self.pairs_of(name);
        let instance_type = type_of(&instance);
        let pair = choose_pair(&pairs, instance_type.as_deref()).cloned();
        if placed {
            return placed_faceplate(faceplate, &pairs, object, instance_type.as_deref())
                .map(|faceplate| Ok((faceplate, pair)));
        }
        let members = pair
            .as_ref()
            .map(|pair| pair.fields.clone())
            .unwrap_or_default();
        let mut faceplate = bind_fields(faceplate, &instance, &members);
        // The title names the equipment: the opening object's tag, else the instance's own name,
        // with the component's title after it, "P101 — Pump" (§8.4 item 1).
        let tag = object
            .tag
            .clone()
            .filter(|tag| !tag.trim().is_empty())
            .unwrap_or_else(|| own_name(&instance).to_string());
        let component = pair
            .as_ref()
            .and_then(|pair| pair.title.clone())
            .unwrap_or_else(|| faceplate.popup.page.title.clone());
        faceplate.popup.page.title = format!("{tag} — {component}");
        // Its symbol shows the equipment as the page does: the object `symbol` takes the opening
        // object's type, variant and tag, so a fan shows a fan on the motor faceplate it shares;
        // another symbol of the same type takes the tag (§8.7 item 7).
        for candidate in faceplate.popup.page.objects.iter_mut() {
            if candidate.id == "symbol" {
                candidate.kind = object.kind.clone();
                candidate.variant = object.variant.clone();
                candidate.tag = object.tag.clone();
            } else if candidate.kind == object.kind && candidate.tag.is_none() {
                candidate.tag = object.tag.clone();
            }
        }
        // Its values read as the page shows them: an object bound to a variable the opening
        // object shows takes that object's unit, and its decimals when it has none (§8.7 item 6).
        let shown: Vec<(&str, &String)> = object
            .bind
            .values()
            .filter_map(|binding| {
                Some((
                    binding.path.as_str(),
                    object.unit.as_ref().or(binding.unit.as_ref())?,
                ))
            })
            .collect();
        let unit_of = |path: &str| {
            shown
                .iter()
                .find(|(shown, _)| *shown == path)
                .map(|(_, unit)| (*unit).clone())
        };
        for candidate in faceplate
            .popup
            .page
            .objects
            .iter_mut()
            .filter(|candidate| candidate.unit.is_none())
        {
            if candidate.kind == "trend" {
                // A trend's pens may show values of different units: each pen bound to a
                // variable the opening object shows takes its unit, the others keep their own.
                for binding in candidate
                    .bind
                    .values_mut()
                    .filter(|binding| binding.unit.is_none())
                {
                    binding.unit = unit_of(&binding.path);
                }
                continue;
            }
            let unit = candidate
                .bind
                .values()
                .find_map(|binding| unit_of(&binding.path));
            if let Some(unit) = unit {
                candidate.unit = Some(unit);
                candidate.decimals = candidate.decimals.or(object.decimals);
            }
        }
        Some(Ok((faceplate, pair)))
    }
}

impl HmiFiles {
    /// The faceplates placed on a page or popup (§8.9), each bound to its instance as its popup
    /// is, by object id; `None` for one whose faceplate the project and its libraries lack.
    pub(in crate::web) fn placed_faceplates(
        &self,
        source: &str,
        page: &HmiPage,
        type_of: &dyn Fn(&str) -> Option<String>,
    ) -> Vec<(String, FoundFaceplate)> {
        page.objects
            .iter()
            .filter(|object| object.kind == PLACED_FACEPLATE)
            .map(|object| {
                let address = format!("{FACEPLATE_PREFIX}{source}/{}", object.id);
                let found = self
                    .object_faceplate(&address, object, type_of)
                    .map(|found| found.map(|(faceplate, _)| faceplate));
                (object.id.clone(), found)
            })
            .collect()
    }
}

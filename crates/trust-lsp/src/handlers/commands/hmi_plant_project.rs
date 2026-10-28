//! The project's equipment design, `plant.toml` (specification 39, §10.4): the choices every view of
//! an instance shares and that are not PLC code yet — its function block, the settings chosen in the
//! inspector, the hardware signals chosen in Devices & Connections, and the instances it depends on.
//! It holds no generated code and no numeric identities; once an instance is declared, its
//! declaration is the truth and Create PLC code keeps it in step with these choices.

mod rename;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use trust_runtime::hmi::member_paths::MemberPath;

pub(crate) use rename::rename_edit;

/// The file's name in the project folder.
pub(crate) const PLANT_FILE: &str = "plant.toml";
/// The format this language server writes and reads.
const FORMAT: u32 = 2;
/// The most instances a design names; generation stays bounded.
const MAX_EQUIPMENT: usize = 4096;
/// The most entries one instance's map holds.
const MAX_ENTRIES: usize = 64;

/// The equipment design of a project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlantDesign {
    /// File format, 2.
    pub format: u32,
    /// The instances, by instance path (`Main.P101`).
    #[serde(default)]
    pub equipment: BTreeMap<String, DesignEntry>,
}

/// One instance's choices.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignEntry {
    /// The function block type, with its namespace.
    pub block: String,
    /// Settings chosen in the inspector, as ST literals (`T#10s`, `4000.0`, `TRUE`, `'m'`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub settings: BTreeMap<String, String>,
    /// Hardware terminals and the PLC signals they are connected to (`RunCommand = "Main.PumpRun"`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub signals: BTreeMap<String, String>,
    /// Dependencies and the instances chosen for them (`Measurement = "Main.LT101"`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, String>,
}

impl Default for PlantDesign {
    fn default() -> Self {
        Self {
            format: FORMAT,
            equipment: BTreeMap::new(),
        }
    }
}

impl PlantDesign {
    /// Reads a design, refusing what Create PLC code could not use rather than guessing.
    pub fn parse(text: &str) -> Result<Self, String> {
        let design: Self =
            toml::from_str(text).map_err(|error| format!("plant.toml does not load: {error}"))?;
        if design.format != FORMAT {
            return Err(format!(
                "plant.toml has format {}; this version reads format {FORMAT}",
                design.format
            ));
        }
        if design.equipment.len() > MAX_EQUIPMENT {
            return Err(format!(
                "plant.toml names more than {MAX_EQUIPMENT} instances"
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for (path, entry) in &design.equipment {
            if MemberPath::parse(path).is_err() || !path.contains('.') {
                return Err(format!(
                    "plant.toml: `{path}` is not an instance path such as Main.P101"
                ));
            }
            if !seen.insert(path.to_ascii_uppercase()) {
                return Err(format!("plant.toml names `{path}` twice"));
            }
            if !is_type_name(&entry.block) {
                return Err(format!(
                    "plant.toml: `{}` of `{path}` is not a function block type name",
                    entry.block
                ));
            }
            for (map, name) in [
                (&entry.settings, "settings"),
                (&entry.signals, "signals"),
                (&entry.dependencies, "dependencies"),
            ] {
                if map.len() > MAX_ENTRIES {
                    return Err(format!(
                        "plant.toml: `{path}` has more than {MAX_ENTRIES} {name}"
                    ));
                }
                for (member, value) in map {
                    let member_ok = trust_runtime::hmi::page_files::is_pair_member(member);
                    let value_ok = if name == "settings" {
                        !value.trim().is_empty() && !value.contains('\n')
                    } else {
                        MemberPath::parse(value).is_ok()
                    };
                    if !member_ok || !value_ok {
                        return Err(format!(
                            "plant.toml: `{path}` has an invalid {name} entry `{member}`"
                        ));
                    }
                }
            }
        }
        Ok(design)
    }

    /// The canonical text: sorted tables, so a repeated write changes nothing.
    pub fn write(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }

    /// The choices for an instance, matched without regard to case.
    pub fn entry(&self, instance: &str) -> Option<&DesignEntry> {
        self.equipment
            .iter()
            .find(|(path, _)| path.eq_ignore_ascii_case(instance))
            .map(|(_, entry)| entry)
    }

    /// The choices for an instance to edit, created with its block when missing.
    pub fn entry_mut(&mut self, instance: &str, block: &str) -> &mut DesignEntry {
        let key = self
            .equipment
            .keys()
            .find(|path| path.eq_ignore_ascii_case(instance))
            .cloned()
            .unwrap_or_else(|| instance.to_string());
        self.equipment.entry(key).or_insert_with(|| DesignEntry {
            block: block.to_string(),
            ..DesignEntry::default()
        })
    }

    /// Moves an instance's choices to a new path, and every reference to it.
    pub fn rename_instance(&mut self, from: &str, to: &str) {
        if let Some(key) = self
            .equipment
            .keys()
            .find(|path| path.eq_ignore_ascii_case(from))
            .cloned()
            && let Some(entry) = self.equipment.remove(&key)
        {
            self.equipment.insert(to.to_string(), entry);
        }
        for entry in self.equipment.values_mut() {
            for value in entry
                .dependencies
                .values_mut()
                .chain(entry.signals.values_mut())
            {
                if value.eq_ignore_ascii_case(from) {
                    *value = to.to_string();
                } else if value
                    .get(..from.len() + 1)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(&format!("{from}.")))
                {
                    *value = format!("{to}{}", &value[from.len()..]);
                }
            }
        }
    }
}

/// The design of a project and its text as the editor or the disk has it; a project without
/// `plant.toml` has an empty design and no text.
pub(crate) fn read_design(
    root: &Path,
    open: &BTreeMap<PathBuf, String>,
) -> Result<(PlantDesign, Option<String>), String> {
    let path = root.join(PLANT_FILE);
    let text = match open.get(&path) {
        Some(text) => Some(text.clone()),
        None => match std::fs::read_to_string(&path) {
            Ok(text) => Some(text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(format!("plant.toml cannot be read: {error}")),
        },
    };
    let design = match text.as_deref() {
        Some(text) => PlantDesign::parse(text)?,
        None => PlantDesign::default(),
    };
    Ok((design, text))
}

/// A possibly qualified identifier: `FB_Pump` or `Plant.FB_Pump`.
pub(crate) fn is_type_name(text: &str) -> bool {
    !text.is_empty()
        && text.split('.').all(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESIGN: &str = r#"format = 2

[equipment."Main.P101"]
block = "Plant.FB_Pump"

[equipment."Main.P101".settings]
StartTimeout = "T#10s"

[equipment."Main.P101".signals]
RunCommand = "Main.PumpRun"

[equipment."Main.T101"]
block = "Plant.FB_Tank"

[equipment."Main.T101".dependencies]
LevelInstrument = "Main.LT101"
"#;

    #[test]
    fn a_design_holds_choices_only_and_writes_back_unchanged() {
        let design = PlantDesign::parse(DESIGN).expect("the design loads");
        let pump = design
            .entry("main.p101")
            .expect("found without regard to case");
        assert_eq!(pump.block, "Plant.FB_Pump");
        assert_eq!(pump.settings["StartTimeout"], "T#10s");
        assert_eq!(pump.signals["RunCommand"], "Main.PumpRun");
        let again = PlantDesign::parse(&design.write()).expect("its own text loads");
        assert_eq!(again, design, "a repeated write changes nothing");
    }

    #[test]
    fn an_older_format_or_an_identity_field_is_refused() {
        assert!(PlantDesign::parse("format = 1\n").is_err());
        let with_id = DESIGN.replace(
            "block = \"Plant.FB_Pump\"",
            "block = \"Plant.FB_Pump\"\nid = \"7\"",
        );
        assert!(
            PlantDesign::parse(&with_id).is_err(),
            "no numeric identities"
        );
    }

    #[test]
    fn renaming_an_instance_moves_its_choices_and_references() {
        let mut design = PlantDesign::parse(DESIGN).expect("the design loads");
        design.rename_instance("Main.LT101", "Main.LT102");
        assert_eq!(
            design.entry("Main.T101").expect("tank").dependencies["LevelInstrument"],
            "Main.LT102"
        );
        design.rename_instance("Main.P101", "Main.P201");
        assert!(design.entry("Main.P101").is_none());
        assert_eq!(
            design.entry("Main.P201").expect("moved").block,
            "Plant.FB_Pump"
        );
    }
}

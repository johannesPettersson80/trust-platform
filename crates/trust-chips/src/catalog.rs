//! The catalog of supported chips: the files of the chips whose firmware crate is in the repository,
//! built into this crate (specification 34, TARGET-03 and Appendix H.6). A chip file without its
//! crate is read by its adapter's tests only, until the chip joins the catalog.

use crate::model::{ChipFile, Component};
use crate::{ChipFileError, Problem};

/// The chip files of the chips whose firmware crate is in the repository, each with its name, in the
/// catalog's order.
const FILES: [(&str, &str); 2] = [
    ("esp32c6", include_str!("../../../chips/esp32c6.toml")),
    ("stm32u5a9", include_str!("../../../chips/stm32u5a9.toml")),
];

/// A set of checked chip files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    chips: Vec<ChipFile>,
}

impl Catalog {
    /// The supported chips: the chip files of the chips whose firmware crate is in the repository, which
    /// this crate embeds.
    ///
    /// # Errors
    ///
    /// An embedded file breaks the format. The crate's tests read every file, so only a build
    /// whose files changed without running them can fail here.
    pub fn embedded() -> Result<Self, ChipFileError> {
        Self::from_files(&FILES)
    }

    /// Reads and checks chip files, each given as its name and its text.
    ///
    /// # Errors
    ///
    /// A file breaks the format ([`ChipFile::parse`]), or two files have one name.
    pub fn from_files(files: &[(&str, &str)]) -> Result<Self, ChipFileError> {
        let mut chips: Vec<ChipFile> = Vec::with_capacity(files.len());
        for &(name, text) in files {
            if chips.iter().any(|chip| chip.identity.name == name) {
                return Err(ChipFileError {
                    chip: name.to_owned(),
                    problem: Problem::Duplicate {
                        list: "chips",
                        entry: name.to_owned(),
                    },
                });
            }
            chips.push(ChipFile::parse(name, text)?);
        }
        Ok(Self { chips })
    }

    /// The chips, in the catalog's order.
    #[must_use]
    pub fn chips(&self) -> &[ChipFile] {
        &self.chips
    }

    /// The chip of this name.
    #[must_use]
    pub fn chip(&self, name: &str) -> Option<&ChipFile> {
        self.chips.iter().find(|chip| chip.identity.name == name)
    }

    /// The chips' names, in the catalog's order.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.chips
            .iter()
            .map(|chip| chip.identity.name.as_str())
            .collect()
    }

    /// The names of the chips whose firmware holds `component`: the chips a tool names when it
    /// refuses a project that needs a component its device lacks (ARCH-14).
    #[must_use]
    pub fn chips_with(&self, component: Component) -> Vec<&str> {
        self.chips
            .iter()
            .filter(|chip| chip.firmware.components.contains(&component))
            .map(|chip| chip.identity.name.as_str())
            .collect()
    }
}

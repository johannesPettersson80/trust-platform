//! The four kinds of visual program and the files of each (specification 17 §3, specification 25
//! §11.4).

/// A kind of visual program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VisualKind {
    /// Ladder Diagram, `*.ladder.json` (specification 16).
    Ladder,
    /// Blockly, `*.blockly.json`.
    Blockly,
    /// Statechart, `*.statechart.json`.
    Statechart,
    /// Sequential Function Chart, `*.sfc.json` or `*.sfc`.
    Sfc,
}

/// Every file-name suffix of a visual program with its kind, in the order a base name is cut.
const SUFFIXES: [(&str, VisualKind); 5] = [
    (".ladder.json", VisualKind::Ladder),
    (".blockly.json", VisualKind::Blockly),
    (".statechart.json", VisualKind::Statechart),
    (".sfc.json", VisualKind::Sfc),
    (".sfc", VisualKind::Sfc),
];

/// The suffix of the runtime entry beside a visual program (specification 17 §3).
pub const RUNTIME_ENTRY_SUFFIX: &str = ".visual.runtime.st";

impl VisualKind {
    /// The four kinds.
    pub const ALL: [Self; 4] = [Self::Ladder, Self::Blockly, Self::Statechart, Self::Sfc];

    /// The kind's name in the commands and messages: `ladder`, `blockly`, `statechart`, `sfc`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Ladder => "ladder",
            Self::Blockly => "blockly",
            Self::Statechart => "statechart",
            Self::Sfc => "sfc",
        }
    }

    /// The kind called `name`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// The kind's name as a reader sees it: Ladder, Blockly, Statechart, SFC.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Ladder => "Ladder",
            Self::Blockly => "Blockly",
            Self::Statechart => "Statechart",
            Self::Sfc => "SFC",
        }
    }

    /// The suffix a new or imported file of this kind gets.
    #[must_use]
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Ladder => ".ladder.json",
            Self::Blockly => ".blockly.json",
            Self::Statechart => ".statechart.json",
            Self::Sfc => ".sfc.json",
        }
    }

    /// The last part of the companion function block's name (`FB_<base>_<suffix>`).
    #[must_use]
    pub fn block_suffix(self) -> &'static str {
        match self {
            Self::Ladder => "LADDER",
            Self::Blockly => "BLOCKLY",
            Self::Statechart => "STATECHART",
            Self::Sfc => "SFC",
        }
    }

    /// The kind of the file `file` (a name or a path) by its suffix, in any case.
    #[must_use]
    pub fn of_file(file: &str) -> Option<Self> {
        let name = file_name(file).to_ascii_lowercase();
        SUFFIXES
            .iter()
            .find(|(suffix, _)| name.ends_with(suffix))
            .map(|(_, kind)| *kind)
    }

    /// Whether `file` (a name or a path) has a suffix of this kind.
    #[must_use]
    pub fn owns(self, file: &str) -> bool {
        let name = file_name(file).to_ascii_lowercase();
        SUFFIXES
            .iter()
            .any(|(suffix, kind)| *kind == self && name.ends_with(suffix))
    }
}

/// The glob of every visual program in a workspace, for the extension's file search.
#[must_use]
pub fn source_glob() -> String {
    let suffixes: Vec<&str> = SUFFIXES
        .iter()
        .map(|(suffix, _)| suffix.trim_start_matches('.'))
        .collect();
    format!("**/*.{{{}}}", suffixes.join(","))
}

/// The last component of a path (either separator).
#[must_use]
pub fn file_name(file: &str) -> &str {
    file.rsplit(['/', '\\']).next().unwrap_or(file)
}

/// The base name of a visual program: its file name with the visual suffixes cut once each, in
/// the order of the suffix list (`simple-start-stop.ladder.json` is `simple-start-stop`).
#[must_use]
pub fn base_name(file: &str) -> String {
    let mut base = file_name(file).to_owned();
    for (suffix, _) in SUFFIXES {
        if base.to_ascii_lowercase().ends_with(suffix) {
            base.truncate(base.len() - suffix.len());
        }
    }
    base
}

/// The file name of the companion beside the program `file`: `<base>.st`.
#[must_use]
pub fn companion_file_name(file: &str) -> String {
    format!("{}.st", base_name(file))
}

/// The file name of the runtime entry beside the program `file`: `<base>.visual.runtime.st`.
#[must_use]
pub fn runtime_entry_file_name(file: &str) -> String {
    format!("{}{RUNTIME_ENTRY_SUFFIX}", base_name(file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kind_comes_from_the_suffix_in_any_case() {
        // Catches a kind detection that misses `.sfc`, which the SFC editor opens and the import
        // keeps, or that is case-sensitive.
        assert_eq!(
            VisualKind::of_file("/a/b/Demo.LADDER.json"),
            Some(VisualKind::Ladder)
        );
        assert_eq!(
            VisualKind::of_file("c:\\x\\flow.sfc"),
            Some(VisualKind::Sfc)
        );
        assert_eq!(
            VisualKind::of_file("chart.statechart.json"),
            Some(VisualKind::Statechart)
        );
        assert_eq!(
            VisualKind::of_file("blocks.blockly.json"),
            Some(VisualKind::Blockly)
        );
        assert_eq!(VisualKind::of_file("package.json"), None);
        assert_eq!(VisualKind::of_file("main.st"), None);
    }

    #[test]
    fn companion_and_runtime_entry_sit_beside_the_program() {
        // Specification 17 §3: `<base>.st` and `<base>.visual.runtime.st`.
        assert_eq!(
            companion_file_name("/tmp/demo/simple-start-stop.ladder.json"),
            "simple-start-stop.st"
        );
        assert_eq!(
            runtime_entry_file_name("snake.sfc.json"),
            "snake.visual.runtime.st"
        );
        assert_eq!(companion_file_name("flow.sfc"), "flow.st");
        assert_eq!(base_name("x.SFC.JSON"), "x");
    }

    #[test]
    fn the_glob_names_every_suffix() {
        assert_eq!(
            source_glob(),
            "**/*.{ladder.json,blockly.json,statechart.json,sfc.json,sfc}"
        );
    }
}

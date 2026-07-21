use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RustFact {
    pub qualified_symbol: String,
    pub item_kind: String,
    pub visibility: String,
    pub signature: String,
    pub body_start: usize,
    pub body_end: usize,
    pub line: usize,
    pub risk_signals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RustScan {
    pub schema_version: u32,
    pub facts: Vec<RustFact>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RustSourceUnit {
    pub path: String,
    pub module: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModuleGraph {
    pub sources: Vec<RustSourceUnit>,
    pub test_only_sources: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CargoTarget {
    pub package: String,
    pub target: String,
    pub target_kind: String,
    pub path: String,
    pub module: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CargoUniverse {
    pub targets: Vec<CargoTarget>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RustScannedSource {
    pub path: String,
    pub module: String,
    pub cargo_bindings: Vec<String>,
    pub facts: Vec<RustFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RustUniverseScan {
    pub schema_version: u32,
    pub cargo_targets: Vec<CargoTarget>,
    pub sources: Vec<RustScannedSource>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolingRustScan {
    pub schema_version: u32,
    pub cargo_targets: Vec<CargoTarget>,
    pub sources: Vec<RustScannedSource>,
    pub diagnostics: Vec<String>,
}

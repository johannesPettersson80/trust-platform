#[derive(Debug, Clone, Serialize)]
pub struct PlcopenProfile {
    pub namespace: &'static str,
    pub profile: &'static str,
    pub version: &'static str,
    pub strict_subset: Vec<&'static str>,
    pub unsupported_nodes: Vec<&'static str>,
    pub compatibility_matrix: Vec<PlcopenCompatibilityMatrixEntry>,
    pub source_mapping: &'static str,
    pub vendor_extension_hook: &'static str,
    pub round_trip_limits: Vec<&'static str>,
    pub known_gaps: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlcopenCompatibilityMatrixEntry {
    pub capability: &'static str,
    pub status: &'static str,
    pub notes: &'static str,
}

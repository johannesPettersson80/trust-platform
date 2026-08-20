#[derive(Debug, Clone, Serialize)]
pub struct PlcopenLibraryShimApplication {
    pub vendor: String,
    pub source_symbol: String,
    pub replacement_symbol: String,
    pub occurrences: usize,
    pub notes: String,
}

//! Lifecycle marker admission records. Markers are a sorted vector now; no producer
//! records rows, and the exported snapshot keeps the record shape for its readers.
use super::add_count;

/// Maximum caller/unit aggregates for lifecycle marker admission.
pub const LIFECYCLE_RECORD_CAPACITY: usize = 48;

/// The unit whose admission geometry is recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleDemandUnit {
    /// Sorted directory keyed by declaration-page index.
    DeclarationDirectory,
    /// Sorted directory keyed by owner-page index.
    OwnerDirectory,
    /// Fixed 64 by 64 owner-mask payload page.
    MaskPage,
}
impl LifecycleDemandUnit {
    /// Stable label for post-run diagnostic export.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DeclarationDirectory => "declaration_directory",
            Self::OwnerDirectory => "owner_directory",
            Self::MaskPage => "mask_page",
        }
    }
}

/// Sum and extrema of one observed demand component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemandRange {
    /// Sum across all demands in this category.
    pub sum: u64,
    /// Smallest observed value.
    pub min: usize,
    /// Largest observed value.
    pub max: usize,
}
impl DemandRange {
    pub(super) const EMPTY: Self = Self {
        sum: 0,
        min: usize::MAX,
        max: 0,
    };
    pub(super) fn add(&mut self, value: usize) -> bool {
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        add_count(&mut self.sum, value as u64)
    }
}

/// One caller and lifecycle-page unit before charge admission.
///
/// Directory rows describe ordered-entry geometry. Mask-page rows describe the
/// 512-byte payload initialization and marker/count/row-summary mutations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifecycleDemandRecord {
    /// Source file of the lifecycle demand caller.
    pub file: &'static str,
    /// One-based caller line.
    pub line: u32,
    /// One-based caller column.
    pub column: u32,
    /// Recorded unit; irrelevant ranges remain empty (`sum` and `max` zero).
    pub unit: LifecycleDemandUnit,
    /// Whether this operation inserts a directory entry.
    pub new_entry: bool,
    /// Whether this operation allocates a previously absent mask page.
    pub new_page: bool,
    /// Whether this operation adds a previously absent marker bit.
    pub new_mark: bool,
    /// Whether this operation sets a previously empty row summary bit.
    pub new_row: bool,
    /// Number of observed demands in this category.
    pub calls: u64,
    /// Existing directory entries before insertion.
    pub entries_len: DemandRange,
    /// Existing directory capacity before insertion.
    pub entries_capacity: DemandRange,
    /// Matching entry index or insertion index.
    pub insertion_index: DemandRange,
    /// Existing directory entries shifted by insertion.
    pub suffix_shifts: DemandRange,
    /// Existing directory entries relocated by capacity growth.
    pub relocation: DemandRange,
    /// Actual comparator calls in the demand search.
    pub demand_comparisons: DemandRange,
    /// Conservative comparator bound for admission searches.
    pub search_work: DemandRange,
    /// Work to validate the saved insertion position before preparation.
    pub position_validation_work: DemandRange,
    /// Additional directory lookup performed by the lifecycle page wrapper.
    pub wrapper_search_work: DemandRange,
    /// Fixed-page initialization work.
    pub page_initialization_work: DemandRange,
    /// Bit, marker-count, and row-summary mutation work.
    pub mutation_work: DemandRange,
    /// Work to probe the marker and validate its saved state.
    pub plan_state_work: DemandRange,
    /// Requested allocation bytes.
    pub allocation_bytes: DemandRange,
    /// Total work demand for this unit.
    pub total_work: DemandRange,
    /// Declaration ID extrema and sum.
    pub declaration: DemandRange,
    /// Owner-page index extrema and sum.
    pub owner_page: DemandRange,
}
impl LifecycleDemandRecord {
    pub(super) const EMPTY: Self = Self {
        file: "",
        line: 0,
        column: 0,
        unit: LifecycleDemandUnit::MaskPage,
        new_entry: false,
        new_page: false,
        new_mark: false,
        new_row: false,
        calls: 0,
        entries_len: DemandRange::EMPTY,
        entries_capacity: DemandRange::EMPTY,
        insertion_index: DemandRange::EMPTY,
        suffix_shifts: DemandRange::EMPTY,
        relocation: DemandRange::EMPTY,
        demand_comparisons: DemandRange::EMPTY,
        search_work: DemandRange::EMPTY,
        position_validation_work: DemandRange::EMPTY,
        wrapper_search_work: DemandRange::EMPTY,
        page_initialization_work: DemandRange::EMPTY,
        mutation_work: DemandRange::EMPTY,
        plan_state_work: DemandRange::EMPTY,
        allocation_bytes: DemandRange::EMPTY,
        total_work: DemandRange::EMPTY,
        declaration: DemandRange::EMPTY,
        owner_page: DemandRange::EMPTY,
    };
}

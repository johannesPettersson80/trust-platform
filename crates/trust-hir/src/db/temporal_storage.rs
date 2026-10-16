//! Source-analysis storage width selected by an explicit artifact compatibility context.

/// Short temporal storage width used for layout validation and constant SIZEOF.
/// Logical time range and precision are independent of this compatibility input.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TemporalStorage {
    /// Existing 1.1/2.0 artifact layout.
    Legacy32,
    /// New 3.0 source layout, uniformly on every platform.
    #[default]
    Uniform64,
}

impl TemporalStorage {
    /// Storage width of TIME, DATE, TOD and DT in bytes.
    #[must_use]
    pub const fn bytes(self) -> u64 {
        match self {
            Self::Legacy32 => 4,
            Self::Uniform64 => 8,
        }
    }
}

#[cfg(test)]
mod tests;

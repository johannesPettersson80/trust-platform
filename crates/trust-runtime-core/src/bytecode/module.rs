//! Portable bytecode containers and borrowed host compatibility view.

use super::{
    BytecodeError, BytecodeMetadata, BytecodeVersion, Section, SectionData, SectionId,
    ValidationLimits, ValidationStats, HEADER_FLAG_CRC32,
};
use alloc::vec::Vec;

/// Decoded bytecode module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeModule {
    /// Container format version.
    pub version: BytecodeVersion,
    /// Header integrity/format flags.
    pub flags: u32,
    /// Container sections in serialized order.
    pub sections: Vec<Section>,
}

impl BytecodeModule {
    /// Create an empty raw container with version-appropriate default flags.
    #[must_use]
    pub fn new(version: BytecodeVersion) -> Self {
        let flags = if version.minor >= 1 {
            HEADER_FLAG_CRC32
        } else {
            0
        };
        Self {
            version,
            flags,
            sections: Vec::new(),
        }
    }

    /// Borrow the first matching section; this does not validate the container.
    #[must_use]
    pub fn section(&self, id: SectionId) -> Option<&SectionData> {
        self.view().section(id)
    }

    /// Borrow a section for producer-side edits.
    #[must_use]
    pub fn section_mut(&mut self, id: SectionId) -> Option<&mut SectionData> {
        self.sections
            .iter_mut()
            .find(|section| section.id == id.as_raw())
            .map(|section| &mut section.data)
    }
}

impl BytecodeModule {
    /// Borrow the byte-oriented operations without copying the section data.
    pub fn view(&self) -> BytecodeModuleView<'_> {
        BytecodeModuleView {
            version: self.version,
            flags: self.flags,
            sections: &self.sections,
        }
    }

    /// Serialize the container, preserving section order and CRC policy.
    pub fn encode(&self) -> Result<Vec<u8>, BytecodeError> {
        self.view().encode()
    }
    /// Validate container semantics without preparing executable state.
    pub fn validate(&self) -> Result<(), BytecodeError> {
        self.view().validate()
    }
    /// Extract resource metadata without constructing a compiler or runtime.
    pub fn metadata(&self) -> Result<BytecodeMetadata, BytecodeError> {
        self.view().metadata()
    }
}

/// Borrowed container data; this is not a validated or executable module.
#[derive(Debug, Clone, Copy)]
pub struct BytecodeModuleView<'a> {
    /// Container format version.
    pub version: BytecodeVersion,
    /// Header integrity/format flags.
    pub flags: u32,
    /// Container sections in serialized order.
    pub sections: &'a [Section],
}

impl<'a> BytecodeModuleView<'a> {
    /// Borrow the first matching section; this does not validate the container.
    #[must_use]
    pub fn section(&self, id: SectionId) -> Option<&'a SectionData> {
        self.sections
            .iter()
            .find(|section| section.id == id.as_raw())
            .map(|section| &section.data)
    }
}

/// Immutable, semantically validated bytecode. This is not executable/profile admission.
///
/// Fields are private: raw containers and borrowed views cannot construct this token.
/// Its borrow prevents mutation of the sections until the last token use.
///
/// ```compile_fail,E0502
/// use trust_runtime_core::bytecode::{BytecodeModule, BytecodeVersion};
/// let mut raw = BytecodeModule::new(BytecodeVersion::new(1, 1));
/// let validated = raw.validated().unwrap();
/// raw.sections.clear();
/// let _ = validated.view();
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ValidatedBytecode<'a> {
    view: BytecodeModuleView<'a>,
    stats: ValidationStats,
}

impl<'a> ValidatedBytecode<'a> {
    /// Borrow the checked section records for preparation.
    pub fn section(&self, id: SectionId) -> Option<&'a SectionData> {
        self.view.section(id)
    }
    /// Return the read-only container API without copying section data.
    pub fn view(&self) -> BytecodeModuleView<'a> {
        self.view
    }
    /// Report analysis accounting for this validation, not allocator measurements.
    pub fn stats(&self) -> ValidationStats {
        self.stats
    }
}

impl<'a> BytecodeModuleView<'a> {
    /// Validate and obtain an immutable token with default hosted analysis limits.
    pub fn validated(self) -> Result<ValidatedBytecode<'a>, BytecodeError> {
        self.validated_with_limits(ValidationLimits::default())
    }

    /// Validate and borrow the container using caller-selected analysis limits.
    pub fn validated_with_limits(
        self,
        limits: ValidationLimits,
    ) -> Result<ValidatedBytecode<'a>, BytecodeError> {
        let stats = self.validate_with_limits(limits)?;
        Ok(ValidatedBytecode { view: self, stats })
    }
}

impl BytecodeModule {
    /// Validate and borrow this raw container without preparing executable state.
    pub fn validated(&self) -> Result<ValidatedBytecode<'_>, BytecodeError> {
        self.view().validated()
    }
    /// Apply explicit analysis limits; embedded profiles must select measured bounds.
    pub fn validated_with_limits(
        &self,
        limits: ValidationLimits,
    ) -> Result<ValidatedBytecode<'_>, BytecodeError> {
        self.view().validated_with_limits(limits)
    }
}

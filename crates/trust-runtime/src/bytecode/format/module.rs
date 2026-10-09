use super::*;

/// Decoded bytecode module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeModule {
    /// Container format version.
    pub version: BytecodeVersion,
    /// Container header flags.
    pub flags: u32,
    /// Raw sections in serialized order.
    pub sections: Vec<Section>,
}

impl BytecodeModule {
    #[must_use]
    /// Construct an empty raw container with version-appropriate default flags.
    pub fn new(version: BytecodeVersion) -> Self {
        trust_runtime_core::bytecode::BytecodeModule::new(version).into()
    }

    #[must_use]
    /// Borrow the first section with this identifier without validating it.
    pub fn section(&self, id: SectionId) -> Option<&SectionData> {
        self.view().section(id)
    }

    #[must_use]
    /// Mutably borrow a raw section for producer-side editing.
    pub fn section_mut(&mut self, id: SectionId) -> Option<&mut SectionData> {
        self.sections
            .iter_mut()
            .find(|section| section.id == id.as_raw())
            .map(|section| &mut section.data)
    }
}

impl BytecodeModule {
    /// Borrow core operations without allocating or cloning section data.
    pub fn view(&self) -> trust_runtime_core::bytecode::BytecodeModuleView<'_> {
        trust_runtime_core::bytecode::BytecodeModuleView {
            version: self.version,
            flags: self.flags,
            sections: &self.sections,
        }
    }

    /// Decode a container through the portable decoder.
    pub fn decode(bytes: &[u8]) -> Result<Self, BytecodeError> {
        trust_runtime_core::bytecode::BytecodeModule::decode(bytes).map(Self::from)
    }

    /// Serialize through the portable bounded encoder.
    pub fn encode(&self) -> Result<Vec<u8>, BytecodeError> {
        self.view().encode()
    }
    /// Validate using hosted default analysis limits.
    pub fn validate(&self) -> Result<(), BytecodeError> {
        self.view().validate()
    }
    /// Inspect versioned bytecode and initialization records without executing them.
    pub fn disassemble(&self) -> Result<String, BytecodeError> {
        self.view().disassemble()
    }
    /// Extract resource metadata without preparing executable state.
    pub fn metadata(&self) -> Result<BytecodeMetadata, BytecodeError> {
        self.view().metadata()
    }
}

impl From<trust_runtime_core::bytecode::BytecodeModule> for BytecodeModule {
    fn from(module: trust_runtime_core::bytecode::BytecodeModule) -> Self {
        Self {
            version: module.version,
            flags: module.flags,
            sections: module.sections,
        }
    }
}

impl From<BytecodeModule> for trust_runtime_core::bytecode::BytecodeModule {
    fn from(module: BytecodeModule) -> Self {
        Self {
            version: module.version,
            flags: module.flags,
            sections: module.sections,
        }
    }
}

impl BytecodeModule {
    /// Validate the 2.0 source-free format without claiming hosted execution support.
    pub fn validated_source_free(
        &self,
        limits: ValidationLimits,
    ) -> Result<ValidatedBytecode<'_>, BytecodeError> {
        self.view().validated_source_free(limits)
    }

    /// Obtain a borrowed validation token before hosted VM materialization.
    pub fn validated(&self) -> Result<ValidatedBytecode<'_>, BytecodeError> {
        self.view().validated()
    }
    /// Validate using explicit analysis limits without preparing executable state.
    pub fn validated_with_limits(
        &self,
        limits: ValidationLimits,
    ) -> Result<ValidatedBytecode<'_>, BytecodeError> {
        self.view().validated_with_limits(limits)
    }
}

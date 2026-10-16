//! Private, fully validated section sources used while preparing VM metadata.
//!
//! Borrowed callers keep the existing immutable validation token and clone only
//! at their existing ownership boundaries. The ordinary byte-loading route can
//! instead validate an owned container here and move selected payloads after
//! preparation has finished borrowing them.

use crate::bytecode::{
    AccessBindings, BytecodeError, BytecodeModule, BytecodeModuleView, ConstructionRoots,
    InitializerIndex, ResourceMeta, SectionData, SectionId, StorageLayout, StringTable, TypeTable,
    ValidatedBytecode, ValidationLimits,
};
use alloc::vec::Vec;

mod private {
    pub trait Sealed {}
}

/// A validated section source that cannot be implemented outside this module.
pub(super) trait ValidatedSource: private::Sealed {
    /// Return the current read-only header and section view.
    fn view(&self) -> BytecodeModuleView<'_>;

    /// Borrow a section by its checked wire identity.
    fn section(&self, id: SectionId) -> Option<&SectionData> {
        self.view().section(id)
    }

    /// Take or clone one section at a final ownership boundary.
    fn take_section(&mut self, id: SectionId) -> Option<SectionData>;

    fn take_bodies(&mut self) -> Option<Vec<u8>> {
        match self.take_section(SectionId::PouBodies)? {
            SectionData::PouBodies(value) => Some(value),
            _ => None,
        }
    }

    fn take_strings(&mut self) -> Option<StringTable> {
        match self.take_section(SectionId::StringTable)? {
            SectionData::StringTable(value) => Some(value),
            _ => None,
        }
    }

    fn take_types(&mut self) -> Option<TypeTable> {
        match self.take_section(SectionId::TypeTable)? {
            SectionData::TypeTable(value) => Some(value),
            _ => None,
        }
    }

    fn take_layout(&mut self) -> Option<StorageLayout> {
        match self.take_section(SectionId::StorageLayout)? {
            SectionData::StorageLayout(value) => Some(value),
            _ => None,
        }
    }

    fn take_roots(&mut self) -> Option<ConstructionRoots> {
        match self.take_section(SectionId::ConstructionRoots)? {
            SectionData::ConstructionRoots(value) => Some(value),
            _ => None,
        }
    }

    fn take_initializers(&mut self) -> Option<InitializerIndex> {
        match self.take_section(SectionId::Initializers)? {
            SectionData::Initializers(value) => Some(value),
            _ => None,
        }
    }

    fn take_access(&mut self) -> Option<AccessBindings> {
        match self.take_section(SectionId::AccessBindings)? {
            SectionData::AccessBindings(value) => Some(value),
            _ => None,
        }
    }

    fn take_resources(&mut self) -> Option<ResourceMeta> {
        match self.take_section(SectionId::ResourceMeta)? {
            SectionData::ResourceMeta(value) => Some(value),
            _ => None,
        }
    }
}

impl private::Sealed for ValidatedBytecode<'_> {}

impl ValidatedSource for ValidatedBytecode<'_> {
    fn view(&self) -> BytecodeModuleView<'_> {
        ValidatedBytecode::view(self)
    }

    fn take_section(&mut self, id: SectionId) -> Option<SectionData> {
        self.section(id).cloned()
    }
}

/// An owned raw module whose complete validation succeeded before construction.
#[derive(Debug)]
pub(super) struct OwnedValidatedSource {
    raw: BytecodeModule,
}

impl OwnedValidatedSource {
    /// Validate every container and section invariant before retaining ownership.
    pub(super) fn new(
        raw: BytecodeModule,
        limits: ValidationLimits,
    ) -> Result<Self, BytecodeError> {
        {
            let _validated = raw.validated_with_limits(limits)?;
        }
        Ok(Self { raw })
    }
}

impl private::Sealed for OwnedValidatedSource {}

impl ValidatedSource for OwnedValidatedSource {
    fn view(&self) -> BytecodeModuleView<'_> {
        self.raw.view()
    }

    fn take_section(&mut self, id: SectionId) -> Option<SectionData> {
        let index = self
            .raw
            .sections
            .iter()
            .position(|section| section.id == id.as_raw())?;
        Some(self.raw.sections.swap_remove(index).data)
    }
}

#[cfg(test)]
mod tests {
    use super::{OwnedValidatedSource, ValidatedSource};
    use crate::bytecode::{
        BytecodeModule, SectionData, SectionId, ValidatedBytecode, ValidationLimits,
    };

    fn fixture() -> BytecodeModule {
        BytecodeModule::decode(include_bytes!(
            "../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ))
        .expect("source-free test fixture decodes")
    }

    #[test]
    fn borrowed_source_retains_sections_after_ownership_accessors() {
        let raw = fixture();
        let validated: ValidatedBytecode<'_> = raw.validated().expect("fixture validates");
        let expected_bodies = match validated.section(SectionId::PouBodies) {
            Some(SectionData::PouBodies(value)) => value.clone(),
            other => panic!("unexpected POU_BODIES section: {other:?}"),
        };
        let expected_strings = match validated.section(SectionId::StringTable) {
            Some(SectionData::StringTable(value)) => value.clone(),
            other => panic!("unexpected STRING_TABLE section: {other:?}"),
        };
        let expected_types = match validated.section(SectionId::TypeTable) {
            Some(SectionData::TypeTable(value)) => value.clone(),
            other => panic!("unexpected TYPE_TABLE section: {other:?}"),
        };
        let mut source = validated;

        assert_eq!(source.take_bodies(), Some(expected_bodies.clone()));
        assert_eq!(source.take_strings(), Some(expected_strings.clone()));
        assert_eq!(source.take_types(), Some(expected_types.clone()));
        assert_eq!(source.take_bodies(), Some(expected_bodies));
        assert_eq!(
            source.section(SectionId::StringTable),
            Some(&SectionData::StringTable(expected_strings))
        );
        assert_eq!(
            source.section(SectionId::TypeTable),
            Some(&SectionData::TypeTable(expected_types))
        );
    }

    #[test]
    fn owned_source_moves_selected_payloads_and_keeps_unselected_sections() {
        let raw = fixture();
        let expected_bodies = match raw.section(SectionId::PouBodies) {
            Some(SectionData::PouBodies(value)) => value.clone(),
            other => panic!("unexpected POU_BODIES section: {other:?}"),
        };
        let expected_strings = match raw.section(SectionId::StringTable) {
            Some(SectionData::StringTable(value)) => value.clone(),
            other => panic!("unexpected STRING_TABLE section: {other:?}"),
        };
        let expected_types = match raw.section(SectionId::TypeTable) {
            Some(SectionData::TypeTable(value)) => value.clone(),
            other => panic!("unexpected TYPE_TABLE section: {other:?}"),
        };
        let expected_pou_index = raw.section(SectionId::PouIndex).cloned();
        let original_body_pointer = match raw.section(SectionId::PouBodies) {
            Some(SectionData::PouBodies(value)) => value.as_ptr(),
            _ => unreachable!(),
        };
        let mut source =
            OwnedValidatedSource::new(raw, ValidationLimits::default()).expect("fixture validates");

        let moved_bodies = source.take_bodies().unwrap();
        assert_eq!(moved_bodies.as_ptr(), original_body_pointer);
        assert_eq!(moved_bodies, expected_bodies);
        assert_eq!(source.take_strings(), Some(expected_strings));
        assert_eq!(source.take_types(), Some(expected_types));
        assert_eq!(source.take_bodies(), None);
        assert_eq!(source.section(SectionId::PouBodies), None);
        assert_eq!(
            source.section(SectionId::PouIndex),
            expected_pou_index.as_ref()
        );
    }

    #[test]
    fn owned_source_constructor_rejects_invalid_raw_metadata() {
        let mut raw = fixture();
        raw.flags = 0;

        assert!(OwnedValidatedSource::new(raw, ValidationLimits::default()).is_err());
    }
}

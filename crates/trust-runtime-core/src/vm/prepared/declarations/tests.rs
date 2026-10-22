//! A program prepared from its mapped image reads its storage declarations and initializers where
//! they lie in the image (specification 34 MEM-14, the program image in place).
use alloc::string::String;

use super::super::records::Records;
use super::*;
use crate::bytecode::TypeData;
use crate::stdlib::contracts::ANY_DURATION_STATE;
use crate::vm::PreparationLimits;

/// The portability fixture: a TON, an FB through an interface, a method-local reference initialized
/// on each call, retained state and nonzero defaults (its README).
const ARTIFACT: &[u8] = include_bytes!(
    "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
);

/// The fixture prepared from bytes, which holds its records decoded, and from its mapped image.
fn both() -> (PreparedModule, PreparedModule) {
    (
        PreparedModule::from_bytes(ARTIFACT, PreparationLimits::default()).unwrap(),
        PreparedModule::from_mapped(ARTIFACT, PreparationLimits::default()).unwrap(),
    )
}

/// Every declaration and initializer that preparation admitted, and what a scan reads of each, is
/// the same whether the program holds its records decoded or reads them in its mapped image.
/// Catches: a record read at the wrong offset or width of its section, a record past the end of
/// its table, or a compact record that differs from its record.
#[test]
fn a_mapped_program_reads_every_record_that_preparation_admitted() {
    let (decoded, mapped) = both();
    assert!(matches!(decoded.storage, Records::Decoded(_)));
    assert!(matches!(decoded.initializers, Records::Decoded(_)));
    assert!(matches!(mapped.storage, Records::InImage { .. }));
    assert!(matches!(mapped.initializers, Records::InImage { .. }));
    assert!(decoded.declaration_count() > 0 && decoded.initializer_count() > 0);
    assert_eq!(mapped.declaration_count(), decoded.declaration_count());
    assert_eq!(mapped.initializer_count(), decoded.initializer_count());
    for index in 0..decoded.declaration_count() {
        assert_eq!(
            mapped.storage_declaration(index),
            decoded.storage_declaration(index),
            "declaration {index}"
        );
        assert_eq!(
            mapped.scan_declaration(index),
            decoded.scan_declaration(index),
            "declaration {index}"
        );
    }
    for index in 0..decoded.initializer_count() {
        assert_eq!(
            mapped.initializer(index),
            decoded.initializer(index),
            "initializer {index}"
        );
        assert_eq!(
            mapped.scan_initializer(index),
            decoded.scan_initializer(index),
            "initializer {index}"
        );
    }
    let past = decoded.declaration_count();
    assert_eq!(mapped.storage_declaration(past), None);
    assert_eq!(mapped.scan_declaration(past), None);
    assert_eq!(mapped.initializer(decoded.initializer_count()), None);
}

/// A record that preparation changes is kept as preparation holds it: the PT and ET of the
/// fixture's TON take the native duration slot (specification 34 §6.2), where the image declares
/// them TIME. Preparation changes no initializer.
/// Catches: a mapped program that reads a changed record from the image, as the compiler wrote it.
#[test]
fn a_mapped_program_keeps_the_records_that_preparation_changed() {
    let (_, mapped) = both();
    let Records::InImage { changed, .. } = &mapped.storage else {
        panic!("a mapped program reads its declarations in its image");
    };
    let mut names: Vec<String> = changed
        .iter()
        .map(|(_, declaration)| {
            mapped.vm.strings[declaration.name_idx as usize].to_ascii_uppercase()
        })
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["ET", "PT"]);
    let ton = mapped
        .vm
        .function_block_ids
        .get(&mapped.vm.strings, "TON")
        .expect("the fixture's TON");
    for (index, declaration) in changed {
        assert_eq!(declaration.owner, StorageOwner::Instance);
        assert_eq!(declaration.owner_pou_id, Some(ton));
        let slot = declaration.type_id.expect("a typed member") as usize;
        assert!(matches!(
            mapped.vm.types.entries[slot].data,
            TypeData::Primitive { prim_id, .. } if prim_id == ANY_DURATION_STATE
        ));
        assert_eq!(
            mapped.storage_declaration(*index as usize).as_ref(),
            Some(declaration)
        );
    }
    let Records::InImage { changed, .. } = &mapped.initializers else {
        panic!("a mapped program reads its initializers in its image");
    };
    assert!(changed.is_empty());
}

/// Of its declarations and initializers, a mapped program keeps on the heap only one compact record
/// for each and the two records that preparation changed. A program prepared from bytes keeps each
/// record decoded as its compact record and its other fields, which take no more bytes than the
/// record (specification 34 MEM-14).
/// Catches: a mapped table that keeps its decoded records on the heap as well, and a decoded table
/// that keeps its compact records beside its records.
#[test]
fn a_mapped_program_keeps_on_the_heap_only_what_a_scan_reads() {
    let (decoded, mapped) = both();
    let part = |prepared: &PreparedModule, name: &str| {
        prepared
            .program_parts()
            .iter()
            .find(|part| part.name == name)
            .expect("a named part")
            .bytes
    };
    let declarations = decoded.declaration_count();
    let initializers = decoded.initializer_count();
    assert_eq!(
        part(&mapped, "storage layout"),
        declarations * size_of::<ScanDeclaration>() + 2 * size_of::<(u32, StorageDeclaration)>()
    );
    assert_eq!(
        part(&mapped, "initializers"),
        initializers * size_of::<ScanInitializer>()
    );
    assert_eq!(
        part(&decoded, "storage layout"),
        declarations * size_of::<(ScanDeclaration, DeclarationRest)>()
    );
    assert_eq!(
        part(&decoded, "initializers"),
        initializers * size_of::<(ScanInitializer, InitializerRest)>()
    );
    assert!(size_of::<(ScanDeclaration, DeclarationRest)>() <= size_of::<StorageDeclaration>());
    assert!(size_of::<(ScanInitializer, InitializerRest)>() <= size_of::<InitializerEntry>());
}

/// A decoded record's two parts join to give the record back, for every declaration and initializer
/// of the fixture, so a program prepared from bytes reads the records that preparation admitted.
/// Catches: a field that the split loses or the join puts in the wrong place.
#[test]
fn a_decoded_record_joins_from_its_parts_unchanged() {
    let decoded = PreparedModule::from_bytes(ARTIFACT, PreparationLimits::default()).unwrap();
    let module = crate::bytecode::BytecodeModule::decode(ARTIFACT).unwrap();
    let (
        Some(crate::bytecode::SectionData::StorageLayout(layout)),
        Some(crate::bytecode::SectionData::Initializers(initializers)),
    ) = (
        module.section(SectionId::StorageLayout),
        module.section(SectionId::Initializers),
    )
    else {
        panic!("the fixture holds a storage layout and an initializer index");
    };
    assert_eq!(decoded.initializer_count(), initializers.entries.len());
    for (index, initializer) in initializers.entries.iter().enumerate() {
        let (scan, rest) = initializer.clone().split();
        assert_eq!(InitializerEntry::join(&scan, &rest), *initializer);
        assert_eq!(decoded.initializer(index).as_ref(), Some(initializer));
    }
    assert_eq!(decoded.declaration_count(), layout.entries.len());
    for declaration in &layout.entries {
        let (scan, rest) = declaration.clone().split();
        assert_eq!(StorageDeclaration::join(&scan, &rest), *declaration);
    }
}

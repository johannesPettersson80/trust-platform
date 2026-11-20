//! The service table `SERVICES` (specification 44 §5.14; §10.2, step 5) on the sample image of the
//! image tests: the program `Vat`, an instance of `VatControl`, with its `State`, an `INT`, and its
//! retained `Batches`, a `UDINT`. Every expected value is worked out from the specification.

use alloc::vec;

use super::super::tests::sample_with_services;
use super::*;

/// `recipe` may write `Vat.State`; `dosing` may write `Batches`, spelled in lower case, and `State`.
const SERVICES: &[(&str, &[&str])] = &[
    ("recipe", &["Vat.State"]),
    ("dosing", &["vat.batches", "Vat.State"]),
];

/// The table reads back with each service's name and its paths as the project spells them, in
/// order, before `SIGN`, and an image without it has no service that writes. Catches a path list
/// read from the wrong index, paths respelled, and a table after the signature, which would not
/// cover it.
#[test]
fn the_service_table_reads_back_before_sign() {
    let bytes = sample_with_services(SERVICES);
    let image = Image::parse(&bytes).unwrap();
    image.check_tables().unwrap();
    assert_eq!(
        image.services().unwrap(),
        vec![
            ImageService {
                name: "recipe",
                paths: vec!["Vat.State"],
            },
            ImageService {
                name: "dosing",
                paths: vec!["vat.batches", "Vat.State"],
            },
        ]
    );
    let services = image
        .section(SectionId::Services)
        .expect("the service table");
    let sign = image.section(SectionId::Sign).expect("the signature");
    assert!(services.offset < sign.offset, "SERVICES lies before SIGN");
    assert_eq!(sign.end(), bytes.len() as u64, "SIGN is last");

    let without = sample_with_services(&[]);
    let without = Image::parse(&without).unwrap();
    assert_eq!(without.section(SectionId::Services), None);
    assert_eq!(without.services().unwrap(), Vec::new());
}

/// A path that names the program instance, which holds no value, and one that names no member are
/// refused by every image check, so that no device takes an image whose service would write what
/// no write reaches. Catches a table checked for its layout alone.
#[test]
fn a_path_that_no_write_reaches_is_refused() {
    const INSTANCE: &[(&str, &[&str])] = &[("recipe", &["Vat"])];
    const NOTHING: &[(&str, &[&str])] = &[("recipe", &["Vat.Nothing"])];
    for services in [INSTANCE, NOTHING] {
        let bytes = sample_with_services(services);
        assert_eq!(
            Image::parse(&bytes).and_then(|image| image.check_tables()),
            Err(ImageError::Record {
                section: SectionId::Services,
                index: 0,
                reason: "path names no elementary variable a write reaches",
            }),
            "{services:?}"
        );
    }
}

/// Step 5 of §10.2 refuses a service table the build cannot have written: a name outside the
/// string pool or empty, a reserved word that is not 0, a name that another's repeats without case,
/// a service without a path, a path list outside the section, a path outside the string pool, and
/// a count its section does not hold. Catches a device that holds a service to a table it cannot
/// read.
#[test]
fn a_malformed_service_table_is_refused() {
    let bytes = sample_with_services(SERVICES);
    let image = Image::parse(&bytes).unwrap();
    let records = image.section(SectionId::Services).unwrap().offset as usize + TABLE_PREFIX_BYTES;
    // The path list follows the two records: recipe's path, then dosing's two.
    let paths = records + 2 * ServiceRecord::BYTES;
    let recipe_name = image
        .table::<ServiceRecord>()
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .name;
    let at = |index: usize, field: usize| records + index * ServiceRecord::BYTES + field;
    let refused = |edit: &dyn Fn(&mut Vec<u8>)| {
        let mut edited = bytes.clone();
        edit(&mut edited);
        Image::parse(&edited)
            .and_then(|image| image.check_tables())
            .expect_err("the table is refused")
    };
    let service = |index, reason| ImageError::Record {
        section: SectionId::Services,
        index,
        reason,
    };
    assert_eq!(
        refused(&|image| image[at(0, 0)..at(0, 4)].copy_from_slice(&u32::MAX.to_le_bytes())),
        service(0, "name not a string of STRINGS")
    );
    assert_eq!(
        refused(&|image| image[at(1, 0)..at(1, 4)].fill(0)),
        service(1, "name not a string of STRINGS")
    );
    assert_eq!(
        refused(&|image| image[at(1, 12)] = 1),
        service(1, "reserved word not 0")
    );
    // dosing takes recipe's name.
    assert_eq!(
        refused(&|image| image[at(1, 0)..at(1, 4)].copy_from_slice(&recipe_name.to_le_bytes())),
        service(1, "name repeated")
    );
    assert_eq!(
        refused(&|image| image[at(0, 8)..at(0, 12)].fill(0)),
        service(0, "path list empty or outside SERVICES")
    );
    // dosing's two paths would begin at index 2, so its second lies past the list's end.
    assert_eq!(
        refused(&|image| image[at(1, 4)..at(1, 8)].copy_from_slice(&2u32.to_le_bytes())),
        service(1, "path list empty or outside SERVICES")
    );
    assert_eq!(
        refused(&|image| image[paths..paths + 4].copy_from_slice(&u32::MAX.to_le_bytes())),
        service(0, "path not a string of STRINGS")
    );
    assert_eq!(
        refused(&|image| image[records - TABLE_PREFIX_BYTES] = 200),
        ImageError::Table(SectionId::Services)
    );
}

/// The repeated name is found without case: `Recipe` repeats `recipe`. Catches two services whose
/// names a device could not tell apart, as `trust.toml`'s names compare.
#[test]
fn a_name_repeated_in_another_case_is_refused() {
    let bytes = sample_with_services(&[("recipe", &["Vat.State"]), ("Recipe", &["Vat.Batches"])]);
    assert_eq!(
        Image::parse(&bytes).and_then(|image| image.check_tables()),
        Err(ImageError::Record {
            section: SectionId::Services,
            index: 1,
            reason: "name repeated",
        })
    );
}

use crate::harness::{lower_application, SourceFile};
use crate::RuntimeBuilder;
use trust_hir::Type;

fn source_files(files: &[(&str, &str)]) -> Vec<SourceFile> {
    files
        .iter()
        .map(|(path, source)| SourceFile::with_path(*path, *source))
        .collect()
}

fn runtime(files: &[(&str, &str)]) -> RuntimeBuilder {
    lower_application(&source_files(files), files.len() > 1)
        .and_then(|lowered| lowered.construct(&[]))
        .map(|(builder, _)| builder)
        .unwrap_or_else(|error| panic!("derived-type constant fixture must compile: {error}"))
}

fn resolved_type<'a>(builder: &'a RuntimeBuilder, name: &str) -> &'a Type {
    let mut type_id = builder
        .registry()
        .lookup(name)
        .unwrap_or_else(|| panic!("missing type {name}"));
    loop {
        match builder.registry().get(type_id) {
            Some(Type::Alias { target, .. }) => type_id = *target,
            _ => {
                return builder
                    .registry()
                    .get(type_id)
                    .unwrap_or_else(|| panic!("missing resolved type {name}"))
            }
        }
    }
}

fn enum_values(builder: &RuntimeBuilder, name: &str) -> Vec<(String, i64)> {
    match resolved_type(builder, name) {
        Type::Enum { values, .. } => values
            .iter()
            .map(|(name, value)| (name.to_string(), *value))
            .collect(),
        other => panic!("expected enumeration type {name}, got {other:?}"),
    }
}

#[test]
fn derived_type_constant_enum_member_arithmetic_folds_beyond_narrow_literals() {
    // IEC_DEVIATIONS 2026-10-30: untyped literals materialize narrowest
    // fitting, so `16 * 8` folds as SInt * SInt; the compile-time exact
    // fold keeps a representable member magnitude (the vendor syslog
    // facility idiom) instead of faulting on the i8 narrowing.
    let builder = runtime(&[(
        "main.st",
        r#"
TYPE Facility : (
    Local0 := 16 * 8,
    Local1,
    Derived := 4 * 8 - 24
);
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(
        enum_values(&builder, "Facility"),
        [
            ("Local0".into(), 128),
            ("Local1".into(), 129),
            ("Derived".into(), 8)
        ]
    );
}

//! Immutable descriptors shared by every default registry.

use super::StdFunctionRef;

// Each family owns its single authoritative registration list. Parameter slices
// live in flash; explicit hosted registration clones only borrowed metadata.
macro_rules! descriptor {
    ($name:literal, $params:ident, $func:path) => {
        (
            $name,
            super::StdFunctionRef {
                params: &super::parameters::$params,
                func: $func,
            },
        )
    };
}
pub(super) use descriptor;

static FAMILIES: &[&[(&str, StdFunctionRef<'static>)]] = &[
    super::assertions::FUNCTIONS,
    super::numeric::FUNCTIONS,
    super::bit::FUNCTIONS,
    super::selection::FUNCTIONS,
    super::comparison::FUNCTIONS,
    super::string::FUNCTIONS,
    super::time::FUNCTIONS,
    super::validate::FUNCTIONS,
];

pub(super) fn get(name: &str) -> Option<&'static StdFunctionRef<'static>> {
    // Eight fixed families, each searched logarithmically, without allocating an
    // uppercase key. Conversion recognition remains algorithmic and separate.
    for family in FAMILIES {
        if let Ok(index) = family.binary_search_by(|(key, _)| {
            key.bytes()
                .cmp(name.bytes().map(|byte| byte.to_ascii_uppercase()))
        }) {
            return Some(&family[index].1);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::{StandardLibrary, StdParams};
    use super::*;
    use crate::{error::RuntimeError, value::Value};
    use alloc::{format, string::String, vec};

    fn custom(_: &[Value]) -> Result<Value, RuntimeError> {
        Ok(Value::Int(73))
    }

    // The full stdlib contract suites remain canonical for individual functions.
    // These successful calls pin registration/binding across every family without
    // treating machine-code address equality (which ICF may change) as behavior.
    fn assert_behavior_corpus(library: &StandardLibrary) {
        use crate::value::Duration;
        for (name, args, expected) in [
            ("ABS", vec![Value::Int(-7)], Value::Int(7)),
            ("SQRT", vec![Value::LReal(4.0)], Value::LReal(2.0)),
            ("ADD", vec![Value::Int(1), Value::Int(2)], Value::Int(3)),
            ("SHL", vec![Value::Byte(1), Value::Int(2)], Value::Byte(4)),
            (
                "SEL",
                vec![Value::Bool(false), Value::Int(11), Value::Int(22)],
                Value::Int(11),
            ),
            (
                "MUX",
                vec![Value::Int(0), Value::Int(11), Value::Int(22)],
                Value::Int(11),
            ),
            ("GT", vec![Value::Int(2), Value::Int(1)], Value::Bool(true)),
            ("LEN", vec![Value::String("abc".into())], Value::Int(3)),
            (
                "CONCAT",
                vec![Value::String("a".into()), Value::String("b".into())],
                Value::String("ab".into()),
            ),
            (
                "ADD_TIME",
                vec![
                    Value::Time(Duration::from_millis(1)),
                    Value::Time(Duration::from_millis(2)),
                ],
                Value::Time(Duration::from_millis(3)),
            ),
            ("IS_VALID", vec![Value::Real(1.0)], Value::Bool(true)),
            ("IS_VALID_BCD", vec![Value::Byte(0x12)], Value::Bool(true)),
            ("ASSERT_TRUE", vec![Value::Bool(true)], Value::Null),
            (
                "ASSERT_GREATER",
                vec![Value::Int(2), Value::Int(1)],
                Value::Null,
            ),
        ] {
            let mixed = name.to_ascii_lowercase();
            let found = library.get(&mixed).unwrap();
            assert_eq!((found.func)(&args), Ok(expected.clone()), "direct {name}");
            assert_eq!(library.call(&mixed, &args), Ok(expected), "registry {name}");
        }
    }

    #[test]
    fn shared_descriptors_preserve_the_pre_static_registration_contract() {
        let library = StandardLibrary::new();
        let baseline = include_str!("registration-baseline.txt");
        assert_eq!(
            FAMILIES.iter().map(|family| family.len()).sum::<usize>(),
            baseline.lines().count()
        );
        for family in FAMILIES {
            assert!(family.windows(2).all(|pair| pair[0].0 < pair[1].0));
            for (name, entry) in *family {
                assert_eq!(
                    FAMILIES
                        .iter()
                        .flat_map(|family| family.iter())
                        .filter(|(other, _)| name == other)
                        .count(),
                    1
                );
                let found = library.get(&name.to_ascii_lowercase()).unwrap();
                assert!(core::ptr::eq(found.params, entry.params));
                assert_eq!((found.func)(&[]), (entry.func)(&[]));
                let shape = match entry.params {
                    StdParams::Fixed(params) => format!(
                        "fixed({})",
                        params
                            .iter()
                            .map(|p| p.as_str())
                            .collect::<alloc::vec::Vec<_>>()
                            .join(",")
                    ),
                    StdParams::Variadic {
                        fixed,
                        prefix,
                        start,
                        min,
                    } => format!(
                        "variadic({};{prefix};{start},{min})",
                        fixed
                            .iter()
                            .map(|p| p.as_str())
                            .collect::<alloc::vec::Vec<_>>()
                            .join(",")
                    ),
                };
                let expected: String = format!("{name} {shape}");
                assert!(baseline.lines().any(|line| line == expected), "{expected}");
            }
        }
        assert_eq!(StandardLibrary::preparation_demand(), (0, 1));
        assert_behavior_corpus(&library);
    }

    #[test]
    fn descriptors_are_pointer_sized_views_of_shared_signatures() {
        assert_eq!(
            core::mem::size_of::<(&str, super::super::StdFunctionRef<'_>)>(),
            4 * core::mem::size_of::<usize>(),
        );
        let library = StandardLibrary::new();
        assert!(core::ptr::eq(
            library.get("ABS").unwrap().params,
            library.get("DAY_OF_WEEK").unwrap().params
        ));
        assert!(core::ptr::eq(
            library.get("SUB").unwrap().params,
            library.get("ADD_TIME").unwrap().params
        ));
        assert!(core::ptr::eq(
            library.get("ADD").unwrap().params,
            library.get("CONCAT").unwrap().params
        ));
        assert!(!core::ptr::eq(
            library.get("ADD").unwrap().params,
            library.get("MUX").unwrap().params
        ));
    }

    #[test]
    fn owned_custom_metadata_and_overrides_preserve_case_and_clone_isolation() {
        let mut library = StandardLibrary::new();
        let original = library.clone();
        library.register("aBs", &["custom"], custom);
        assert_eq!(
            library.call("AbS", &[Value::Int(-7)]).unwrap(),
            Value::Int(73)
        );
        assert_eq!(
            original.call("abs", &[Value::Int(-7)]).unwrap(),
            Value::Int(7)
        );
        let StdParams::Fixed(params) = library.get("ABS").unwrap().params else {
            panic!("fixed override")
        };
        assert_eq!(params.as_ref(), &[smol_str::SmolStr::new("CUSTOM")]);
        library.register_variadic_with_fixed("custom", &["first"], "arg", 3, 2, custom);
        let StdParams::Variadic {
            fixed,
            prefix,
            start,
            min,
        } = library.get("CuStOm").unwrap().params
        else {
            panic!("variadic registration")
        };
        assert_eq!(fixed.as_ref(), &[smol_str::SmolStr::new("FIRST")]);
        assert_eq!(prefix, "ARG");
        assert_eq!((*start, *min), (3, 2));
        library.register("custom", &[], custom);
        assert!(
            matches!(library.get("CUSTOM").unwrap().params, StdParams::Fixed(params) if params.is_empty())
        );
        assert!(library.get("äbs").is_none());
        assert!(library.get(" ABS").is_none());
    }

    #[test]
    fn empty_default_and_partial_registration_remain_distinct_from_new() {
        let mut empty = StandardLibrary::default();
        assert!(empty.get("ABS").is_none());
        // Conversion fallback was available even in an empty library.
        assert_eq!(
            empty.call("INT_TO_DINT", &[Value::Int(2)]).unwrap(),
            Value::DInt(2)
        );
        super::super::time::register(&mut empty);
        assert!(empty.get("ADD_TIME").is_some());
        assert!(empty.get("ABS").is_none());
        for (name, expected) in super::super::time::FUNCTIONS {
            let found = empty.get(name).unwrap();
            assert_eq!(found.params, expected.params);
            assert_eq!((found.func)(&[]), (expected.func)(&[]));
        }
    }

    #[cfg(feature = "hir")]
    #[test]
    fn hosted_family_registration_preserves_implementations_and_parameters() {
        let mut library = StandardLibrary::default();
        super::super::assertions::register(&mut library);
        super::super::numeric::register(&mut library);
        super::super::bit::register(&mut library);
        super::super::selection::register(&mut library);
        super::super::comparison::register(&mut library);
        super::super::string::register(&mut library);
        super::super::time::register(&mut library);
        super::super::validate::register(&mut library);
        assert_eq!(
            library.functions.len(),
            include_str!("registration-baseline.txt").lines().count()
        );
        for family in FAMILIES {
            for (name, expected) in *family {
                let found = library.get(name).unwrap();
                assert_eq!(found.params, expected.params);
                assert_eq!((found.func)(&[]), (expected.func)(&[]));
            }
        }
        assert_behavior_corpus(&library);
    }
}

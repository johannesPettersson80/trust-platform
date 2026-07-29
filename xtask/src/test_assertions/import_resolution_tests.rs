use super::{extract_from_context, ContextSource};

#[test]
fn resolves_pub_super_helper_reexported_to_a_sibling_test_module() {
    let root_path = "crates/example/tests/contract.rs";
    let helper_path = "crates/example/tests/contract/support.rs";
    let test_path = "crates/example/tests/contract/part.rs";
    let inventory = extract_from_context(
        test_path,
        "delegates",
        vec![
            ContextSource {
                path: root_path.to_string(),
                module: Vec::new(),
                source: "use support::*;\n".to_string(),
            },
            ContextSource {
                path: helper_path.to_string(),
                module: vec!["support".to_string()],
                source: "pub(super) fn assert_contract() { assert!(true); }\n".to_string(),
            },
            ContextSource {
                path: test_path.to_string(),
                module: vec!["part".to_string()],
                source: concat!(
                    "use super::*;\n",
                    "#[test]\n",
                    "fn delegates() { assert_contract(); }\n",
                )
                .to_string(),
            },
        ],
    )
    .expect("extract parent-visible sibling helper assertions");

    assert!(
        inventory.unresolved_assertion_helpers.is_empty(),
        "pub(super) helper must resolve for sibling test modules: {:?}",
        inventory.unresolved_assertion_helpers
    );
    assert_eq!(1, inventory.assertions.len());
    assert_eq!("support::assert_contract", inventory.assertions[0].function);
    assert_eq!(helper_path, inventory.assertions[0].source_path);
}

#[test]
fn resolves_repeated_super_glob_imports_to_the_ancestor_scope() {
    let root_path = "crates/example/tests/contract.rs";
    let middle_path = "crates/example/tests/contract/middle.rs";
    let test_path = "crates/example/tests/contract/middle/part.rs";
    let inventory = extract_from_context(
        test_path,
        "delegates",
        vec![
            ContextSource {
                path: root_path.to_string(),
                module: Vec::new(),
                source: "fn assert_contract() { assert!(true); }\n".to_string(),
            },
            ContextSource {
                path: middle_path.to_string(),
                module: vec!["middle".to_string()],
                source: String::new(),
            },
            ContextSource {
                path: test_path.to_string(),
                module: vec!["middle".to_string(), "part".to_string()],
                source: concat!(
                    "use super::super::*;\n",
                    "#[test]\n",
                    "fn delegates() { assert_contract(); }\n",
                )
                .to_string(),
            },
        ],
    )
    .expect("extract repeated-super helper assertions");

    assert!(
        inventory.unresolved_assertion_helpers.is_empty(),
        "repeated super import must resolve to the root scope: {:?}",
        inventory.unresolved_assertion_helpers
    );
    assert_eq!(1, inventory.assertions.len());
    assert_eq!("assert_contract", inventory.assertions[0].function);
    assert_eq!(root_path, inventory.assertions[0].source_path);
}

#[test]
fn resolves_named_imports_reexported_through_a_parent_module() {
    let root_path = "crates/example/tests/runtime.rs";
    let lower_path = "crates/example/tests/runtime/lower.rs";
    let implementation_path = "crates/example/tests/runtime/implementation.rs";
    let test_path = "crates/example/tests/runtime/tests.rs";
    let inventory = extract_from_context(
        test_path,
        "delegates",
        vec![
            ContextSource {
                path: root_path.to_string(),
                module: vec![
                    "runtime".to_string(),
                    "vm".to_string(),
                    "register_ir".to_string(),
                ],
                source: "use self::lower::assert_contract;\n".to_string(),
            },
            ContextSource {
                path: lower_path.to_string(),
                module: vec![
                    "runtime".to_string(),
                    "vm".to_string(),
                    "register_ir".to_string(),
                    "lower".to_string(),
                ],
                source: "pub(super) use self::implementation::*;\n".to_string(),
            },
            ContextSource {
                path: implementation_path.to_string(),
                module: vec![
                    "runtime".to_string(),
                    "vm".to_string(),
                    "register_ir".to_string(),
                    "lower".to_string(),
                    "implementation".to_string(),
                ],
                source: concat!(
                    "pub(in crate::runtime::vm::register_ir) ",
                    "fn assert_contract() { assert!(true); }\n",
                )
                .to_string(),
            },
            ContextSource {
                path: test_path.to_string(),
                module: vec![
                    "runtime".to_string(),
                    "vm".to_string(),
                    "register_ir".to_string(),
                    "tests".to_string(),
                ],
                source: concat!(
                    "use super::assert_contract;\n",
                    "#[test]\n",
                    "fn delegates() { assert_contract(); }\n",
                )
                .to_string(),
            },
        ],
    )
    .expect("extract transitively imported parent helper");

    assert!(
        inventory.unresolved_assertion_helpers.is_empty(),
        "parent named imports must resolve transitively: {:?}",
        inventory.unresolved_assertion_helpers
    );
    assert_eq!(1, inventory.assertions.len());
    assert_eq!(
        "runtime::vm::register_ir::lower::implementation::assert_contract",
        inventory.assertions[0].function
    );
    assert_eq!(implementation_path, inventory.assertions[0].source_path);
}

#[test]
fn resolves_function_local_named_imports() {
    let test_path = "crates/example/tests/contract.rs";
    let helper_path = "crates/example/tests/support.rs";
    let inventory = extract_from_context(
        test_path,
        "delegates",
        vec![
            ContextSource {
                path: test_path.to_string(),
                module: Vec::new(),
                source: concat!(
                    "#[test]\n",
                    "fn delegates() {\n",
                    "    use crate::support::assert_contract;\n",
                    "    assert_contract();\n",
                    "}\n",
                )
                .to_string(),
            },
            ContextSource {
                path: helper_path.to_string(),
                module: vec!["support".to_string()],
                source: "pub(crate) fn assert_contract() { assert!(true); }\n".to_string(),
            },
        ],
    )
    .expect("extract function-local imported helper");

    assert!(
        inventory.unresolved_assertion_helpers.is_empty(),
        "function-local imports must resolve: {:?}",
        inventory.unresolved_assertion_helpers
    );
    assert_eq!(1, inventory.assertions.len());
    assert_eq!("support::assert_contract", inventory.assertions[0].function);
    assert_eq!(helper_path, inventory.assertions[0].source_path);
}

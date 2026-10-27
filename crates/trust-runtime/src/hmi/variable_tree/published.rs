//! Typed metadata for explicitly published mesh variables (specification 39 §7.9), read in the
//! running program through its symbol and type tables (specification 44 §13.3).
use super::*;
use crate::hmi::commands::write::write_block;
use crate::hmi::member_paths::{locate_member, MemberPath};
use crate::runtime::program_access::ProgramRead;
use trust_runtime_core::native::memory::{layout, Layout};

/// Describes only configured publish paths, retaining the local binding write restrictions.
/// Missing paths are omitted; a published container does not advertise unpublished children.
///
/// # Errors
///
/// None today; the result keeps the catalogue's answer, which a peer reads as a refusal.
pub fn published_variables(
    program: &dyn ProgramRead,
    paths: &[String],
) -> Result<Vec<VariableNode>, String> {
    let mut nodes = Vec::new();
    for written in paths {
        let Ok(path) = MemberPath::parse(written) else {
            continue;
        };
        let Some(at) = locate_member(&path, program) else {
            continue;
        };
        let name = written.rsplit('.').next().unwrap_or(written);
        let block = write_block(program, &path);
        let mut node = VariableNode {
            name: name.to_string(),
            path: written.clone(),
            kind: VariableKind::Value,
            type_name: String::new(),
            kinds: Vec::new(),
            writable: block.is_none(),
            write_block: block,
            access: None,
            children: Vec::new(),
            array: None,
        };
        match layout(&program.program_image(), at.type_index) {
            Ok(Layout::Instance(_)) => {
                node.kind = if program
                    .program_image()
                    .find_symbol(&written.to_ascii_uppercase())
                    .is_some_and(|(_, symbol)| {
                        symbol.flags & trust_runtime_core::native::image::SYMBOL_PROGRAM != 0
                    }) {
                    VariableKind::Program
                } else {
                    VariableKind::FunctionBlock
                };
                node.type_name = program.type_name_of(&at).to_string();
            }
            Ok(Layout::Instances { .. }) => {
                node.kind = VariableKind::Array;
                node.type_name = program.type_name_of(&at).to_string();
            }
            Ok(Layout::Value) => {
                let Ok(value) = program.read_at(&at) else {
                    continue;
                };
                node.type_name = value_type_name(&value).unwrap_or_default();
                node.kinds = ValueKind::kinds_of(&value);
                match &value {
                    Value::Struct(_) => node.kind = VariableKind::Structure,
                    Value::Array(array) => {
                        node.kind = VariableKind::Array;
                        let bounds = array
                            .dimensions()
                            .iter()
                            .map(|(lower, upper)| format!("{lower}..{upper}"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        let element = array
                            .elements()
                            .first()
                            .and_then(value_type_name)
                            .unwrap_or_default();
                        node.type_name = format!("ARRAY[{bounds}] OF {element}");
                    }
                    _ => {}
                }
            }
            Err(_) => continue,
        }
        nodes.push(node);
    }
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::TestHarness;

    const PROGRAM: &str = r#"
FUNCTION_BLOCK Pump
VAR_INPUT
    Demand : REAL;
END_VAR
VAR_OUTPUT
    Running : BOOL;
END_VAR
    Running := TRUE;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    P101 : Pump;
    Levels : ARRAY[1..2] OF INT := [12, 24];
END_VAR
    P101();
END_PROGRAM
"#;

    #[test]
    fn published_catalogue_preserves_types_and_owner_write_restrictions() {
        let mut harness = TestHarness::from_source(PROGRAM).unwrap();
        harness.cycle();
        let nodes = harness
            .runtime()
            .with_program_view(|program| {
                published_variables(
                    program,
                    &[
                        "Main.P101.Running".into(),
                        "main.p101.demand".into(),
                        "Main.Levels[2]".into(),
                        "Main.Missing".into(),
                        "Main.Levels[8]".into(),
                    ],
                )
            })
            .unwrap();
        assert_eq!(
            nodes.len(),
            3,
            "only the existing explicitly published variables"
        );
        assert_eq!(nodes[0].type_name, "BOOL");
        assert!(!nodes[0].writable);
        assert!(nodes[0].write_block.as_deref().unwrap().contains("output"));
        assert_eq!(nodes[1].path, "main.p101.demand");
        assert_eq!(nodes[1].type_name, "REAL");
        assert!(nodes[1].writable);
        assert_eq!(nodes[2].type_name, "INT");
        assert!(nodes[2].kinds.contains(&ValueKind::Number));
    }

    #[test]
    fn a_published_container_does_not_offer_its_unpublished_members_as_individual_tags() {
        let mut harness = TestHarness::from_source(PROGRAM).unwrap();
        harness.cycle();
        let nodes = harness
            .runtime()
            .with_program_view(|program| {
                published_variables(program, &["Main.P101".into(), "Main.Levels".into()])
            })
            .unwrap();
        assert_eq!(nodes.len(), 2);
        assert!(nodes
            .iter()
            .all(|node| node.children.is_empty() && node.array.is_none()));
    }
}

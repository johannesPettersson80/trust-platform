//! Typed metadata for explicitly published mesh variables (specification 39 §7.9).
use super::*;
use crate::hmi::member_paths::{resolve_member_with, MemberPath};

/// Describes only configured publish paths, retaining the local binding write restrictions.
/// Missing paths are omitted; a published container does not advertise unpublished children.
pub fn published_variables(
    metadata: &RuntimeMetadata,
    storage: &VariableStorage,
    paths: &[String],
) -> Result<Vec<VariableNode>, String> {
    let mut builder = TreeBuilder {
        metadata,
        storage,
        ancestors: Vec::new(),
        visited: 0,
    };
    let mut nodes = Vec::new();
    for written in paths {
        let Ok(path) = MemberPath::parse(written) else {
            continue;
        };
        let mut block = global_write_block(metadata, &path.root);
        let Some(value) = resolve_member_with(&path, storage, |kind, member| {
            if block.is_none() {
                block = member_write_block(metadata, kind, member);
            }
        }) else {
            continue;
        };
        let name = written.rsplit('.').next().unwrap_or(written);
        let mut node = builder.node(name, written.clone(), value, None, block, 0)?;
        node.children.clear();
        node.array = None;
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
        let runtime = harness.runtime();
        let nodes = published_variables(
            &runtime.metadata_snapshot(),
            runtime.storage(),
            &[
                "Main.P101.Running".into(),
                "main.p101.demand".into(),
                "Main.Levels[2]".into(),
                "Main.Missing".into(),
                "Main.Levels[8]".into(),
            ],
        )
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
        let runtime = harness.runtime();
        let nodes = published_variables(
            &runtime.metadata_snapshot(),
            runtime.storage(),
            &["Main.P101".into(), "Main.Levels".into()],
        )
        .unwrap();
        assert_eq!(nodes.len(), 2);
        assert!(nodes
            .iter()
            .all(|node| node.children.is_empty() && node.array.is_none()));
    }
}

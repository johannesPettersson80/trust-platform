//! The OPC UA browse walk: continuation points, and the depth and node bounds.
//!
//! The walk runs against `OpcUaBrowseSource`, so a live session and a test source that answers
//! in pages share one implementation. It follows every `BrowseNext` continuation point, and
//! when the depth or node bound stops the tree it records a warning naming the bound (design
//! row E21).

use ::opcua::client::prelude::{AttributeService, Session, ViewService};
use ::opcua::types::{
    BrowseDescription, BrowseDescriptionResultMask, BrowseDirection, ByteString, NodeClass,
    NodeClassMask, NodeId, ReferenceDescription, ReferenceTypeId,
};

use super::{localized_text, opcua_status_error, OpcUaBrowseNode, OpcUaBrowseResult, RuntimeError};

/// One page of a node's forward hierarchical references and the server's continuation point.
pub(super) struct OpcUaBrowsePage {
    references: Vec<ReferenceDescription>,
    continuation_point: ByteString,
}

/// The two OPC UA services the browse walk needs.
///
/// A live session and a test source that answers in pages both implement it, so the walk's
/// continuation and bound rules are proven without a server.
pub(super) trait OpcUaBrowseSource {
    /// One page of references, continuing from `continuation_point` when it is present.
    fn browse_page(
        &self,
        node_id: &NodeId,
        continuation_point: Option<&ByteString>,
    ) -> Result<OpcUaBrowsePage, RuntimeError>;

    /// The apply-facing data-type id, type name and writability of a variable node.
    fn variable_metadata(&self, node_id: &NodeId) -> (String, String, bool);
}

/// The live-session browse source.
pub(super) struct SessionBrowseSource<'a> {
    pub(super) session: &'a Session,
}

impl OpcUaBrowseSource for SessionBrowseSource<'_> {
    fn browse_page(
        &self,
        node_id: &NodeId,
        continuation_point: Option<&ByteString>,
    ) -> Result<OpcUaBrowsePage, RuntimeError> {
        let results = match continuation_point {
            Some(point) => self
                .session
                .browse_next(false, std::slice::from_ref(point))
                .map_err(opcua_status_error)?,
            None => self
                .session
                .browse(&[BrowseDescription {
                    node_id: node_id.clone(),
                    browse_direction: BrowseDirection::Forward,
                    reference_type_id: ReferenceTypeId::HierarchicalReferences.into(),
                    include_subtypes: true,
                    node_class_mask: (NodeClassMask::OBJECT | NodeClassMask::VARIABLE).bits(),
                    result_mask: BrowseDescriptionResultMask::all().bits(),
                }])
                .map_err(opcua_status_error)?,
        };
        let Some(result) = results.and_then(|results| results.into_iter().next()) else {
            return Ok(empty_page());
        };
        Ok(OpcUaBrowsePage {
            references: result.references.unwrap_or_default(),
            continuation_point: result.continuation_point,
        })
    }

    fn variable_metadata(&self, node_id: &NodeId) -> (String, String, bool) {
        variable_metadata(self.session, node_id.clone())
    }
}

fn empty_page() -> OpcUaBrowsePage {
    OpcUaBrowsePage {
        references: Vec::new(),
        continuation_point: ByteString::null(),
    }
}

/// The depth and node bounds of one browse, and whether each cut the tree.
struct OpcUaBrowseBounds {
    max_depth: usize,
    max_nodes: usize,
    count: usize,
    depth_cut: bool,
    node_cut: bool,
}

/// Walk the Objects folder into a tree, following continuation points within the bounds.
pub(super) fn browse_tree(
    source: &dyn OpcUaBrowseSource,
    max_depth: usize,
    max_nodes: usize,
) -> Result<OpcUaBrowseResult, RuntimeError> {
    let mut bounds = OpcUaBrowseBounds {
        max_depth,
        max_nodes,
        count: 0,
        depth_cut: false,
        node_cut: false,
    };
    let nodes = browse_children(source, NodeId::objects_folder_id(), "", 0, &mut bounds)?;
    let mut warnings = Vec::new();
    if bounds.depth_cut {
        warnings.push(format!(
            "OPC UA browse reached the maximum depth of {max_depth}; deeper nodes were not read"
        ));
    }
    if bounds.node_cut {
        warnings.push(format!(
            "OPC UA browse stopped after {max_nodes} nodes; further nodes were not read"
        ));
    }
    Ok(OpcUaBrowseResult { nodes, warnings })
}

/// Every forward hierarchical reference of `node_id`, following continuation points until the
/// server has no more or `limit` references were read.
///
/// The flag reports that `limit` cut a longer list, so the caller can warn instead of returning
/// a partial node silently.
fn browse_all_references(
    source: &dyn OpcUaBrowseSource,
    node_id: &NodeId,
    limit: usize,
) -> Result<(Vec<ReferenceDescription>, bool), RuntimeError> {
    let mut references = Vec::new();
    let mut continuation = None;
    loop {
        let page = source.browse_page(node_id, continuation.as_ref())?;
        for reference in page.references {
            if references.len() >= limit {
                return Ok((references, true));
            }
            references.push(reference);
        }
        if page.continuation_point.is_null() {
            return Ok((references, false));
        }
        if references.len() >= limit {
            return Ok((references, true));
        }
        continuation = Some(page.continuation_point);
    }
}

fn browse_children(
    source: &dyn OpcUaBrowseSource,
    node_id: NodeId,
    parent_path: &str,
    depth: usize,
    bounds: &mut OpcUaBrowseBounds,
) -> Result<Vec<OpcUaBrowseNode>, RuntimeError> {
    let (references, truncated) = browse_all_references(source, &node_id, bounds.max_nodes)?;
    if depth >= bounds.max_depth {
        if !references.is_empty() || truncated {
            bounds.depth_cut = true;
        }
        return Ok(Vec::new());
    }
    if bounds.count >= bounds.max_nodes {
        if !references.is_empty() || truncated {
            bounds.node_cut = true;
        }
        return Ok(Vec::new());
    }
    let mut nodes = Vec::new();
    for reference in references {
        if bounds.count >= bounds.max_nodes {
            bounds.node_cut = true;
            break;
        }
        let child_id = reference.node_id.node_id.clone();
        if child_id.is_null() {
            continue;
        }
        let name = localized_text(reference.display_name)
            .unwrap_or_else(|| reference.browse_name.name.to_string());
        let path = if parent_path.is_empty() {
            name.clone()
        } else {
            format!("{parent_path}/{name}")
        };
        let is_variable = reference.node_class == NodeClass::Variable;
        let (data_type_id, data_type, writable) = if is_variable {
            source.variable_metadata(&child_id)
        } else {
            ("Object".to_string(), "object".to_string(), false)
        };
        bounds.count += 1;
        let children = browse_children(source, child_id.clone(), path.as_str(), depth + 1, bounds)?;
        nodes.push(OpcUaBrowseNode {
            id: child_id.to_string(),
            name,
            path,
            data_type_id,
            data_type,
            writable,
            children,
        });
    }
    if truncated {
        bounds.node_cut = true;
    }
    Ok(nodes)
}

/// The apply-facing data-type id, type name and writability of a variable node.
fn variable_metadata(session: &Session, node_id: NodeId) -> (String, String, bool) {
    let values = session
        .read(
            &[
                ::opcua::types::ReadValueId {
                    node_id: node_id.clone(),
                    attribute_id: ::opcua::types::AttributeId::DataType as u32,
                    index_range: ::opcua::types::UAString::null(),
                    data_encoding: ::opcua::types::QualifiedName::null(),
                },
                ::opcua::types::ReadValueId {
                    node_id,
                    attribute_id: ::opcua::types::AttributeId::UserAccessLevel as u32,
                    index_range: ::opcua::types::UAString::null(),
                    data_encoding: ::opcua::types::QualifiedName::null(),
                },
            ],
            ::opcua::types::TimestampsToReturn::Neither,
            0.0,
        )
        .unwrap_or_default();
    let data_type_id = values
        .first()
        .and_then(|value| value.value.as_ref())
        .map(|value| value.to_string())
        .unwrap_or_else(|| "Value".to_string());
    let data_type = opcua_data_type_for_apply(data_type_id.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| "unknown".to_string());
    let writable = values
        .get(1)
        .and_then(|value| value.value.as_ref())
        .and_then(|value| match value {
            ::opcua::types::Variant::Byte(bits) => Some(bits & 0b10 != 0),
            ::opcua::types::Variant::UInt16(bits) => Some(bits & 0b10 != 0),
            ::opcua::types::Variant::UInt32(bits) => Some(bits & 0b10 != 0),
            _ => None,
        })
        .unwrap_or(false);
    (data_type_id, data_type, writable)
}

fn opcua_data_type_for_apply(data_type_id: &str) -> Option<&'static str> {
    match data_type_id {
        "i=1" | "ns=0;i=1" => Some("bool"),
        "i=2" | "ns=0;i=2" | "i=4" | "ns=0;i=4" => Some("int16"),
        "i=3" | "ns=0;i=3" | "i=5" | "ns=0;i=5" => Some("uint16"),
        "i=6" | "ns=0;i=6" => Some("int32"),
        "i=7" | "ns=0;i=7" => Some("uint32"),
        "i=8" | "ns=0;i=8" => Some("int64"),
        "i=9" | "ns=0;i=9" => Some("uint64"),
        "i=10" | "ns=0;i=10" => Some("float"),
        "i=11" | "ns=0;i=11" => Some("double"),
        "i=12" | "ns=0;i=12" => Some("string"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use ::opcua::types::{ExpandedNodeId, LocalizedText, QualifiedName};

    use super::*;

    /// A fake browse source that answers each node in the pages it was given.
    struct FakeBrowseSource {
        pages: HashMap<NodeId, Vec<Vec<ReferenceDescription>>>,
    }

    impl OpcUaBrowseSource for FakeBrowseSource {
        fn browse_page(
            &self,
            node_id: &NodeId,
            continuation_point: Option<&ByteString>,
        ) -> Result<OpcUaBrowsePage, RuntimeError> {
            let index = continuation_point.map_or(0, |point| {
                let bytes: &[u8] = point.as_ref();
                bytes.first().copied().map_or(0, usize::from)
            });
            let pages = self.pages.get(node_id);
            let page_count = pages.map_or(0, |pages| pages.len());
            let Some(page) = pages.and_then(|pages| pages.get(index)) else {
                return Ok(empty_page());
            };
            let continuation_point = if index + 1 < page_count {
                ByteString::from(vec![
                    u8::try_from(index + 1).expect("page index fits a byte")
                ])
            } else {
                ByteString::null()
            };
            Ok(OpcUaBrowsePage {
                references: page.clone(),
                continuation_point,
            })
        }

        fn variable_metadata(&self, _node_id: &NodeId) -> (String, String, bool) {
            ("Object".to_string(), "object".to_string(), false)
        }
    }

    fn object_reference(name: &str) -> ReferenceDescription {
        ReferenceDescription {
            reference_type_id: ReferenceTypeId::Organizes.into(),
            is_forward: true,
            node_id: ExpandedNodeId::new(NodeId::new(1u16, name.to_string())),
            browse_name: QualifiedName::new(1u16, name),
            display_name: LocalizedText::new("", name),
            node_class: NodeClass::Object,
            type_definition: ExpandedNodeId::null(),
        }
    }

    fn source_with_root_pages(pages: Vec<Vec<ReferenceDescription>>) -> FakeBrowseSource {
        let mut map = HashMap::new();
        map.insert(NodeId::objects_folder_id(), pages);
        FakeBrowseSource { pages: map }
    }

    fn node_names(nodes: &[OpcUaBrowseNode]) -> Vec<&str> {
        nodes.iter().map(|node| node.name.as_str()).collect()
    }

    #[test]
    fn opcua_browse_follows_continuation_points() {
        // Design row E21: a server that answers one node in pages of two must still yield the
        // whole tree. A walk that reads only the first answer returns two nodes, not five.
        let source = source_with_root_pages(vec![
            vec![object_reference("Alpha"), object_reference("Beta")],
            vec![object_reference("Gamma"), object_reference("Delta")],
            vec![object_reference("Epsilon")],
        ]);

        let result = browse_tree(&source, 4, 512).expect("browse the paged fake tree");

        assert_eq!(
            node_names(&result.nodes),
            vec!["Alpha", "Beta", "Gamma", "Delta", "Epsilon"]
        );
        assert_eq!(result.warnings, Vec::<String>::new());
    }

    #[test]
    fn a_browse_cut_by_its_bound_says_so() {
        // Spec 11 §6.5: a node bound that limits the tree is named in a warning. A silent
        // truncation that returns two nodes without a warning is the wrong behavior this
        // catches.
        let source = source_with_root_pages(vec![vec![
            object_reference("Alpha"),
            object_reference("Beta"),
            object_reference("Gamma"),
        ]]);

        let result = browse_tree(&source, 4, 2).expect("browse within the node bound");

        assert_eq!(node_names(&result.nodes), vec!["Alpha", "Beta"]);
        assert_eq!(
            result.warnings,
            vec!["OPC UA browse stopped after 2 nodes; further nodes were not read".to_string()]
        );
    }

    #[test]
    fn a_browse_cut_at_the_depth_bound_says_so() {
        // Spec 11 §6.5: a depth bound that hides a node's children is named in a warning. A
        // walk that stops one level too deep, or reports no warning, is the wrong behavior this
        // catches.
        let mut source = source_with_root_pages(vec![vec![object_reference("Alpha")]]);
        source.pages.insert(
            NodeId::new(1u16, "Alpha"),
            vec![vec![object_reference("Beta")]],
        );
        source.pages.insert(
            NodeId::new(1u16, "Beta"),
            vec![vec![object_reference("Gamma")]],
        );

        let result = browse_tree(&source, 2, 512).expect("browse to the depth bound");

        assert_eq!(node_names(&result.nodes), vec!["Alpha"]);
        assert_eq!(node_names(&result.nodes[0].children), vec!["Beta"]);
        assert_eq!(
            result.warnings,
            vec![
                "OPC UA browse reached the maximum depth of 2; deeper nodes were not read"
                    .to_string()
            ]
        );
    }
}

//! Bounded concrete equipment paths from declaration shapes, including multidimensional arrays.
use trust_runtime::hmi::variable_tree::VariableNode;
pub(in crate::handlers::commands) fn matching(
    nodes: &[VariableNode],
    limit: usize,
    accept: &dyn Fn(&VariableNode) -> bool,
) -> Result<Vec<VariableNode>, String> {
    let mut found = Vec::new();
    for node in nodes {
        visit(node, limit, accept, &mut found)?;
    }
    Ok(found)
}
fn visit(
    node: &VariableNode,
    limit: usize,
    accept: &dyn Fn(&VariableNode) -> bool,
    found: &mut Vec<VariableNode>,
) -> Result<(), String> {
    if accept(node) {
        if found.len() >= limit {
            return Err(format!("Equipment selection exceeds {limit} instances"));
        }
        found.push(node.clone());
        return Ok(());
    }
    for child in &node.children {
        visit(child, limit, accept, found)?;
    }
    if let Some(array) = &node.array {
        if !has_equipment(&array.element) {
            return Ok(());
        }
        let mut indices = vec![Vec::<i64>::new()];
        for [low, high] in &array.dimensions {
            let count = high
                .checked_sub(*low)
                .and_then(|n| n.checked_add(1))
                .and_then(|n| usize::try_from(n).ok())
                .ok_or("Invalid equipment array bounds")?;
            if indices.len().saturating_mul(count) > limit {
                return Err(format!(
                    "{} exceeds the {limit}-element selection limit",
                    node.path
                ));
            }
            indices = indices
                .into_iter()
                .flat_map(|prefix| {
                    (*low..=*high).map(move |value| {
                        let mut next = prefix.clone();
                        next.push(value);
                        next
                    })
                })
                .collect();
        }
        for indices in indices {
            let path = format!(
                "{}[{}]",
                node.path,
                indices
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let mut element = (*array.element).clone();
            let previous = element.path.clone();
            rebase(&mut element, &previous, &path);
            visit(&element, limit, accept, found)?;
        }
    }
    Ok(())
}
fn has_equipment(node: &VariableNode) -> bool {
    node.kind == trust_runtime::hmi::variable_tree::VariableKind::FunctionBlock
        || node.children.iter().any(has_equipment)
        || node
            .array
            .as_ref()
            .is_some_and(|array| has_equipment(&array.element))
}
fn rebase(node: &mut VariableNode, from: &str, to: &str) {
    if let Some(suffix) = node.path.strip_prefix(from) {
        node.path = format!("{to}{suffix}");
    }
    for child in &mut node.children {
        rebase(child, from, to);
    }
    if let Some(array) = &mut node.array {
        rebase(&mut array.element, from, to);
    }
}

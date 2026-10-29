//! `TYPE Derived EXTENDS Base : STRUCT … END_STRUCT` (CODESYS DUT inheritance, a vendor extension
//! every profile accepts; specification 02 §3.5.1): the structure holds the base's fields first,
//! then its own, so that `derived.a` reads the base's field and a copy to the base's type takes the
//! base's part. Bases are added after every file's types are lowered, so a base in another file or
//! declared after its derived type is found.

use std::collections::HashSet;

use smol_str::SmolStr;
use trust_hir::types::TypeRegistry;
use trust_hir::{Type, TypeId};
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::super::types::CompileError;
use super::super::util::node_text;
use super::types::qualify_with_namespaces;

/// Gives each structure of `syntaxes` declared with `EXTENDS` its base's fields, ahead of its own.
pub(crate) fn add_struct_bases(
    syntaxes: &[SyntaxNode],
    registry: &mut TypeRegistry,
) -> Result<(), CompileError> {
    let mut extended = Vec::new();
    for root in syntaxes {
        for declaration in root
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::TypeDecl)
        {
            let mut derived = None;
            for child in declaration.children() {
                match child.kind() {
                    SyntaxKind::Name => {
                        derived = Some(qualify_with_namespaces(&declaration, &node_text(&child)));
                    }
                    SyntaxKind::ExtendsClause => {
                        let base = child
                            .children()
                            .find(|node| node.kind() == SyntaxKind::Name)
                            .map(|name| node_text(&name));
                        if let (Some(derived), Some(base)) = (derived.clone(), base) {
                            let qualified = qualify_with_namespaces(&declaration, &base);
                            extended.push((derived, qualified, SmolStr::new(base)));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    let mut done = HashSet::new();
    for (derived, _, _) in &extended {
        add_base(derived, &extended, registry, &mut done, &mut Vec::new())?;
    }
    Ok(())
}

/// Adds the base's fields to `derived`, its own base's first.
fn add_base(
    derived: &SmolStr,
    extended: &[(SmolStr, SmolStr, SmolStr)],
    registry: &mut TypeRegistry,
    done: &mut HashSet<SmolStr>,
    visiting: &mut Vec<SmolStr>,
) -> Result<(), CompileError> {
    if done.contains(derived) {
        return Ok(());
    }
    let Some((_, qualified, written)) = extended.iter().find(|(name, _, _)| name == derived) else {
        return Ok(());
    };
    if visiting.contains(derived) {
        return Err(CompileError::new(format!(
            "structure {derived} extends itself through its bases"
        )));
    }
    let base = registry
        .lookup(qualified)
        .or_else(|| registry.lookup(written))
        .ok_or_else(|| CompileError::new(format!("unknown base structure {written}")))?;
    let base_name = registry.type_name(base).unwrap_or_else(|| written.clone());
    visiting.push(derived.clone());
    add_base(&base_name, extended, registry, done, visiting)?;
    visiting.pop();
    let base_fields = match registry.get(base) {
        Some(Type::Struct { fields, .. }) => fields.clone(),
        _ => {
            return Err(CompileError::new(format!(
                "the base {written} of structure {derived} is no structure"
            )))
        }
    };
    let id: TypeId = registry
        .lookup(derived)
        .ok_or_else(|| CompileError::new(format!("unknown structure {derived}")))?;
    if let Some(Type::Struct { name, fields }) = registry.get(id).cloned() {
        let mut all = base_fields;
        all.extend(fields);
        registry.replace(id, Type::Struct { name, fields: all });
    }
    done.insert(derived.clone());
    Ok(())
}

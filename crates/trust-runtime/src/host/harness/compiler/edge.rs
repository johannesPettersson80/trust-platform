use smol_str::SmolStr;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::program_model::EdgeInput;

use super::super::types::CompileError;
use super::pou::qualified_pou_name;
use super::vars::{edge_qualifier_from_decl, parse_var_decl, var_block_kind, VarBlockKind};

pub(crate) fn collect_edge_declarations(
    root: &SyntaxNode,
) -> Result<Vec<(SmolStr, Vec<EdgeInput>)>, CompileError> {
    let mut declarations = Vec::new();
    for pou in root
        .descendants()
        .filter(|node| matches!(node.kind(), SyntaxKind::Program | SyntaxKind::FunctionBlock))
    {
        let name = qualified_pou_name(&pou)?;
        let mut inputs = Vec::new();
        for block in pou
            .children()
            .filter(|child| child.kind() == SyntaxKind::VarBlock)
        {
            if var_block_kind(&block)? != VarBlockKind::Input {
                continue;
            }
            for declaration in block
                .children()
                .filter(|child| child.kind() == SyntaxKind::VarDecl)
            {
                let Some(qualifier) = edge_qualifier_from_decl(&declaration) else {
                    continue;
                };
                let parts = parse_var_decl(&declaration)?;
                inputs.extend(
                    parts
                        .names
                        .into_iter()
                        .map(|name| EdgeInput { name, qualifier }),
                );
            }
        }
        if !inputs.is_empty() {
            declarations.push((name, inputs));
        }
    }
    Ok(declarations)
}

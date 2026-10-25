//! `JMP` and labels (IEC §7.3.3, Table 72; specification 06 §12, §13; specification 44 §6.6): a
//! body whose statement list holds labels runs as a loop over its segments, each from a label to
//! the next, which a jump enters by number from anywhere in the body, a nested statement included,
//! and the end of a segment falls through to the next. Each pass of the loop takes one from the
//! statement budget, so that a backward jump cannot run without end (NX-10).

use smol_str::SmolStr;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::{call, Lowerer};
use crate::native::NativeError;

impl Lowerer<'_, '_> {
    /// Lowers the statement list `list` of a body: as it stands, or as the loop over its segments
    /// when it holds labels.
    pub(in crate::native) fn body(&mut self, list: &SyntaxNode) -> Result<(), NativeError> {
        let statements: Vec<SyntaxNode> = list
            .children()
            .filter(|child| super::super::front::is_statement(child.kind()))
            .collect();
        let mut starts = vec![0];
        let mut labels = Vec::new();
        for (index, statement) in statements.iter().enumerate() {
            if statement.kind() != SyntaxKind::LabelStmt {
                continue;
            }
            if index > 0 && starts.last() != Some(&index) {
                starts.push(index);
            }
            labels.push((label_name(statement), starts.len() - 1));
        }
        if labels.is_empty() {
            return self.statements(list.children());
        }
        self.jumps = Some(labels);
        self.line("let mut jmp_at: u32 = 0;");
        self.line("'jmp: loop {");
        self.indent += 1;
        self.line("tick(cx)?;");
        self.line("match jmp_at {");
        self.indent += 1;
        for (segment, start) in starts.iter().enumerate() {
            let end = starts.get(segment + 1).copied().unwrap_or(statements.len());
            self.line(&format!("{segment} => {{"));
            self.indent += 1;
            for statement in &statements[*start..end] {
                self.statement(statement)?;
            }
            self.line(&format!("jmp_at = {};", segment + 1));
            self.indent -= 1;
            self.line("}");
        }
        self.line("_ => break 'jmp,");
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        self.jumps = None;
        Ok(())
    }

    /// `JMP label;`: the segment of the label, from where it stands.
    pub(super) fn jump(&mut self, node: &SyntaxNode) -> Result<(), NativeError> {
        let target = node
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
            .map(|name| SmolStr::new(call::ident(&name).to_ascii_uppercase()));
        let segment = target.as_ref().and_then(|target| {
            self.jumps.as_ref().and_then(|labels| {
                labels
                    .iter()
                    .find(|(label, _)| label == target)
                    .map(|(_, segment)| *segment)
            })
        });
        let Some(segment) = segment else {
            return Err(self.unsupported(node, "a JMP to a label inside a statement"));
        };
        self.line(&format!("{{ jmp_at = {segment}; continue 'jmp; }}"));
        Ok(())
    }

    /// `label: statement`: the statement, which begins a segment of the body.
    pub(super) fn labelled(&mut self, node: &SyntaxNode) -> Result<(), NativeError> {
        let name = label_name(node);
        if !self
            .jumps
            .as_ref()
            .is_some_and(|labels| labels.iter().any(|(label, _)| *label == name))
        {
            return Err(self.unsupported(node, "a label inside a statement"));
        }
        self.statements(node.children())
    }
}

/// The name of the label of `node`, a `LabelStmt`, in capitals.
fn label_name(node: &SyntaxNode) -> SmolStr {
    node.children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .map_or_else(SmolStr::default, |name| {
            SmolStr::new(call::ident(&name).to_ascii_uppercase())
        })
}

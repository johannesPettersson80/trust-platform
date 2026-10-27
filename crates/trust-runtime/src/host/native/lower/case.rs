//! `CASE` (IEC §7.3.3.3.3; specification 06 §6): a `match` on an integer, a bit string, a `CHAR` or
//! an enumeration, and a chain of tests in source order on a selector of another elementary type,
//! a text or a real, evaluated once.

use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::Lowerer;
use crate::native::types::{Elem, Kind, Ty};
use crate::native::NativeError;

impl Lowerer<'_, '_> {
    pub(super) fn case_statement(&mut self, node: &SyntaxNode) -> Result<(), NativeError> {
        let mut children = node.children();
        let Some(selector) = children.next() else {
            return Err(self.unsupported(node, "CASE without a selector"));
        };
        let selector_ty = self.type_of(&selector)?;
        let Some(elem) = self
            .info(selector_ty)
            .scalar()
            .filter(|elem| elem.is_integer() || elem.is_bits() || *elem == Elem::Char)
        else {
            // A selector of another elementary type (IEC §7.3.3.3.3), a text or a real, is
            // compared with each label in turn.
            return self.case_chain(&selector, selector_ty, children);
        };
        // The labels as patterns. A label that names a constant variable, not a literal, makes the
        // CASE a chain of tests in source order, which compares it with the constant's value, which
        // never changes (IEC §7.3.3.3.3).
        let branches: Vec<SyntaxNode> = children.collect();
        let mut arms = Vec::new();
        for branch in &branches {
            if branch.kind() != SyntaxKind::CaseBranch {
                continue;
            }
            let mut patterns = Vec::new();
            for label in branch
                .children()
                .filter(|child| child.kind() == SyntaxKind::CaseLabel)
            {
                match self.case_label(&label, selector_ty, elem) {
                    Ok(pattern) => patterns.push(pattern),
                    Err(_) => return self.case_chain(&selector, selector_ty, branches.into_iter()),
                }
            }
            arms.push(patterns);
        }
        let value = self.scalar(&selector)?;
        let value = self.bind(&value);
        self.line(&format!("match {value} {{"));
        self.indent += 1;
        let mut has_else = false;
        let mut arms = arms.into_iter();
        for branch in branches {
            match branch.kind() {
                SyntaxKind::CaseBranch => {
                    let patterns = arms.next().unwrap_or_default();
                    self.line(&format!("{} => {{", patterns.join(" | ")));
                    self.indent += 1;
                    self.statements(
                        branch
                            .children()
                            .filter(|child| child.kind() != SyntaxKind::CaseLabel),
                    )?;
                    self.indent -= 1;
                    self.line("}");
                }
                SyntaxKind::ElseBranch => {
                    has_else = true;
                    self.line("_ => {");
                    self.indent += 1;
                    self.statements(branch.children())?;
                    self.indent -= 1;
                    self.line("}");
                }
                _ => {}
            }
        }
        if !has_else {
            self.line("_ => {}");
        }
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    /// A CASE whose selector of type `selector_ty` is no integer, or one with a label that names a
    /// constant: the selector evaluated once, and the branches `branches` as a chain of tests of its
    /// labels in source order, each label equal to the selector as `=` compares them, or a range
    /// holding it (specification 06 §6).
    fn case_chain(
        &mut self,
        selector: &SyntaxNode,
        selector_ty: Ty,
        branches: impl Iterator<Item = SyntaxNode>,
    ) -> Result<(), NativeError> {
        let text = matches!(self.info(selector_ty).kind, Kind::Str(_) | Kind::WStr(_));
        let compare = if matches!(self.info(selector_ty).kind, Kind::WStr(_)) {
            "compare_wide"
        } else {
            "compare"
        };
        if !text && self.info(selector_ty).scalar().is_none() {
            return Err(self.unsupported(selector, "a CASE selector of this type"));
        }
        let value = if text {
            let (reference, _) = self.text_ref(selector)?;
            self.bind(&reference)
        } else {
            let value = self.scalar(selector)?;
            self.bind(&value)
        };
        // The tests of each branch, worked out before the chain: a label is a constant.
        let mut tests = Vec::new();
        let mut otherwise = None;
        for branch in branches {
            match branch.kind() {
                SyntaxKind::CaseBranch => {
                    let mut labels = Vec::new();
                    for label in branch
                        .children()
                        .filter(|child| child.kind() == SyntaxKind::CaseLabel)
                    {
                        let Some(range) = label.children().next() else {
                            return Err(self.unsupported(&label, "an empty CASE label"));
                        };
                        let bounds: Vec<SyntaxNode> = if range.kind() == SyntaxKind::Subrange {
                            range.children().collect()
                        } else {
                            vec![range]
                        };
                        let mut held = Vec::with_capacity(2);
                        for bound in &bounds {
                            held.push(if text {
                                let (reference, _) = self.text_ref(bound)?;
                                format!("lib::{compare}(({value}).view(), ({reference}).view())")
                            } else {
                                let bound = self.scalar_to(bound, selector_ty)?;
                                format!("{value}.partial_cmp(&{bound})")
                            });
                        }
                        labels.push(match (held.as_slice(), text) {
                            ([single], true) => {
                                format!("({single} == core::cmp::Ordering::Equal)")
                            }
                            ([single], false) => {
                                format!("({single} == Some(core::cmp::Ordering::Equal))")
                            }
                            // trust-hir refuses a range of texts (specification 06 §6).
                            ([lower, upper], false) => format!(
                                "(matches!({lower}, Some(core::cmp::Ordering::Equal | core::cmp::Ordering::Greater)) && matches!({upper}, Some(core::cmp::Ordering::Less | core::cmp::Ordering::Equal)))"
                            ),
                            _ => return Err(self.unsupported(&label, "this CASE label")),
                        });
                    }
                    tests.push((labels.join(" || "), branch));
                }
                SyntaxKind::ElseBranch => otherwise = Some(branch),
                _ => {}
            }
        }
        let chained = !tests.is_empty();
        for (index, (test, branch)) in tests.into_iter().enumerate() {
            let keyword = if index == 0 { "if" } else { "} else if" };
            self.line(&format!("{keyword} {test} {{"));
            self.indent += 1;
            self.statements(
                branch
                    .children()
                    .filter(|child| child.kind() != SyntaxKind::CaseLabel),
            )?;
            self.indent -= 1;
        }
        if let Some(branch) = otherwise {
            self.line(if chained { "} else {" } else { "{" });
            self.indent += 1;
            self.statements(branch.children())?;
            self.indent -= 1;
            self.line("}");
        } else if chained {
            self.line("}");
        }
        Ok(())
    }

    /// One CASE label as a Rust pattern: a value or an inclusive range, of the selector's type.
    fn case_label(
        &mut self,
        label: &SyntaxNode,
        selector: Ty,
        elem: Elem,
    ) -> Result<String, NativeError> {
        let Some(range) = label.children().next() else {
            return Err(self.unsupported(label, "an empty CASE label"));
        };
        let bounds: Vec<SyntaxNode> = if range.kind() == SyntaxKind::Subrange {
            range.children().collect()
        } else {
            vec![range]
        };
        let mut values = Vec::with_capacity(2);
        for bound in &bounds {
            let value = self.constant_integer(bound, selector)?;
            values.push(format!("{value}{}", elem.rust()));
        }
        match values.as_slice() {
            [single] => Ok(single.clone()),
            [lower, upper] => Ok(format!("{lower}..={upper}")),
            _ => Err(self.unsupported(label, "a CASE label of more than two bounds")),
        }
    }
}

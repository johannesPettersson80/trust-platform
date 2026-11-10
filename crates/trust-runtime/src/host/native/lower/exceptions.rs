//! CODESYS's exception handling, `__TRY`, `__CATCH`, `__FINALLY` and `__ENDTRY` (specification 06 §15).
//!
//! The try statements run in a closure, so that a fault returns from it instead of from the body. A
//! `RETURN`, and an `EXIT` or a `CONTINUE` of a loop around the construct, cannot leave a closure, so
//! inside one they return their flow (1, 2 or 3) and the construct takes it after the `__FINALLY`
//! statements. A fault that `abi::exception_code` maps sets the `__CATCH` variable, clears the fault
//! record and runs the catch statements in a closure of their own; any other fault, and a fault of the
//! catch statements, goes on after the `__FINALLY` statements, which run inline.

use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::super::front::is_statement;
use super::super::NativeError;
use super::Lowerer;

/// The flow a `RETURN` returns from a closure of a `__TRY` construct.
const FLOW_RETURN: u8 = 1;
/// The flow an `EXIT` of a loop around the construct returns.
const FLOW_EXIT: u8 = 2;
/// The flow a `CONTINUE` of a loop around the construct returns.
const FLOW_CONTINUE: u8 = 3;

/// Which flows the statements of one closure returned.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TryFlows {
    returns: bool,
    exits: bool,
    continues: bool,
}

impl TryFlows {
    fn merge(&mut self, other: Self) {
        self.returns |= other.returns;
        self.exits |= other.exits;
        self.continues |= other.continues;
    }
}

impl Lowerer<'_, '_> {
    /// `__TRY ... [__CATCH(variable) ...] [__FINALLY ...] __ENDTRY`.
    pub(super) fn try_statement(&mut self, node: &SyntaxNode) -> Result<(), NativeError> {
        self.labels += 1;
        let flow = format!("flow{}", self.labels);
        self.line(&format!("let {flow}: R<u8> = (|| -> R<u8> {{"));
        let mut flows =
            self.flow_closure(node.children().filter(|child| is_statement(child.kind())))?;
        self.line("})();");
        if let Some(catch) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::CatchClause)
        {
            let Some(variable) = catch.children().find(|child| !is_statement(child.kind())) else {
                return Err(self.unsupported(&catch, "__CATCH without its variable"));
            };
            let place = self.place(&variable)?;
            self.line(&format!("let {flow}: R<u8> = match {flow} {{"));
            self.indent += 1;
            self.line("Err(fault) => match abi::exception_code(fault) {");
            self.indent += 1;
            self.line("Some(code) => {");
            self.indent += 1;
            self.line("cx.fault = abi::FaultRecord::default();");
            self.store_scalar(&place, "code");
            self.line("(|| -> R<u8> {");
            let caught =
                self.flow_closure(catch.children().filter(|child| is_statement(child.kind())))?;
            flows.merge(caught);
            self.line("})()");
            self.indent -= 1;
            self.line("}");
            self.line("None => Err(fault),");
            self.indent -= 1;
            self.line("},");
            self.line("flow => flow,");
            self.indent -= 1;
            self.line("};");
        }
        if let Some(finally) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::FinallyClause)
        {
            self.statements(finally.children())?;
        }
        self.line(&format!("match {flow}? {{"));
        self.indent += 1;
        if flows.returns {
            let exit = self.exit.clone();
            self.note_return();
            self.line(&format!("{FLOW_RETURN} => {{ {exit} }}"));
        }
        for (flow, used, exit) in [
            (FLOW_EXIT, flows.exits, true),
            (FLOW_CONTINUE, flows.continues, false),
        ] {
            if used {
                let Some(leave) = self.leave_loop(exit) else {
                    return Err(self.unsupported(node, "EXIT or CONTINUE outside a loop"));
                };
                self.line(&format!("{flow} => {{ {leave} }}"));
            }
        }
        self.line("_ => {}");
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    /// Lowers `statements` as the body of a closure of a `__TRY` construct, ending with its normal
    /// flow, 0; returns the flows they returned.
    fn flow_closure(
        &mut self,
        statements: impl Iterator<Item = SyntaxNode>,
    ) -> Result<TryFlows, NativeError> {
        let outer_loops = std::mem::take(&mut self.loops);
        let outer_exit = std::mem::replace(&mut self.exit, format!("return Ok({FLOW_RETURN});"));
        self.try_flows.push(TryFlows::default());
        self.indent += 1;
        let result = self.statements(statements);
        self.line("Ok(0)");
        self.indent -= 1;
        let flows = self.try_flows.pop().unwrap_or_default();
        self.loops = outer_loops;
        self.exit = outer_exit;
        result?;
        Ok(flows)
    }

    /// Notes a `RETURN` lowered to the body's exit: inside a closure of a `__TRY` construct the exit
    /// returns the flow, which the construct then takes.
    pub(super) fn note_return(&mut self) {
        if let Some(flows) = self.try_flows.last_mut() {
            flows.returns = true;
        }
    }

    /// The Rust that leaves the innermost loop, `EXIT` (`exit`) or its body, `CONTINUE`: a `break` of the
    /// loop's label, or inside a closure of a `__TRY` construct around which the loop lies, the return of
    /// the flow. `None` outside every loop.
    pub(super) fn leave_loop(&mut self, exit: bool) -> Option<String> {
        if let Some(labels) = self.loops.last() {
            let label = if exit { &labels.exit } else { &labels.next };
            return Some(format!("break {label};"));
        }
        let flows = self.try_flows.last_mut()?;
        if exit {
            flows.exits = true;
            Some(format!("return Ok({FLOW_EXIT});"))
        } else {
            flows.continues = true;
            Some(format!("return Ok({FLOW_CONTINUE});"))
        }
    }
}

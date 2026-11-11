//! A statement that holds a construct the generator does not compile yet (specification 44 NX-09):
//! the construct goes to the model's refusals, which fail the build at its end with every one of
//! them, and the lowering goes on with the next statement (specification 11 §6.8.1c).

use trust_syntax::syntax::SyntaxNode;

use super::Lowerer;
use crate::native::NativeError;

impl Lowerer<'_, '_> {
    /// Lowers the statement `node`. A construct in it that the generator does not compile yet goes
    /// to the model's refusals, and the lowering goes on with the next statement from the state
    /// before this one: its indentation, locals, loops, `__TRY` flows and held `FOR` variables.
    /// Any other error ends the lowering.
    pub(super) fn statement_or_refused(&mut self, node: &SyntaxNode) -> Result<(), NativeError> {
        let (indent, locals, loops, try_flows, held) = (
            self.indent,
            self.locals.len(),
            self.loops.len(),
            self.try_flows.len(),
            self.held.len(),
        );
        match self.statement(node) {
            Err(error) if error.is_unsupported() => {
                self.indent = indent;
                self.locals.truncate(locals);
                self.loops.truncate(loops);
                self.try_flows.truncate(try_flows);
                self.held.truncate(held);
                self.model.refused.push(error);
                Ok(())
            }
            lowered => lowered,
        }
    }
}

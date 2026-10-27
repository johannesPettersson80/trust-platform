//! setVariable handling: the variable a listing showed, named by the listing's handle and the name
//! it showed, written, forced or released on the connected runtime (specification 13,
//! "setVariable and setExpression").

use serde_json::Value;

use crate::protocol::{Request, SetVariableArguments};

use super::super::super::remote::writes::WriteTarget;
use super::super::super::{DebugAdapter, DispatchOutcome};
use super::parse_set_directive;

impl DebugAdapter {
    /// `setVariable`: the value text is an IEC literal of the variable's type, `force: X` forces
    /// it and `release` releases its force; while the debugger holds the scan the value goes into
    /// the held scan at once ([`DebugAdapter::write_variable`]).
    pub(in crate::adapter) fn handle_set_variable(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let Some(args) = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<SetVariableArguments>(value).ok())
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid setVariable args")],
                ..DispatchOutcome::default()
            };
        };
        let directive = match parse_set_directive(&args.value) {
            Ok(directive) => directive,
            Err(message) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, &message)],
                    ..DispatchOutcome::default()
                };
            }
        };
        if self.remote_session.is_none() {
            return self.not_connected(&request);
        }
        let target = WriteTarget::Listed {
            variables_reference: args.variables_reference,
            name: args.name,
        };
        self.write_variable(&request, &target, &directive, args.value.trim())
    }
}

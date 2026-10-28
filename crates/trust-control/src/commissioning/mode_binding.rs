//! The mode binding of a controller (specification 35, A1 and C0; `controllerModeBinding.ts`): the address through which a mode
//! signal is read and written. The address rule itself is `signals::controller_mode_address`, which V0 ported; this module gives
//! it the default of `controllerModeAddress`, the controller's own mode signal.

use crate::loop_definition::LoopDefinition;
use crate::signals::controller_mode_address;

/// The address of a mode signal (`controllerModeAddress(loop, mode)`): the declared BOOL signal that names the mode, or the mode
/// itself when it is a bit address such as `%MX0.2`. Without a `mode`, the controller's mode signal is used, as the TypeScript's
/// default does. An empty mode has no address.
#[must_use]
pub fn mode_address(loop_definition: &LoopDefinition, mode: Option<&str>) -> Option<String> {
    let mode = mode.or(loop_definition.controller.mode_signal.as_deref())?;
    if mode.is_empty() {
        return None;
    }
    controller_mode_address(loop_definition, mode)
}

#[cfg(test)]
#[path = "mode_binding_tests.rs"]
mod tests;

//! The channel shape of a cascade (specification 35, C6): the manipulated variable is the one plant input, and the two outputs are the
//! inner measurement, which has no setpoint, and the outer measurement, which has the setpoint. The gate serves such channels on a loop
//! whose program law is a cascade seed; any other channels of a cascade loop stay parked.

use super::ControllerProject;
use crate::loop_definition::LoopDefinition;

impl LoopDefinition {
    /// Whether the loop's channels have the shape of a cascade (C6), and its program law is a cascade seed. The plant input is the
    /// manipulated variable; of the two outputs, the one without a setpoint is the inner measurement and the one with a setpoint is the
    /// outer measurement, which is a process variable.
    #[must_use]
    pub(crate) fn is_cascade_channels(&self) -> bool {
        let Some(loop_channels) = &self.channels else {
            return false;
        };
        if self
            .controller
            .project
            .as_ref()
            .and_then(ControllerProject::cascade)
            .is_none()
        {
            return false;
        }
        let [plant_input] = loop_channels.inputs.as_slice() else {
            return false;
        };
        let [first, second] = loop_channels.outputs.as_slice() else {
            return false;
        };
        let outer = match (
            first.setpoint_signal_id.is_some(),
            second.setpoint_signal_id.is_some(),
        ) {
            (false, true) => second,
            (true, false) => first,
            _ => return false,
        };
        plant_input.signal_id == "manipulatedVariable" && outer.signal_id == "processVariable"
    }
}

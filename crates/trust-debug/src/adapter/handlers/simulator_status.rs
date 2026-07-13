//! Authoritative local simulator readiness.

use serde_json::Value;

use crate::protocol::{Request, SimulatorStatusResponseBody};

use super::super::{DebugAdapter, DispatchOutcome};

impl DebugAdapter {
    pub(in crate::adapter) fn handle_simulator_status(
        &self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let runner = self.runner.is_some();
        let control_server = self.control_server.is_some();
        let ready = self.simulator_launch_succeeded && runner && control_server;

        DispatchOutcome {
            responses: vec![self.ok_response(
                &request,
                Some(SimulatorStatusResponseBody {
                    ready,
                    runner,
                    control_server,
                }),
            )],
            ..DispatchOutcome::default()
        }
    }
}

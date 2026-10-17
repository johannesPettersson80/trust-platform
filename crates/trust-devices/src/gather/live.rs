//! Asking one target (design §1.5): the `status` probe, or a managed runtime's status from its
//! fleet root, and then the requests that a reachable runtime answers on one connection, each
//! with its own wait.

use trust_runtime::config::fleet_manifest::{
    managed_status, read_control_auth_token, runtime_project_path,
};
use trust_runtime_client::{
    probe, ControlClient, ControlEndpoint, ControlError, TargetProbe, RESPONSE_LINE_LIMIT,
};

use super::{Budget, LiveData, ManagedState, ManagedStatus, Reach, Reply};
use crate::request::Token;
use crate::targets::{ManagedTarget, Target, TargetOrigin};

/// Asks `target` for its status and, when its runtime answers, for its topology, its
/// connectors and its capabilities. The runtime of the project's endpoint and the simulator are
/// also asked for their catalog, and the selected runtime for its recent events.
pub(super) fn ask_target(target: &Target, budget: &Budget) -> (Reach, LiveData) {
    if let Some(managed) = &target.managed {
        return ask_managed(target, managed, budget);
    }
    let token = target.token.as_ref().map(Token::secret);
    let found = probe(&target.endpoint, target.enabled, token, budget.probe);
    let live = if matches!(found, TargetProbe::Reachable(_)) {
        read_live(target, token, budget)
    } else {
        LiveData::default()
    };
    (Reach::Probe(found), live)
}

/// A managed runtime answers through its fleet root, as `trust-runtime fleet runtime status`
/// reads it. A status that cannot be read is `Unavailable`, never `Stopped` (specification 25
/// §5.10).
fn ask_managed(target: &Target, managed: &ManagedTarget, budget: &Budget) -> (Reach, LiveData) {
    let status = match managed_status(&managed.fleet_root, &managed.runtime) {
        Ok(report) => ManagedStatus {
            state: ManagedState::from_status(Some(report.status.as_str())),
            report: Some(report),
            error: None,
        },
        Err(error) => ManagedStatus {
            state: ManagedState::Unavailable,
            report: None,
            error: Some(format!("{error:#}")),
        },
    };
    let live = if status.state == ManagedState::Running {
        let project = runtime_project_path(&managed.fleet_root, &managed.runtime.path);
        read_control_auth_token(&project).map_or_else(
            |_| LiveData::default(),
            |token| read_live(target, token.as_deref(), budget),
        )
    } else {
        LiveData::default()
    };
    (Reach::Managed(Box::new(status)), live)
}

fn read_live(target: &Target, token: Option<&str>, budget: &Budget) -> LiveData {
    let primary = matches!(
        target.origin,
        TargetOrigin::Project | TargetOrigin::Simulator
    );
    let mut connection = Connection {
        endpoint: &target.endpoint,
        token,
        budget,
        client: None,
    };
    let topology = connection.request("fleet.topology");
    let connectors = connection.request("connectors.status");
    let capabilities = connection.request("comm.capabilities");
    let schema = primary.then(|| connection.request("comm.schema"));
    let events = target.events.then(|| connection.request("events.tail"));
    LiveData {
        topology: Some(topology),
        connectors: Some(connectors),
        capabilities: Some(capabilities),
        schema,
        events,
    }
}

/// One connection for the requests after the status. After a request whose connection broke or
/// whose answer did not fit, the next request opens a new connection, because the old one may
/// still deliver the late answer.
struct Connection<'a> {
    endpoint: &'a str,
    token: Option<&'a str>,
    budget: &'a Budget,
    client: Option<ControlClient>,
}

impl Connection<'_> {
    fn request(&mut self, kind: &str) -> Reply {
        let mut client = if let Some(client) = self.client.take() {
            client
        } else {
            self.connect()?
        };
        let reply = client.request(kind, None);
        if !matches!(
            reply,
            Err(ControlError::Transport(_) | ControlError::Malformed(_))
        ) {
            self.client = Some(client);
        }
        reply
    }

    fn connect(&self) -> Result<ControlClient, ControlError> {
        let endpoint = ControlEndpoint::parse(self.endpoint).ok_or_else(|| {
            ControlError::Transport(format!("{} is not a control endpoint", self.endpoint))
        })?;
        let client = ControlClient::connect(&endpoint, self.token, self.budget.probe.connect)?
            .with_response_limit(RESPONSE_LINE_LIMIT);
        client.set_timeout(self.budget.request)?;
        Ok(client)
    }
}

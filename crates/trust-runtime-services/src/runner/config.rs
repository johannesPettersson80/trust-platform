//! `config.get` and `config.set` on a device without an operating system (specification 11, the
//! runtime configuration contract; specification 34 TARGET-07). The loop answers both from the
//! settings the platform keeps of the installed program's record, by the shared rules of
//! [`crate::settings`]: the keys every runtime has, and none of a computer's own services, which a
//! chip does not run. A request that passes every check changes the platform's settings before its
//! reply is written, as trust-runtime commits its settings before it replies; the scheduler's copies
//! of the watchdog, the fault policy and the retain save interval change at the program's next
//! boundary, where trust-runtime's scan thread takes them, also while it is paused or faulted. A
//! program that has ended takes none, as trust-runtime's closed scheduler refuses them.

use core::cell::Cell;

use serde_json::{json, Map, Value};
use trust_runtime_core::ports::{ControlLink, MonotonicClock, RunnerWait};

use super::host::RunnerHost;
use super::{ResourceCycle, Runner, RunnerPlatform};
use crate::chip::watchdog::cycle_timeout_ms;
use crate::protocol::{ControlRequest, ControlResponse};
use crate::resource::commands::{CommandSendError, SchedulerPolicies};
use crate::resource::replies::{no_installed_program, queue_refusal};
use crate::settings::values::config_value_error;
use crate::settings::{self, RuntimeSettings, StagedSettings, TokenChange};

/// A `config.set` that passed every check, which the platform takes before the reply is written.
pub(super) struct Commit {
    settings: RuntimeSettings,
    token: TokenChange,
}

impl Commit {
    /// The platform takes the settings and the token.
    pub(super) fn apply<P: RunnerPlatform>(self, platform: &mut P) {
        platform.commit_settings(self.settings, self.token);
    }
}

/// What a `config.set` leaves for the loop: the change the platform takes before the request's
/// reply is written, and the scheduler's copies the program takes at its next boundary.
#[derive(Default)]
pub(super) struct ConfigState {
    commit: Cell<Option<Commit>>,
    policies: Cell<Option<SchedulerPolicies>>,
}

impl ConfigState {
    /// The change of a `config.set` that every check passed, which the platform takes before the
    /// shell writes the request's reply.
    pub(super) fn take_commit(&self) -> Option<Commit> {
        self.commit.take()
    }

    /// The scheduler's copies a `config.set` left for the program's next boundary.
    pub(super) fn take_policies(&self) -> Option<SchedulerPolicies> {
        self.policies.take()
    }
}

impl<P: RunnerPlatform, C> RunnerHost<'_, P, C> {
    /// Answers `config.get` and `config.set` from the platform's settings; none for any other
    /// operation, and for both on a platform that keeps no settings and answers them itself.
    pub(super) fn answer_config(&self, request: &ControlRequest) -> Option<ControlResponse> {
        if !matches!(request.r#type.as_str(), "config.get" | "config.set") {
            return None;
        }
        let Some(settings) = self.platform.runtime_settings() else {
            // A chip without a program has no record to report; another platform answers itself.
            return self
                .program
                .missing_program()
                .map(|reason| no_installed_program(request.id, reason));
        };
        Some(if request.r#type == "config.get" {
            self.config_get(request.id, &settings)
        } else {
            self.config_set(request.id, request.params.as_ref(), settings)
        })
    }

    fn config_get(&self, id: u64, settings: &RuntimeSettings) -> ControlResponse {
        let Some(status) = self.platform.status_settings() else {
            return ControlResponse::error(id, "settings unavailable".into());
        };
        let mut out = Map::new();
        settings::project(
            settings,
            self.platform.control_token_length(),
            &status,
            &mut out,
        );
        ControlResponse::ok(id, Value::Object(out))
    }

    fn config_set(
        &self,
        id: u64,
        params: Option<&Value>,
        settings: RuntimeSettings,
    ) -> ControlResponse {
        let params = match params {
            None => return ControlResponse::error(id, "missing params".into()),
            Some(Value::Object(params)) => params,
            Some(_) => {
                return ControlResponse::error(
                    id,
                    "invalid config payload: params must be an object".into(),
                );
            }
        };
        let mut staged = StagedSettings::new(settings);
        // A device runs none of a computer's own services, so it has no key of theirs.
        let token_required = self.platform.control_token_required();
        if let Err(error) =
            settings::stage_request(params, &mut staged, token_required, |_, _, _| None)
        {
            return ControlResponse::error(id, error);
        }
        let timeout = cycle_timeout_ms(&staged.settings.watchdog);
        if timeout != cycle_timeout_ms(&settings.watchdog)
            && !self.platform.takes_cycle_timeout_ms(timeout)
        {
            return ControlResponse::error(
                id,
                config_value_error(
                    "watchdog.timeout_ms",
                    "the device's hardware watchdog does not take the interval this timeout needs",
                ),
            );
        }
        if let Some(policies) = staged.changed_policies() {
            if self.resource.ended() {
                return queue_refusal(id, CommandSendError::Closed, self);
            }
            self.config.policies.set(Some(policies));
        }
        self.config.commit.set(Some(Commit {
            settings: staged.settings,
            token: staged.token,
        }));
        ControlResponse::ok(
            id,
            json!({"updated": staged.updated, "restart_required": staged.restart_required}),
        )
    }
}

impl<L, W, Y, P, C> Runner<L, W, Y, P, C>
where
    L: ControlLink,
    W: RunnerWait,
    Y: ResourceCycle,
    P: RunnerPlatform,
    C: MonotonicClock + Clone,
{
    /// The program takes the scheduler's copies a `config.set` left, at its boundary.
    pub(super) fn take_policies(&mut self) {
        if let Some(policies) = self.shell.config.take_policies() {
            self.cycle.apply_policies(policies);
        }
    }
}

#[cfg(test)]
mod contract_tests;

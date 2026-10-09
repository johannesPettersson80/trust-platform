//! A pulse's end (specification 39, §12.6 and §21.5 item 1). The runtime owns it: the exchange
//! schedules the end when the pulse starts and writes it at the display boundaries after the
//! pulse's duration, trying again until it is written, whatever happens to the browser, the
//! request or the session. What the exchange tells about the end goes to the audit log (§12.7),
//! the session's value streams, the command records and the command's confirmation. It is told on
//! the runtime thread, so it only records and queues.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use serde_json::{json, Value as Json};

use super::{command_message, send_audit, CommandRequest, Commands};
use crate::control::ControlState;
use crate::hmi::access::sessions::SessionInfo;
use crate::hmi::commands::plan::Plan;
use crate::hmi::commands::registry::{CommandAnswer, CommandRegistry, Outcome};
use crate::hmi::commands::write::path_text;
use crate::hmi::exchange::{PulseEnd, PulseEndReport};
use crate::hmi::write_value::json_of;

/// A pulse's end as the command that started it records it.
struct PulseEndRecord {
    control_state: Arc<ControlState>,
    registry: Arc<CommandRegistry>,
    alarms: Arc<super::super::hmi_alarms::AlarmService>,
    session: SessionInfo,
    id: String,
    page: String,
    object: String,
    bundle: String,
    target: String,
    /// What the command asked, as the event log says it.
    message: String,
    /// The command's confirmation records its final outcome; otherwise the end does.
    confirmed: bool,
    /// Tells the confirmation that the end was written or replaced.
    ended: Sender<()>,
}

/// The report the exchange keeps with a local pulse's end, and how the command's confirmation
/// learns that the end was written or replaced.
pub(super) fn end_report(
    commands: &Commands,
    session: &SessionInfo,
    request: &CommandRequest,
    id: &str,
    plan: &Plan,
) -> (PulseEndReport, Receiver<()>) {
    let (ended, wait) = mpsc::channel();
    let mut record = PulseEndRecord {
        control_state: commands.control_state.clone(),
        registry: commands.registry.clone(),
        alarms: commands.alarms.clone(),
        session: session.clone(),
        id: id.to_string(),
        page: request.page.clone(),
        object: request.object.clone(),
        bundle: request.bundle.clone(),
        target: path_text(&plan.path),
        message: command_message(&request.action, Some(&plan.requested)),
        confirmed: plan.confirm.is_some(),
        ended,
    };
    (Box::new(move |end| record.tell(end)), wait)
}

impl PulseEndRecord {
    fn tell(&mut self, end: PulseEnd) {
        match end {
            PulseEnd::Written { written, attempts } => {
                let answer = CommandAnswer {
                    ended: true,
                    ..CommandAnswer::new(&self.id, Outcome::Applied)
                };
                self.audit(
                    &answer,
                    json_of(&written.old),
                    json_of(&written.readback),
                    attempts,
                );
                if !self.confirmed {
                    self.finished(Outcome::Applied, None);
                }
                self.registry.later(&self.session.id, answer);
                let _ = self.ended.send(());
            }
            PulseEnd::Retrying { reason } => {
                // Never assumed ended: the page and the log say the point may still be set.
                let answer = CommandAnswer {
                    reason: Some(format!(
                        "the pulse end is not written yet ({reason}); the runtime tries again at each display boundary"
                    )),
                    ..CommandAnswer::new(&self.id, Outcome::Unknown)
                };
                self.audit(&answer, None, None, 1);
                self.registry.later(&self.session.id, answer);
            }
            PulseEnd::Replaced { attempts } => {
                // The later command has its own outcome on the page; only the records say this.
                let reason = "a later command to the same point replaced the pulse end";
                let answer = CommandAnswer {
                    reason: Some(reason.to_string()),
                    ..CommandAnswer::new(&self.id, Outcome::Unknown)
                };
                self.audit(&answer, None, None, attempts);
                if !self.confirmed {
                    self.finished(Outcome::Applied, Some(reason.to_string()));
                }
                let _ = self.ended.send(());
            }
        }
    }

    /// The end's audit record (§12.7): the command's own record with the end's old, requested
    /// and read-back values and how many display boundaries tried it.
    fn audit(
        &self,
        answer: &CommandAnswer,
        old: Option<Json>,
        readback: Option<Json>,
        attempts: u32,
    ) {
        send_audit(
            &self.control_state,
            "hmi.command",
            Some(&self.session),
            answer,
            json!({
                "command": self.id,
                "page": self.page,
                "object": self.object,
                "target": self.target,
                "old": old,
                "requested": false,
                "readback": readback,
                "bundle": self.bundle,
                "pulse": "end",
                "attempts": attempts,
            }),
        );
    }

    /// The command's final outcome among the records (specification 40, §5.1).
    fn finished(&self, outcome: Outcome, reason: Option<String>) {
        self.alarms.command(
            &self.target,
            &self.message,
            Some(self.session.user.as_str()),
            outcome,
            reason,
        );
    }
}

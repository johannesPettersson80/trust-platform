//! Operator actions on alarms (specification 40, §4, §5.1): `POST /api/hmi/alarm`.
//!
//! Every request is checked in this order:
//! - an id and a session of a named user;
//! - the action and its value;
//! - an alarm of the running program;
//! - the action's role.
//!
//! The runtime thread then writes the instance's request field. The answer is refused, applied or
//! timed out; confirmation follows on the session's value streams, and every outcome goes to the
//! audit log.

use std::io::Read as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Value as Json};
use tiny_http::Request;

use super::super::hmi_command::{read_points, send_audit, write_point};
use super::super::hmi_operate::session_token;
use super::{respond, viewer, AlarmService, HmiAlarmContext};
use crate::control::ControlState;
use crate::hmi::access::policy::require_role;
use crate::hmi::access::sessions::SessionInfo;
use crate::hmi::alarms::{alarm_action, allows, AlarmAction, EventKind};
use crate::hmi::commands::plan::matches;
use crate::hmi::commands::registry::{Begin, CommandAnswer, CommandRegistry, Outcome};
use crate::hmi::commands::write::{path_text, Written};
use crate::hmi::member_paths::MemberPath;
use crate::hmi::write_value::json_of;

/// How long the instance may take to show an action applied.
const CONFIRM_WITHIN: Duration = Duration::from_secs(5);
/// How often a confirmation is looked for.
const POLL: Duration = Duration::from_millis(50);
/// The most actions followed at once for confirmation.
const MAX_FOLLOWED: usize = 64;
/// The longest command id.
const MAX_ID: usize = 128;
/// The largest request body read.
const MAX_BODY: u64 = 16 * 1024;

static FOLLOWED: AtomicUsize = AtomicUsize::new(0);

/// An operator action as the page sends it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AlarmActionRequest {
    id: String,
    alarm: String,
    action: String,
    #[serde(default)]
    value: Option<Json>,
}

/// An action the runtime wrote.
struct Applied {
    alarm: String,
    action: AlarmAction,
    target: MemberPath,
    written: Written,
}

/// A refused action: its answer, and its target once it is known (for the audit log).
struct Refused {
    target: Option<MemberPath>,
    answer: CommandAnswer,
}

/// `POST /api/hmi/alarm`: the session's activity, then the action's answer.
pub(super) fn act(mut request: Request, ctx: &HmiAlarmContext<'_>) {
    if let Err(error) = viewer(&request, ctx) {
        let _ = request.respond(super::super::auth_helpers::auth_error_response(error));
        return;
    }
    let session = session_token(&request).and_then(|token| ctx.sessions.touch(&token));
    let mut body = String::new();
    let parsed = request
        .as_reader()
        .take(MAX_BODY)
        .read_to_string(&mut body)
        .ok()
        .and_then(|_| serde_json::from_str::<AlarmActionRequest>(&body).ok());
    let Some(action) = parsed else {
        return respond(
            request,
            400,
            &json!({ "ok": false, "error": "send { \"id\", \"alarm\", \"action\", \"value\"? }" }),
        );
    };
    let actions = Actions {
        registry: ctx.registry.clone(),
        control_state: ctx.control_state.clone(),
        alarms: ctx.alarms.clone(),
    };
    let answer = actions.execute(action, session);
    respond(
        request,
        200,
        &serde_json::to_value(answer).unwrap_or(Json::Null),
    );
}

#[derive(Clone)]
struct Actions {
    registry: Arc<CommandRegistry>,
    control_state: Arc<ControlState>,
    alarms: Arc<AlarmService>,
}

impl Actions {
    /// Executes a request for a session (or none) and gives the first answer.
    fn execute(&self, request: AlarmActionRequest, session: Option<SessionInfo>) -> CommandAnswer {
        let id = request.id.trim().to_string();
        if id.is_empty() || id.len() > MAX_ID {
            return CommandAnswer::refused(
                &request.id,
                "a command needs an id of 1 to 128 characters",
            );
        }
        let Some(session) = session else {
            let answer = CommandAnswer::refused(&id, "log in to send commands");
            self.audit(None, &request, None, None, &answer);
            self.finished(None, &request, None, &answer);
            return answer;
        };
        if let Begin::Repeat(answer) = self.registry.begin(&id) {
            return answer;
        }
        let (answer, applied) = match self.decide(&request, &session, &id) {
            Ok(applied) => {
                let answer = CommandAnswer {
                    readback: json_of(&applied.written.readback),
                    ..CommandAnswer::new(&id, Outcome::Applied)
                };
                // A comment shows in no read, so it is recorded once it is written; the other
                // actions waited as pending before their write (see `decide`).
                if applied.action.kind == EventKind::Comment {
                    self.alarms.operator_action(
                        &applied.alarm,
                        applied.action.kind,
                        &session.user,
                        applied.action.detail.clone(),
                        applied.action.seconds,
                    );
                }
                self.audit(
                    Some(&session),
                    &request,
                    Some(&applied.target),
                    json_of(&applied.written.old),
                    &answer,
                );
                (answer, Some(applied))
            }
            Err(refused) => {
                let Refused { target, answer } = *refused;
                self.audit(Some(&session), &request, target.as_ref(), None, &answer);
                self.finished(Some(&session), &request, None, &answer);
                (answer, None)
            }
        };
        self.registry.answer(&answer);
        if let Some(applied) = applied {
            self.follow(session, request, applied);
        }
        answer
    }

    /// The checks of §5.1 and the write, or the refusal.
    fn decide(
        &self,
        request: &AlarmActionRequest,
        session: &SessionInfo,
        id: &str,
    ) -> Result<Applied, Box<Refused>> {
        let refuse = |target: Option<MemberPath>, reason: String| {
            Box::new(Refused {
                target,
                answer: CommandAnswer::refused(id, reason),
            })
        };
        let action = alarm_action(request.action.trim(), request.value.as_ref())
            .map_err(|reason| refuse(None, reason))?;
        let named = request.alarm.trim();
        let alarm = self
            .alarms
            .alarm_path(named)
            .ok_or_else(|| refuse(None, format!("the program has no alarm \"{named}\"")))?;
        require_role(action.role, session.role).map_err(|reason| refuse(None, reason))?;
        // The same rule that decides what the display offers (specification 40, §4).
        let allowed = self
            .alarms
            .alarm_status(&alarm)
            .is_some_and(|status| allows(&status, session.role, request.action.trim()));
        if !allowed {
            return Err(refuse(
                None,
                format!(
                    "the alarm's state does not allow \"{}\" now",
                    request.action.trim()
                ),
            ));
        }
        let target = MemberPath::parse(&format!("{alarm}.Request.{}", action.field))
            .map_err(|reason| refuse(None, reason))?;
        // The read that shows the transition records it, and that read can come at the end of
        // the scan that applies the write: the action waits as pending before the write, so that
        // the record names the user (specification 40, §5.1). A write the runtime refuses or does
        // not apply in time withdraws it, so that no later transition is recorded as the user's.
        let transition = action.kind != EventKind::Comment;
        if transition {
            self.alarms.operator_action(
                &alarm,
                action.kind,
                &session.user,
                action.detail.clone(),
                action.seconds,
            );
        }
        let refused = |answer: CommandAnswer| {
            if transition {
                self.alarms
                    .withdraw_operator_action(&alarm, action.kind, &session.user);
            }
            Box::new(Refused {
                target: Some(target.clone()),
                answer,
            })
        };
        match write_point(&self.control_state, &target, action.write.clone()) {
            Some(Ok(written)) => Ok(Applied {
                alarm,
                action,
                target,
                written,
            }),
            Some(Err(refused_write)) => {
                Err(refused(CommandAnswer::write_refused(id, refused_write)))
            }
            None => Err(refused(CommandAnswer {
                reason: Some("the runtime did not apply it in time".to_string()),
                ..CommandAnswer::new(id, Outcome::TimedOut)
            })),
        }
    }

    /// After an applied action: confirmation by the instance's state, on a thread of its own.
    fn follow(&self, session: SessionInfo, request: AlarmActionRequest, applied: Applied) {
        if FOLLOWED
            .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_FOLLOWED).then_some(count + 1)
            })
            .is_err()
        {
            let answer = CommandAnswer::new(request.id.trim(), Outcome::Applied);
            self.finished(Some(&session), &request, Some(&applied), &answer);
            return;
        }
        let actions = self.clone();
        thread::spawn(move || {
            let id = request.id.trim().to_string();
            let started = Instant::now();
            let (member, expected) = &applied.action.confirm;
            let point = MemberPath::parse(&format!("{}.{member}", applied.alarm)).ok();
            let mut last = None;
            let confirmed = loop {
                last = point
                    .as_ref()
                    .and_then(|point| {
                        read_points(&actions.control_state, std::slice::from_ref(point))
                            .into_iter()
                            .next()
                            .flatten()
                    })
                    .or(last);
                // The first look waits a scan, so the instance has applied the request.
                let seen = started.elapsed() >= POLL;
                if seen && last.as_ref().is_some_and(|read| matches(read, expected)) {
                    break true;
                }
                if started.elapsed() >= CONFIRM_WITHIN {
                    break false;
                }
                thread::sleep(POLL);
            };
            let answer = CommandAnswer {
                readback: last,
                reason: (!confirmed).then(|| "not confirmed in time".to_string()),
                ..CommandAnswer::new(
                    &id,
                    if confirmed {
                        Outcome::Confirmed
                    } else {
                        Outcome::TimedOut
                    },
                )
            };
            actions.audit(
                Some(&session),
                &request,
                Some(&applied.target),
                json_of(&applied.written.old),
                &answer,
            );
            actions.finished(Some(&session), &request, Some(&applied), &answer);
            actions.registry.later(&session.id, answer);
            FOLLOWED.fetch_sub(1, Ordering::AcqRel);
        });
    }

    /// The action's final outcome among the records (§5.1): under the alarm's path, with the
    /// action's name.
    fn finished(
        &self,
        session: Option<&SessionInfo>,
        request: &AlarmActionRequest,
        applied: Option<&Applied>,
        answer: &CommandAnswer,
    ) {
        let alarm = applied.map_or_else(
            || {
                self.alarms
                    .alarm_path(request.alarm.trim())
                    .unwrap_or_else(|| request.alarm.trim().to_string())
            },
            |applied| applied.alarm.clone(),
        );
        let name = applied
            .map(|applied| applied.action.name)
            .or_else(|| {
                alarm_action(request.action.trim(), request.value.as_ref())
                    .ok()
                    .map(|action| action.name)
            })
            .unwrap_or(request.action.trim());
        let message = match applied.and_then(|applied| applied.action.detail.as_deref()) {
            Some(seconds) if applied.is_some_and(|applied| applied.action.field == "Shelve") => {
                format!("{name} for {seconds}")
            }
            _ => name.to_string(),
        };
        self.alarms.command(
            &alarm,
            &message,
            session.map(|session| session.user.as_str()),
            answer.outcome,
            answer.reason.clone(),
        );
    }

    /// The audit record (§5.1): the alarm, the action and its value, with the command's target
    /// and the value it replaced.
    fn audit(
        &self,
        session: Option<&SessionInfo>,
        request: &AlarmActionRequest,
        target: Option<&MemberPath>,
        old: Option<Json>,
        answer: &CommandAnswer,
    ) {
        send_audit(
            &self.control_state,
            "hmi.alarm",
            session,
            answer,
            json!({
                "command": answer.id,
                "alarm": request.alarm.trim(),
                "action": request.action.trim(),
                "value": request.value,
                "target": target.map(path_text),
                "old": old,
                "readback": answer.readback,
            }),
        );
    }
}

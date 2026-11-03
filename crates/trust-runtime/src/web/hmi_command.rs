//! HMI commands over the web (specification 39, §12.2, §12.5–12.8): `POST /api/hmi/command`.
//!
//! Every request is checked in this order:
//! - a session of a named user;
//! - the page version;
//! - the control on the active page;
//! - the permission (the policy narrowed by the control) for the user's role and the limits;
//! - the interlock.
//!
//! The runtime thread then writes the point and reads it back. The answer is refused, applied or
//! timed out. Confirmation follows on the session's value streams, and every outcome goes to the
//! audit log. A pulse's end belongs to the runtime from the moment the pulse starts (§21.5 item 1).
//! A control that calls a method sends `invoke` (§21.2, `capability`); the Settings dialog has its
//! own route (§21.4, `settings`).

mod capability;
mod pulse;
pub(super) mod settings;

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Value as Json};
use smol_str::SmolStr;

use super::hmi_display::files::HmiFiles;
use crate::control::{ControlAuditEvent, ControlState};
use crate::hmi::access::config::HmiAccessConfig;
use crate::hmi::access::policy::{authorize, effective_permission, within_limits};
use crate::hmi::access::sessions::SessionInfo;
use crate::hmi::catalogue::Catalogue;
use crate::hmi::commands::plan::{matches, plan, Expected, Plan};
use crate::hmi::commands::registry::{Begin, CommandAnswer, CommandRegistry, Outcome};
use crate::hmi::commands::write::{path_text, WriteAction, WriteRefused, Written};
use crate::hmi::member_paths::MemberPath;
use crate::hmi::write_value::json_of;

/// How long the runtime thread may take to write a command.
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
/// How often a confirmation is looked for.
const POLL: Duration = Duration::from_millis(50);
/// The most commands followed at once for confirmation or a remote pulse's end.
const MAX_FOLLOWED: usize = 64;
/// The longest command id.
const MAX_ID: usize = 128;

static FOLLOWED: AtomicUsize = AtomicUsize::new(0);
static AUDIT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// A command as the page sends it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CommandRequest {
    id: String,
    bundle: String,
    page: String,
    object: String,
    action: String,
    #[serde(default)]
    value: Option<Json>,
    /// The value the control showed when it sent the command (specification 39 §5.8): `null`
    /// while it showed none; absent when the sender does not say.
    #[serde(default, deserialize_with = "present")]
    shown: Option<Json>,
}

/// A member that is present, `null` included: absent is `None` (the member's default), `null`
/// is `Some(Json::Null)`.
fn present<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Json>, D::Error> {
    Json::deserialize(deserializer).map(Some)
}

/// A refused command: its answer, and its plan once it is known (for the audit log).
struct Refused {
    plan: Option<Plan>,
    answer: CommandAnswer,
}

/// An applied command: its plan, the point before and after, and for a local pulse how its
/// confirmation learns that the runtime wrote the end.
struct Applied {
    plan: Plan,
    written: Written,
    ended: Option<Receiver<()>>,
}

/// What a command needs from the runtime.
pub(super) struct Commands {
    pub(super) registry: Arc<CommandRegistry>,
    pub(super) access: Arc<HmiAccessConfig>,
    pub(super) control_state: Arc<ControlState>,
    /// The bundles; the page and its version are read under their lock (§16.4).
    pub(super) store: Arc<crate::hmi::bundle::store::BundleStore>,
    /// The records, which keep each command's final outcome (specification 40, §5.1).
    pub(super) alarms: Arc<super::hmi_alarms::AlarmService>,
}

impl Commands {
    /// Executes a request for a session (or none) and gives the first answer.
    pub(super) fn execute(
        &self,
        request: CommandRequest,
        session: Option<SessionInfo>,
    ) -> CommandAnswer {
        let id = request.id.trim().to_string();
        if id.is_empty() || id.len() > MAX_ID {
            return CommandAnswer::refused(
                &request.id,
                "a command needs an id of 1 to 128 characters",
            );
        }
        let Some(session) = session else {
            let answer = CommandAnswer::refused(&id, "log in to send commands");
            self.audit(None, &id, None, None, &request, &answer, &request.bundle);
            self.finished(None, &request, None, &answer);
            return answer;
        };
        if request.action == "invoke" {
            return self.execute_capability(request, session);
        }
        if let Begin::Repeat(answer) = self.registry.begin(&id) {
            return answer;
        }
        let (answer, followed) = match self.decide(&request, &session, &id) {
            Ok(applied) => {
                let answer = CommandAnswer {
                    readback: json_of(&applied.written.readback),
                    ..CommandAnswer::new(&id, Outcome::Applied)
                };
                self.audit(
                    Some(&session),
                    &id,
                    Some(&applied.plan),
                    json_of(&applied.written.old),
                    &request,
                    &answer,
                    &request.bundle,
                );
                (answer, Some(applied))
            }
            Err(refused) => {
                let Refused { plan, answer } = *refused;
                self.audit(
                    Some(&session),
                    &id,
                    plan.as_ref(),
                    None,
                    &request,
                    &answer,
                    &request.bundle,
                );
                self.finished(Some(&session), &request, plan.as_ref(), &answer);
                (answer, None)
            }
        };
        self.registry.answer(&answer);
        if let Some(applied) = followed {
            self.follow(session, request, applied);
        }
        answer
    }

    /// The checks of §12.2 and the write, or the refusal (with the plan once it is known).
    fn decide(
        &self,
        request: &CommandRequest,
        session: &SessionInfo,
        id: &str,
    ) -> Result<Applied, Box<Refused>> {
        let refuse = |plan: Option<Plan>, reason: String| {
            Box::new(Refused {
                plan,
                answer: CommandAnswer::refused(id, reason),
            })
        };
        let control_state = self.control_state.clone();
        let type_of = move |path: &str| instance_type(&control_state, path);
        let (version, page) = self.store.read(|root| {
            let files = HmiFiles::new(root.to_path_buf());
            (
                files.version(),
                files.page_or_popup_with(&request.page, &type_of),
            )
        });
        if request.bundle != version {
            return Err(refuse(
                None,
                "the page is out of date; reload it".to_string(),
            ));
        }
        let page = match page {
            Some(Ok(page)) => page,
            _ => {
                return Err(refuse(
                    None,
                    format!("the project has no page \"{}\" that loads", request.page),
                ))
            }
        };
        let Some(object) = page
            .objects
            .iter()
            .find(|object| object.id == request.object)
        else {
            return Err(refuse(
                None,
                format!("the page has no object \"{}\"", request.object),
            ));
        };
        let Some(component) = Catalogue::shipped().component(&object.kind) else {
            return Err(refuse(None, format!("\"{}\" sends no command", object.id)));
        };
        let plan = plan(
            object,
            component,
            &request.action,
            request.value.as_ref(),
            request.shown.as_ref(),
        )
        .map_err(|reason| refuse(None, reason))?;
        let permission = effective_permission(&self.access.write, &plan.path, object)
            .map_err(|reason| refuse(Some(plan.clone()), reason))?;
        authorize(&permission, session.role)
            .map_err(|reason| refuse(Some(plan.clone()), reason))?;
        if let Some(number) = plan.number {
            within_limits(&permission, number)
                .map_err(|reason| refuse(Some(plan.clone()), reason))?;
        }
        if let Some((permit, reason)) = &plan.interlock
            && let Some(refusal) = self.interlocked(permit, reason.as_ref())
        {
            return Err(refuse(Some(plan.clone()), refusal));
        }
        let mut ended = None;
        match if crate::hmi::remote_values::remote_path(&plan.path).is_some() {
            self.control_state
                .resource
                .hmi_exchange()
                .remote_operations
                .write(session, &plan.path, &plan.action, id, plan.pulse)
        } else if let Some(pulse) = plan.pulse {
            // The end is scheduled with the pulse's start, in the runtime (§21.5 item 1).
            let (report, wait) = pulse::end_report(self, session, request, id, &plan);
            ended = Some(wait);
            crate::control::hmi_ports::hmi_exchange(&self.control_state).write_pulse(
                plan.path.clone(),
                plan.action.clone(),
                WRITE_TIMEOUT,
                pulse,
                report,
            )
        } else {
            self.write(&plan.path, plan.action.clone())
        } {
            Some(Ok(written)) => Ok(Applied {
                plan,
                written,
                ended,
            }),
            Some(Err(refused)) => Err(Box::new(Refused {
                plan: Some(plan),
                answer: CommandAnswer::write_refused(id, refused),
            })),
            None => Err(Box::new(Refused {
                plan: Some(plan),
                answer: CommandAnswer {
                    reason: Some("the runtime did not apply it in time".to_string()),
                    ..CommandAnswer::new(id, Outcome::TimedOut)
                },
            })),
        }
    }

    /// After an applied command: a remote pulse's observed end, then confirmation, on a thread of
    /// its own. A local pulse's end is not followed here: the runtime scheduled it with the
    /// pulse's start and tells it through [`pulse::end_report`]. A command that is not followed is
    /// final as applied.
    fn follow(&self, session: SessionInfo, request: CommandRequest, applied: Applied) {
        let Applied {
            plan,
            written,
            ended,
        } = applied;
        let id = request.id.trim().to_string();
        let remote = crate::hmi::remote_values::remote_path(&plan.path).is_some();
        if plan.confirm.is_none() && !(remote && plan.pulse.is_some()) {
            // A local pulse's final outcome is recorded when the runtime has written its end.
            if plan.pulse.is_none() {
                let applied = CommandAnswer::new(&id, Outcome::Applied);
                self.finished(Some(&session), &request, Some(&plan), &applied);
            }
            return;
        }
        if FOLLOWED
            .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_FOLLOWED).then_some(count + 1)
            })
            .is_err()
        {
            // Not followed: final as applied. A local pulse still ends; its end is the runtime's.
            let applied = CommandAnswer::new(&id, Outcome::Applied);
            self.finished(Some(&session), &request, Some(&plan), &applied);
            return;
        }
        let commands = Commands {
            registry: self.registry.clone(),
            access: self.access.clone(),
            control_state: self.control_state.clone(),
            store: self.store.clone(),
            alarms: self.alarms.clone(),
        };
        thread::spawn(move || {
            let id = request.id.trim().to_string();
            let started = Instant::now();
            match (plan.pulse, &ended) {
                (Some(pulse), _) if remote => {
                    thread::sleep(pulse);
                    // The owning runtime ends the pulse; this one observes the end.
                    let reset = commands.observe(&plan.path, &session) == Some(json!(false));
                    let mut ended = CommandAnswer::new(
                        &id,
                        if reset {
                            Outcome::Applied
                        } else {
                            Outcome::Unknown
                        },
                    );
                    ended.ended = reset;
                    if !reset {
                        ended.reason = Some("the owner's pulse end could not be observed".into());
                    }
                    if plan.confirm.is_none() {
                        commands.finished(Some(&session), &request, Some(&plan), &ended);
                    }
                    commands.registry.later(&session.id, ended);
                }
                (Some(pulse), Some(ended)) => {
                    // The runtime writes the end (§21.5 item 1); the page is told it before the
                    // confirmation, which waits for it at most until the pulse and the
                    // confirmation time have passed and the runtime had time to write.
                    let wait = pulse
                        .max(plan.timeout)
                        .saturating_add(WRITE_TIMEOUT)
                        .saturating_sub(started.elapsed());
                    let _ = ended.recv_timeout(wait);
                }
                _ => {}
            }
            if let Some((point, expected)) = &plan.confirm {
                let expected = match expected {
                    Expected::Value(value) => Some(value.clone()),
                    Expected::Readback => json_of(&written.readback),
                };
                let deadline = started + plan.timeout;
                let mut last = None;
                let confirmed = loop {
                    last = commands.observe(point, &session).or(last);
                    // The first look waits a scan, so a value the program overrides is seen.
                    let seen = started.elapsed() >= POLL;
                    if seen
                        && last
                            .as_ref()
                            .zip(expected.as_ref())
                            .is_some_and(|(read, wanted)| matches(read, wanted))
                    {
                        break true;
                    }
                    if Instant::now() >= deadline {
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
                commands.audit(
                    Some(&session),
                    &id,
                    Some(&plan),
                    json_of(&written.old),
                    &request,
                    &answer,
                    &request.bundle,
                );
                commands.finished(Some(&session), &request, Some(&plan), &answer);
                commands.registry.later(&session.id, answer);
            }
            FOLLOWED.fetch_sub(1, Ordering::AcqRel);
        });
    }

    /// The command's final outcome among the records (specification 40, §5.1).
    fn finished(
        &self,
        session: Option<&SessionInfo>,
        request: &CommandRequest,
        plan: Option<&Plan>,
        answer: &CommandAnswer,
    ) {
        let target = plan.map_or_else(|| request.object.clone(), |plan| path_text(&plan.path));
        let asked = plan.map(|plan| &plan.requested).or(request.value.as_ref());
        self.alarms.command(
            &target,
            &command_message(&request.action, asked),
            session.map(|session| session.user.as_str()),
            answer.outcome,
            answer.reason.clone(),
        );
    }

    /// Why a control's interlock refuses a command now, `None` while its permit reads TRUE: the
    /// `interlock-reason` text itself, which the display shows as "Not accepted: <reason>"
    /// (§12.6, §12.8 item 5, §21.2 items 2 and 3).
    fn interlocked(&self, permit: &MemberPath, reason: Option<&MemberPath>) -> Option<String> {
        let mut paths = vec![permit.clone()];
        paths.extend(reason.cloned());
        let values = self.read(&paths);
        if values.first().and_then(|value| value.as_ref()) == Some(&json!(true)) {
            return None;
        }
        let why = values
            .get(1)
            .and_then(|value| value.as_ref())
            .and_then(Json::as_str)
            .filter(|text| !text.is_empty())
            .unwrap_or("the interlock does not permit it");
        Some(why.to_string())
    }

    fn write(
        &self,
        path: &MemberPath,
        action: WriteAction,
    ) -> Option<Result<Written, WriteRefused>> {
        write_point(&self.control_state, path, action)
    }

    fn observe(&self, path: &MemberPath, session: &SessionInfo) -> Option<Json> {
        if crate::hmi::remote_values::remote_path(path).is_some() {
            self.control_state
                .resource
                .hmi_exchange()
                .remote_operations
                .observe(session, path)
        } else {
            self.read(std::slice::from_ref(path))
                .into_iter()
                .next()
                .flatten()
        }
    }

    fn read(&self, paths: &[MemberPath]) -> Vec<Option<Json>> {
        read_points(&self.control_state, paths)
    }

    /// An audit record (§12.7): time, user and session, command id, target, old and requested
    /// value, outcome and the page version.
    #[allow(clippy::too_many_arguments)]
    fn audit(
        &self,
        session: Option<&SessionInfo>,
        id: &str,
        plan: Option<&Plan>,
        old: Option<Json>,
        request: &CommandRequest,
        answer: &CommandAnswer,
        bundle: &str,
    ) {
        send_audit(
            &self.control_state,
            "hmi.command",
            session,
            answer,
            json!({
                "command": id,
                "page": request.page,
                "object": request.object,
                "target": plan.map(|plan| path_text(&plan.path)),
                "old": old,
                "requested": plan.map_or_else(|| request.value.clone().unwrap_or(Json::Null), |plan| plan.requested.clone()),
                "readback": answer.readback,
                "bundle": bundle,
            }),
        );
    }
}

/// What a command asked, as the event log says it: `Set`, `Pulse`, or `Write 42`.
fn command_message(action: &str, asked: Option<&Json>) -> String {
    let mut name = action.trim().to_string();
    if let Some(first) = name.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    match asked {
        Some(Json::String(text)) if action == "write" => format!("{name} {text}"),
        Some(value) if action == "write" && !value.is_null() => format!("{name} {value}"),
        _ => name,
    }
}

/// Writes a point at the program's next `Hmi.Display` call; `None` when no call took it in time,
/// and then it is withdrawn, never applied later (specification 39, §18 Phase 5).
pub(super) fn write_point(
    control_state: &ControlState,
    path: &MemberPath,
    action: WriteAction,
) -> Option<Result<Written, WriteRefused>> {
    control_state
        .resource
        .hmi_exchange()
        .write(path.clone(), action, WRITE_TIMEOUT)
}

/// The type of the function block instance a path names, or `None` (specification 39, §8.7).
pub(super) fn instance_type(control_state: &ControlState, path: &str) -> Option<String> {
    let path = MemberPath::parse(path).ok()?;
    control_state
        .resource
        .hmi_exchange()
        .read_next(&[path], WRITE_TIMEOUT)?
        .instance_types
        .into_iter()
        .next()
        .flatten()
}

/// Plain JSON values of points, `None` where unknown.
pub(super) fn read_points(control_state: &ControlState, paths: &[MemberPath]) -> Vec<Option<Json>> {
    let Some(read) = control_state
        .resource
        .hmi_exchange()
        .read_next(paths, WRITE_TIMEOUT)
    else {
        return vec![None; paths.len()];
    };
    read.values
        .into_iter()
        .map(|value| {
            value
                .filter(|value| value.stale_ms.is_none())
                .and_then(|value| serde_json::to_value(value).ok())
                .and_then(|wire| wire.get("value").cloned())
        })
        .collect()
}

/// An audit record (§12.7) of a command's outcome: the time, the user and session, the outcome
/// and its reason, with the command's own `details`.
pub(super) fn send_audit(
    control_state: &ControlState,
    request_type: &str,
    session: Option<&SessionInfo>,
    answer: &CommandAnswer,
    details: Json,
) {
    let _ = submit_audit(control_state, request_type, session, answer, details);
}

/// Whether the existing audit sink accepted a record; this is not a disk-save acknowledgement.
pub(super) fn submit_audit(
    control_state: &ControlState,
    request_type: &str,
    session: Option<&SessionInfo>,
    answer: &CommandAnswer,
    mut details: Json,
) -> bool {
    let Some(sender) = &control_state.audit_tx else {
        return false;
    };
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let sequence = AUDIT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    if let Some(fields) = details.as_object_mut() {
        fields.insert(
            "time_ms".to_string(),
            json!(u64::try_from(timestamp_ms).unwrap_or(u64::MAX)),
        );
        fields.insert(
            "user".to_string(),
            json!(session.map(|session| session.user.as_str())),
        );
        fields.insert(
            "session".to_string(),
            json!(session.map(|session| session.id.as_str())),
        );
        fields.insert(
            "outcome".to_string(),
            serde_json::to_value(answer.outcome).unwrap_or(Json::Null),
        );
        fields.insert("reason".to_string(), json!(answer.reason));
    }
    sender
        .send(ControlAuditEvent {
            event_id: SmolStr::new(format!("hmi-{timestamp_ms}-{sequence}")),
            timestamp_ms,
            request_id: sequence,
            request_type: SmolStr::new(request_type),
            correlation_id: Some(SmolStr::new(answer.id.as_str())),
            ok: matches!(answer.outcome, Outcome::Applied | Outcome::Confirmed),
            error: answer.reason.as_deref().map(SmolStr::new),
            auth_present: session.is_some(),
            client: None,
            details: Some(details),
        })
        .is_ok()
}

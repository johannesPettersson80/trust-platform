//! Method commands through the HMI command endpoint (specification 39, §21.2, §21.5 item 2):
//! `action = "invoke"`. The runtime finds the command from the active page, never from the
//! request, and checks the session's role, the control's interlock, and the instance's interface
//! and the inputs against the running program's image; only then does it record the command for
//! its retries and call the method at the next display boundary. The method's BOOL result is the
//! outcome: TRUE accepted, FALSE refused with the instance's `CommandText` as the reason.
use super::*;
use crate::hmi::access::policy::{command_role, own_limits, require_role};
use crate::hmi::commands::{
    capability::MethodCommand, capability_checks, plan::interlock, program_checks,
    registry::ScopedBegin,
};
use crate::hmi::member_paths::PathSegment;
use crate::hmi::method_exchange::{Invocation, Reply};
use crate::hmi::page_files::HmiObject;
use crate::value::Value;

/// The member a refusal's reason is read from (§21.2 item 3).
const REFUSAL_TEXT: &str = "CommandText";
/// What a refusal says when the instance gives no reason.
const NOT_ACCEPTED: &str = "not accepted";

/// A method command that passed every check, ready to be called.
struct Authorized {
    object: HmiObject,
    command: MethodCommand,
    path: MemberPath,
    inputs: Vec<(SmolStr, Value)>,
    generation: u64,
}

impl Commands {
    pub(super) fn execute_capability(
        &self,
        request: CommandRequest,
        session: SessionInfo,
    ) -> CommandAnswer {
        let id = request.id.trim().to_string();
        let payload = json!({
            "bundle": request.bundle,
            "page": request.page,
            "object": request.object,
            "value": request.value,
        })
        .to_string();
        // A retry gets the first answer; nothing is recorded before the command is authorized.
        if let Some(answer) = self.registry.replay_scoped(&session.user, &id, &payload) {
            return answer;
        }
        let authorized = match self.authorize_capability(&request, &session) {
            Ok(authorized) => authorized,
            Err(refused) => {
                let answer = CommandAnswer::write_refused(&id, refused);
                self.capability_audit(&request, &session, None, &answer);
                return answer;
            }
        };
        let ticket = match self.registry.begin_scoped(&session.user, &id, payload) {
            ScopedBegin::New(ticket) => ticket,
            ScopedBegin::Answer(answer) => return answer,
        };
        let answer = self.call(&id, &authorized);
        self.capability_audit(&request, &session, Some(&authorized.command), &answer);
        self.registry.answer_scoped(&ticket, answer.clone());
        if answer.outcome == Outcome::Applied {
            self.follow_capability(session, request, authorized);
        }
        answer
    }

    /// The checks of §21.2 item 2 in order: the control on the active page and its command, the
    /// session's role, the interlock, the instance's interface and method, and the inputs. A
    /// refusal is a [`WriteRefused`]: its reason, and the stable code of an input's text longer
    /// than the input holds (§12.8 item 5).
    fn authorize_capability(
        &self,
        request: &CommandRequest,
        session: &SessionInfo,
    ) -> Result<Authorized, WriteRefused> {
        let exchange = crate::control::hmi_ports::hmi_exchange(&self.control_state);
        let state = self.control_state.clone();
        let type_of = move |path: &str| instance_type(&state, path);
        let (version, page, generation) = self.store.read(|root| {
            let files = HmiFiles::new(root.to_path_buf());
            (
                files.version(),
                files.page_or_popup_with(&request.page, &type_of),
                exchange.methods.generation(),
            )
        });
        if version != request.bundle {
            return Err("the page is out of date; reload it".into());
        }
        let Some(Ok(page)) = page else {
            return Err(format!("the project has no page \"{}\" that loads", request.page).into());
        };
        let object = page
            .objects
            .into_iter()
            .find(|object| object.id == request.object)
            .ok_or_else(|| format!("the page has no object \"{}\"", request.object))?;
        let command = object
            .command
            .clone()
            .ok_or_else(|| format!("\"{}\" calls no method", object.id))?;
        command.validate()?;
        let path = MemberPath::parse(&command.instance)?;
        if crate::hmi::remote_values::remote_path(&path).is_some() {
            return Err("open this equipment's own runtime to send its commands".into());
        }
        require_role(
            command_role(&self.access.write, &path, &object),
            session.role,
        )?;
        if let Some((permit, reason)) = interlock(&object)?
            && let Some(refusal) = self.interlocked(&permit, reason.as_ref())
        {
            return Err(refusal.into());
        }
        // The instance, the interface, the method and its inputs are those of the program that
        // runs, read from its image's tables whatever the debugging switch (specification 44
        // §5.4, §5.7, §13.3).
        let program = self
            .control_state
            .program_image()
            .ok_or("the equipment is not available")?;
        let image = program.image();
        let equipment = program_checks::equipment(&image, &command.instance)?;
        let entered = match request.value.as_ref() {
            Some(value) => {
                let (low, high) = self.limits(&object)?;
                Some(capability_checks::entered_value(&object, value, low, high)?)
            }
            None => None,
        };
        let inputs = program_checks::method_inputs(&image, &equipment, &command, entered.as_ref())?;
        Ok(Authorized {
            object,
            command,
            path,
            inputs,
            generation,
        })
    }

    /// The control's limits now: its own `min` and `max`, narrowed by the values its `low-limit`
    /// and `high-limit` bindings read (§5.8). A bound limit that cannot be read refuses.
    fn limits(&self, object: &HmiObject) -> Result<(Option<f64>, Option<f64>), String> {
        let (mut low, mut high) = own_limits(object);
        let mut bound = Vec::new();
        for (property, is_low) in [("low-limit", true), ("high-limit", false)] {
            if let Some(binding) = object.bind.get(property) {
                bound.push((is_low, MemberPath::parse(&binding.path)?));
            }
        }
        if bound.is_empty() {
            return Ok((low, high));
        }
        let paths: Vec<MemberPath> = bound.iter().map(|(_, path)| path.clone()).collect();
        for ((is_low, _), value) in bound.iter().zip(self.read(&paths)) {
            let number = value
                .as_ref()
                .and_then(Json::as_f64)
                .ok_or("the control's limits cannot be read")?;
            if *is_low {
                low = Some(low.map_or(number, |own| own.max(number)));
            } else {
                high = Some(high.map_or(number, |own| own.min(number)));
            }
        }
        Ok((low, high))
    }

    /// Calls the method at the next display boundary: TRUE is accepted; FALSE is refused with the
    /// instance's `CommandText`, or "not accepted" without one (§21.2 items 2 and 3).
    fn call(&self, id: &str, authorized: &Authorized) -> CommandAnswer {
        let mut reason = authorized.path.clone();
        reason
            .segments
            .push(PathSegment::Field(SmolStr::new(REFUSAL_TEXT)));
        let queued = crate::control::hmi_ports::hmi_exchange(&self.control_state)
            .methods
            .enqueue(
                Invocation {
                    path: authorized.path.clone(),
                    interface: authorized.command.interface.clone(),
                    method: authorized.command.method.clone(),
                    inputs: authorized.inputs.clone(),
                    reason: Some(reason),
                },
                authorized.generation,
                Instant::now() + WRITE_TIMEOUT,
            );
        // Losing the response cannot establish that the call did not start.
        let reply = match queued {
            Ok(receiver) => receiver.recv_timeout(WRITE_TIMEOUT).unwrap_or_else(|_| {
                Reply::Fault("the runtime did not answer in time; the outcome is unknown".into())
            }),
            Err(reason) => Reply::Refused(reason),
        };
        match reply {
            Reply::Returned(Value::Bool(true), _) => CommandAnswer::new(id, Outcome::Applied),
            Reply::Returned(Value::Bool(false), reason) => CommandAnswer::refused(
                id,
                reason
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty())
                    .unwrap_or_else(|| NOT_ACCEPTED.to_string()),
            ),
            Reply::Returned(..) => CommandAnswer {
                reason: Some("the method did not answer TRUE or FALSE".into()),
                ..CommandAnswer::new(id, Outcome::Unknown)
            },
            Reply::Refused(reason) => CommandAnswer::refused(id, reason),
            Reply::Fault(reason) => CommandAnswer {
                reason: Some(reason),
                ..CommandAnswer::new(id, Outcome::Unknown)
            },
        }
    }

    /// The command's audit record (§12.7) and its outcome among the records (specification 40,
    /// §5.1): a method command has no old value; it asks for its method with the value sent.
    fn capability_audit(
        &self,
        request: &CommandRequest,
        session: &SessionInfo,
        command: Option<&MethodCommand>,
        answer: &CommandAnswer,
    ) {
        let target = command.map_or_else(
            || request.object.clone(),
            |command| command.instance.clone(),
        );
        let method = command.map_or("invoke", |command| command.method.as_str());
        send_audit(
            &self.control_state,
            "hmi.command",
            Some(session),
            answer,
            json!({
                "command": answer.id,
                "page": request.page,
                "object": request.object,
                "target": target,
                "method": method,
                "old": Json::Null,
                "requested": request.value.clone().unwrap_or(Json::Null),
                "readback": answer.readback,
                "bundle": request.bundle,
            }),
        );
        self.alarms.command(
            &target,
            method,
            Some(&session.user),
            answer.outcome,
            answer.reason.clone(),
        );
    }

    /// After an accepted command: its confirmation by the control's `feedback` reaching `expect`
    /// within its `timeout`, on a thread of its own (§12.5, §21.2 item 3). A command without
    /// feedback is final as accepted.
    fn follow_capability(
        &self,
        session: SessionInfo,
        request: CommandRequest,
        authorized: Authorized,
    ) {
        let Authorized {
            object,
            command,
            generation,
            ..
        } = authorized;
        let Some(feedback) = object.bind.get("feedback") else {
            return;
        };
        let Some(expected) = object
            .expect
            .as_ref()
            .and_then(|expect| serde_json::to_value(expect).ok())
        else {
            return;
        };
        let Ok(point) = MemberPath::parse(&feedback.path) else {
            return;
        };
        let quality = match feedback.quality.as_deref().map(MemberPath::parse) {
            None => None,
            Some(Ok(path)) => Some(path),
            Some(Err(_)) => return,
        };
        if FOLLOWED
            .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_FOLLOWED).then_some(count + 1)
            })
            .is_err()
        {
            return;
        }
        let timeout = object.timeout.map_or(
            crate::hmi::commands::plan::DEFAULT_TIMEOUT,
            Duration::from_secs_f64,
        );
        let commands = Commands {
            registry: self.registry.clone(),
            access: self.access.clone(),
            control_state: self.control_state.clone(),
            store: self.store.clone(),
            alarms: self.alarms.clone(),
        };
        thread::spawn(move || {
            let deadline = Instant::now() + timeout;
            let mut answer = CommandAnswer {
                reason: Some("accepted; the feedback did not confirm it in time".into()),
                ..CommandAnswer::new(&request.id, Outcome::TimedOut)
            };
            loop {
                thread::sleep(POLL);
                let exchange = crate::control::hmi_ports::hmi_exchange(&commands.control_state);
                if exchange.methods.generation() != generation {
                    answer = CommandAnswer {
                        reason: Some(
                            "the program changed after it accepted; outcome unknown".into(),
                        ),
                        ..CommandAnswer::new(&request.id, Outcome::Unknown)
                    };
                    break;
                }
                let current_bundle = commands
                    .store
                    .read(|root| HmiFiles::new(root.to_path_buf()).version());
                if current_bundle != request.bundle {
                    answer = CommandAnswer {
                        reason: Some("the HMI changed after it accepted; outcome unknown".into()),
                        ..CommandAnswer::new(&request.id, Outcome::Unknown)
                    };
                    break;
                }
                let read = match &quality {
                    Some(quality) => {
                        let values = commands.read(&[point.clone(), quality.clone()]);
                        let good = values.get(1).and_then(Option::as_ref).is_some_and(|value| {
                            value == &json!(true)
                                || value
                                    .as_str()
                                    .is_some_and(|text| text.eq_ignore_ascii_case("Good"))
                        });
                        good.then(|| values.first().cloned().flatten()).flatten()
                    }
                    None => commands.observe(&point, &session),
                };
                if read.as_ref().is_some_and(|value| matches(value, &expected)) {
                    answer = CommandAnswer {
                        readback: read,
                        ..CommandAnswer::new(&request.id, Outcome::Confirmed)
                    };
                    break;
                }
                if Instant::now() >= deadline {
                    break;
                }
            }
            commands.capability_audit(&request, &session, Some(&command), &answer);
            commands.registry.later(&session.id, answer);
            FOLLOWED.fetch_sub(1, Ordering::AcqRel);
        });
    }
}

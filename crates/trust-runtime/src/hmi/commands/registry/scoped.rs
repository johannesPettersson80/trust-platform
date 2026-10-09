//! The replay records of method commands (specification 39, §12.8 item 5, §21.5 item 2): one per
//! user and command id, made only once the command is authorized, so a browser's retry gets the
//! first answer without a second call. A user keeps at most 64 records: the oldest answered one
//! makes room for a new command, and a user with 64 commands still open waits for their answers.
//! One user's records never take another user's room.
use super::{CommandAnswer, CommandRegistry, Outcome, REMEMBER};
use std::collections::HashMap;
use std::time::Instant;

/// The most records one user keeps, and so the most commands one user has open.
const PER_USER: usize = 64;
/// The longest description of a command a record keeps.
const MAX_PAYLOAD: usize = 16384;

#[derive(Default)]
pub(super) struct ScopedState {
    /// Each user's records by command id.
    users: HashMap<String, HashMap<String, Record>>,
    /// The last record's number.
    next: u64,
}

struct Record {
    created: Instant,
    /// What the command asked, so an id used again for another command is refused.
    payload: String,
    /// Tells this record from an earlier one of the same id.
    ticket: u64,
    /// The first answer; none while the command is open.
    answer: Option<CommandAnswer>,
}

impl Record {
    fn answered(&self) -> bool {
        self.answer.is_some()
    }

    /// What a repeat of the command gets.
    fn replay(&self, id: &str, payload: &str) -> CommandAnswer {
        if self.payload != payload {
            return CommandAnswer::refused(
                id,
                "this command id was already used for another command",
            );
        }
        self.answer
            .clone()
            .unwrap_or_else(|| CommandAnswer::new(id, Outcome::Pending))
    }
}

/// An authorized command's record, for its answer.
#[derive(Debug, Clone)]
pub(crate) struct Ticket {
    principal: String,
    id: String,
    number: u64,
}

/// Whether an authorized command is new.
pub(crate) enum ScopedBegin {
    /// Recorded now: execute it and give its answer with the ticket.
    New(Ticket),
    /// Not executed: the answer it gets.
    Answer(CommandAnswer),
}

/// Forgets answered records older than ten minutes; an open command's record stays, so a command
/// still being executed is never executed again.
fn forget_old(records: &mut HashMap<String, Record>, now: Instant) {
    records.retain(|_, record| !record.answered() || now.duration_since(record.created) < REMEMBER);
}

impl CommandRegistry {
    /// The answer a repeated command gets, looked up before the command is authorized again:
    /// its first answer, pending while it is executed, or a refusal when the id was used for
    /// another command. `None` for a command id the user has not sent; nothing is recorded.
    pub(crate) fn replay_scoped(
        &self,
        principal: &str,
        id: &str,
        payload: &str,
    ) -> Option<CommandAnswer> {
        let now = (self.now)();
        let mut state = self.lock();
        let records = state.scoped.users.get_mut(principal)?;
        forget_old(records, now);
        records.get(id).map(|record| record.replay(id, payload))
    }

    /// Records an authorized command. The user is derived by the host from the session, never
    /// from the request; the payload describes the command (bundle, page, object, value).
    pub(crate) fn begin_scoped(&self, principal: &str, id: &str, payload: String) -> ScopedBegin {
        let refuse = |reason: &str| ScopedBegin::Answer(CommandAnswer::refused(id, reason));
        if principal.is_empty()
            || principal.len() > 256
            || id.is_empty()
            || id.len() > 128
            || payload.len() > MAX_PAYLOAD
        {
            return refuse("invalid command identity or payload size");
        }
        let now = (self.now)();
        let mut state = self.lock();
        let Some(number) = state.scoped.next.checked_add(1) else {
            return refuse("command identity space exhausted");
        };
        let records = state.scoped.users.entry(principal.to_owned()).or_default();
        forget_old(records, now);
        // A repeat that arrived while this one was authorized gets the first one's answer.
        if let Some(record) = records.get(id) {
            return ScopedBegin::Answer(record.replay(id, &payload));
        }
        if records.len() >= PER_USER {
            let oldest = records
                .iter()
                .filter(|(_, record)| record.answered())
                .min_by_key(|(_, record)| (record.created, record.ticket))
                .map(|(key, _)| key.clone());
            match oldest {
                Some(key) => {
                    records.remove(&key);
                }
                None => {
                    return refuse("64 of your commands are still open; wait for their answers")
                }
            }
        }
        records.insert(
            id.to_owned(),
            Record {
                created: now,
                payload,
                ticket: number,
                answer: None,
            },
        );
        state.scoped.next = number;
        ScopedBegin::New(Ticket {
            principal: principal.to_owned(),
            id: id.to_owned(),
            number,
        })
    }

    /// The first answer of a recorded command, which its repeats get.
    pub(crate) fn answer_scoped(&self, ticket: &Ticket, answer: CommandAnswer) {
        if answer.id != ticket.id {
            return;
        }
        let mut state = self.lock();
        let record = state
            .scoped
            .users
            .get_mut(&ticket.principal)
            .and_then(|records| records.get_mut(&ticket.id));
        if let Some(record) = record {
            if record.ticket == ticket.number && record.answer.is_none() {
                record.answer = Some(answer);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    const PAYLOAD: &str = "bundle/page/start";

    fn ticket(registry: &CommandRegistry, user: &str, id: &str) -> Ticket {
        match registry.begin_scoped(user, id, PAYLOAD.into()) {
            ScopedBegin::New(ticket) => ticket,
            ScopedBegin::Answer(answer) => panic!("expected a new command, got {answer:?}"),
        }
    }

    fn outcome(begin: ScopedBegin) -> Option<Outcome> {
        match begin {
            ScopedBegin::New(_) => None,
            ScopedBegin::Answer(answer) => Some(answer.outcome),
        }
    }

    #[test]
    fn a_repeat_gets_the_first_answer_and_each_user_has_their_own_ids() {
        let registry = CommandRegistry::default();
        assert_eq!(registry.replay_scoped("alice", "same-id", PAYLOAD), None);
        let alice = ticket(&registry, "alice", "same-id");
        ticket(&registry, "bob", "same-id");
        assert_eq!(
            registry
                .replay_scoped("alice", "same-id", PAYLOAD)
                .map(|answer| answer.outcome),
            Some(Outcome::Pending),
            "still being executed"
        );
        registry.answer_scoped(&alice, CommandAnswer::new("same-id", Outcome::Applied));
        assert_eq!(
            outcome(registry.begin_scoped("alice", "same-id", PAYLOAD.into())),
            Some(Outcome::Applied)
        );
        assert_eq!(
            registry
                .replay_scoped("bob", "same-id", PAYLOAD)
                .map(|answer| answer.outcome),
            Some(Outcome::Pending),
            "bob's command of the same id is his own"
        );
    }

    #[test]
    fn an_id_used_again_for_another_command_is_refused() {
        let registry = CommandRegistry::default();
        ticket(&registry, "alice", "c1");
        assert_eq!(
            registry
                .replay_scoped("alice", "c1", "bundle/page/stop")
                .map(|answer| answer.outcome),
            Some(Outcome::Refused)
        );
    }

    #[test]
    fn one_users_open_commands_never_take_another_users_room() {
        let registry = CommandRegistry::default();
        for index in 0..PER_USER {
            ticket(&registry, "alice", &index.to_string());
        }
        assert_eq!(
            outcome(registry.begin_scoped("alice", "overflow", PAYLOAD.into())),
            Some(Outcome::Refused),
            "64 open commands of one user"
        );
        ticket(&registry, "bob", "first");
        assert_eq!(
            outcome(registry.begin_scoped("alice", "0", PAYLOAD.into())),
            Some(Outcome::Pending),
            "an open command is never forgotten"
        );
    }

    #[test]
    fn the_oldest_answered_record_makes_room_for_a_new_command() {
        let time = Arc::new(Mutex::new(Instant::now()));
        let clock = time.clone();
        let registry = CommandRegistry::with_clock(Arc::new(move || *clock.lock().expect("clock")));
        for index in 0..PER_USER {
            let ticket = ticket(&registry, "alice", &index.to_string());
            registry.answer_scoped(
                &ticket,
                CommandAnswer::new(&index.to_string(), Outcome::Applied),
            );
            *time.lock().expect("clock") += Duration::from_millis(1);
        }
        ticket(&registry, "alice", "next");
        assert_eq!(
            registry.replay_scoped("alice", "0", PAYLOAD),
            None,
            "the oldest went"
        );
        assert!(registry.replay_scoped("alice", "1", PAYLOAD).is_some());
    }

    #[test]
    fn a_record_is_forgotten_after_ten_minutes_but_never_while_its_command_is_open() {
        let time = Arc::new(Mutex::new(Instant::now()));
        let clock = time.clone();
        let registry = CommandRegistry::with_clock(Arc::new(move || *clock.lock().expect("clock")));
        let answered = ticket(&registry, "alice", "answered");
        registry.answer_scoped(&answered, CommandAnswer::new("answered", Outcome::Applied));
        ticket(&registry, "alice", "open");
        *time.lock().expect("clock") += REMEMBER + Duration::from_secs(1);
        assert_eq!(registry.replay_scoped("alice", "answered", PAYLOAD), None);
        assert!(registry.replay_scoped("alice", "open", PAYLOAD).is_some());
    }
}

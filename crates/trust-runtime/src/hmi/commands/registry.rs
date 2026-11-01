//! The record of HMI commands by id (specification 39, §12.5, §12.8): a repeated id gets the
//! first answer for ten minutes without a second write, and later outcomes wait for the value
//! streams of the session that sent the command. Method commands keep their records per user
//! (`scoped`, §21.5 item 2).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
mod scoped;
pub(crate) use scoped::ScopedBegin;
use smol_str::SmolStr;

/// How long a command id is remembered.
pub const REMEMBER: Duration = Duration::from_secs(10 * 60);
/// The most later outcomes kept for the streams.
const MAX_EVENTS: usize = 1024;

/// A command's outcome (§12.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Sent, not yet decided.
    Pending,
    /// By permission, policy, limits, interlock or an out-of-date page, with the reason.
    Refused,
    /// Written to the program in a scan cycle.
    Applied,
    /// The feedback reached the expected state in time.
    Confirmed,
    /// Not applied, or not confirmed, in time.
    TimedOut,
    /// The connection was lost before the outcome was known; never assumed successful.
    Unknown,
}

/// What the runtime answers about a command.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CommandAnswer {
    /// The command's id.
    pub id: String,
    /// Its outcome.
    pub outcome: Outcome,
    /// The value read back from the program.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readback: Option<serde_json::Value>,
    /// Why it was refused or timed out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The stable code of a refusal that has one: `runtime_string_capacity_exceeded` for a text
    /// longer than its point holds (specification 39, §12.8 item 5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// The runtime on which a named account must log in before the command is sent; the display
    /// asks for that login (specification 39, remote operation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    /// For a pulse: it has ended.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ended: bool,
}

impl CommandAnswer {
    /// An answer without a value or a reason.
    pub fn new(id: &str, outcome: Outcome) -> Self {
        Self {
            id: id.to_string(),
            outcome,
            readback: None,
            reason: None,
            error_code: None,
            login: None,
            ended: false,
        }
    }

    /// A refusal with its reason.
    pub fn refused(id: &str, reason: impl Into<String>) -> Self {
        Self {
            reason: Some(reason.into()),
            ..Self::new(id, Outcome::Refused)
        }
    }

    /// The refusal of a write the runtime did not make: its reason, and its stable code when it has
    /// one.
    pub fn write_refused(id: &str, refused: super::write::WriteRefused) -> Self {
        Self {
            error_code: refused.error_code.map(str::to_string),
            login: refused.login,
            ..Self::refused(id, refused.reason)
        }
    }
}

/// Whether a command id is new.
#[derive(Debug, Clone, PartialEq)]
pub enum Begin {
    /// Never seen, or forgotten: execute it.
    New,
    /// Seen within ten minutes: its first answer, or pending while it is still decided.
    Repeat(CommandAnswer),
}

/// A later outcome for the streams of one session.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandEvent {
    /// Increases with every event.
    pub seq: u64,
    /// The session that sent the command.
    pub session: SmolStr,
    /// The outcome.
    pub answer: CommandAnswer,
}

/// The runtime's record of commands.
pub struct CommandRegistry {
    state: Mutex<State>,
    now: Arc<dyn Fn() -> Instant + Send + Sync>,
}

#[derive(Default)]
struct State {
    scoped: scoped::ScopedState,
    first: HashMap<String, (Instant, Option<CommandAnswer>)>,
    events: VecDeque<CommandEvent>,
    next: u64,
}

impl std::fmt::Debug for CommandRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandRegistry").finish_non_exhaustive()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::with_clock(Arc::new(Instant::now))
    }
}

impl CommandRegistry {
    /// A record on a given clock, for tests.
    pub fn with_clock(now: Arc<dyn Fn() -> Instant + Send + Sync>) -> Self {
        Self {
            state: Mutex::new(State::default()),
            now,
        }
    }

    /// Starts a command id.
    pub fn begin(&self, id: &str) -> Begin {
        let now = (self.now)();
        let mut state = self.lock();
        state
            .first
            .retain(|_, (started, _)| now.duration_since(*started) < REMEMBER);
        if let Some((_, answer)) = state.first.get(id) {
            return Begin::Repeat(
                answer
                    .clone()
                    .unwrap_or_else(|| CommandAnswer::new(id, Outcome::Pending)),
            );
        }
        state.first.insert(id.to_string(), (now, None));
        Begin::New
    }

    /// The first answer to a command, which a repeated id gets.
    pub fn answer(&self, answer: &CommandAnswer) {
        if let Some((_, first)) = self.lock().first.get_mut(&answer.id) {
            *first = Some(answer.clone());
        }
    }

    /// A later outcome, for the streams of the session.
    pub fn later(&self, session: &str, answer: CommandAnswer) {
        let mut state = self.lock();
        state.next += 1;
        let seq = state.next;
        state.events.push_back(CommandEvent {
            seq,
            session: SmolStr::new(session),
            answer,
        });
        while state.events.len() > MAX_EVENTS {
            state.events.pop_front();
        }
    }

    /// The session's later outcomes after `seq`, oldest first.
    pub fn events_after(&self, session: &str, seq: u64) -> Vec<CommandEvent> {
        self.lock()
            .events
            .iter()
            .filter(|event| event.seq > seq && event.session == session)
            .cloned()
            .collect()
    }

    /// The newest event's number; a stream starts after it.
    pub fn last_seq(&self) -> u64 {
        self.lock().next
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_id_is_remembered_for_ten_minutes() {
        let time = Arc::new(Mutex::new(Instant::now()));
        let clock = time.clone();
        let registry = CommandRegistry::with_clock(Arc::new(move || *clock.lock().expect("clock")));
        assert_eq!(registry.begin("c1"), Begin::New);
        assert_eq!(
            registry.begin("c1"),
            Begin::Repeat(CommandAnswer::new("c1", Outcome::Pending)),
            "still being decided"
        );
        let answer = CommandAnswer {
            readback: Some(serde_json::json!(61.5)),
            ..CommandAnswer::new("c1", Outcome::Applied)
        };
        registry.answer(&answer);
        *time.lock().expect("clock") += Duration::from_secs(9 * 60);
        assert_eq!(registry.begin("c1"), Begin::Repeat(answer));
        *time.lock().expect("clock") += Duration::from_secs(61);
        assert_eq!(
            registry.begin("c1"),
            Begin::New,
            "forgotten after ten minutes"
        );
    }

    #[test]
    fn a_refusal_for_an_owner_login_names_the_runtime_in_its_own_field() {
        // Specification 39, remote operation: a control whose owner has no session asks for a named
        // login on that runtime. The answer names the runtime as `login`, so no display parses it out
        // of the reason, and the audit's reason keeps `owner-login:<runtime>`. Catches a refusal that
        // carries the runtime only inside its reason, and a `login` on an ordinary refusal.
        let answer = CommandAnswer::write_refused(
            "c1",
            super::super::write::WriteRefused::owner_login("Owner"),
        );
        assert_eq!(
            serde_json::to_value(&answer).expect("an answer serializes"),
            serde_json::json!({
                "id": "c1",
                "outcome": "refused",
                "reason": "owner-login:Owner",
                "login": "Owner",
            })
        );
        let ordinary = CommandAnswer::write_refused("c2", "out of range".into());
        assert_eq!(
            serde_json::to_value(&ordinary).expect("an answer serializes"),
            serde_json::json!({ "id": "c2", "outcome": "refused", "reason": "out of range" })
        );
    }

    #[test]
    fn later_outcomes_go_to_the_session_that_sent_the_command() {
        let registry = CommandRegistry::default();
        let start = registry.last_seq();
        registry.later("s1", CommandAnswer::new("c1", Outcome::Confirmed));
        registry.later("s2", CommandAnswer::new("c2", Outcome::Confirmed));
        let mine = registry.events_after("s1", start);
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].answer.id, "c1");
        assert!(registry.events_after("s1", mine[0].seq).is_empty());
    }
}

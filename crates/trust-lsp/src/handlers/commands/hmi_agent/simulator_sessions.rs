//! The named simulator sessions of the HMI agent's simulator journeys (specification 43, AT-08).
//!
//! The extension logs in to a local simulator's HMI for a session opened here and keeps the
//! session's cookie (specification 39 §11 item 6). The language server keeps what it decides about
//! the session: its opaque ID, the workspace and journey name it belongs to, the target it was
//! opened for, its expiry ten minutes on and the journey it runs. A session opens only for a target
//! that `checkTarget` accepts, while its workspace holds fewer than five unexpired sessions. It runs
//! one journey at a time, named by the journey ID its client chose, and a journey whose target
//! differs from the session's is refused and closes the session. Closing a session cancels the
//! journey it runs. A cancel names the journey it cancels: a cancel that finds that journey not
//! running, as when it arrives just before the journey starts, refuses that journey's start for two
//! seconds, and affects no other journey.
//! The target is kept as a hash of its web address, control endpoint and access token, so the token
//! itself is not kept. Time is passed in, so the rules hold for any clock.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use super::documents::sha256_hex;
use super::draft_records::new_uuid;
use super::error::{AgentError, AgentResult, CANCELLED, EXPIRED_DRAFT, NOT_CONNECTED};
use super::target::is_local_simulator;

/// How long a session lasts.
pub(super) const SESSION_LIFETIME: Duration = Duration::from_secs(10 * 60);
/// The most unexpired sessions one workspace holds.
pub(super) const MAX_SESSIONS: usize = 5;
/// How long a cancel that found its journey not running still refuses that journey's start: it can
/// arrive just before the journey it was sent for.
pub(super) const EARLY_CANCEL: Duration = Duration::from_secs(2);
/// The most cancels a session remembers for journeys that have not started; the oldest goes first.
const EARLY_CANCELS: usize = 16;

/// One named session.
struct Session {
    workspace: String,
    name: String,
    /// The hash of the target it was opened for.
    target: String,
    expires: SystemTime,
    /// The ID and the cancellation of the journey it runs, while it runs one.
    journey: Option<(String, Arc<AtomicBool>)>,
    /// The journeys a cancel named while they did not run, and when.
    early_cancels: Vec<(String, SystemTime)>,
}

impl Session {
    /// Whether the session has not expired at `now`: it lasts up to its expiry, included.
    fn unexpired(&self, now: SystemTime) -> bool {
        now <= self.expires
    }

    /// The session as the agent's context lists it.
    fn view(&self, id: &str) -> Value {
        json!({ "sessionId": id, "name": self.name, "expiresAt": iso_time(self.expires) })
    }
}

/// The named simulator sessions of one language server.
pub(super) struct SimulatorSessions {
    sessions: Mutex<BTreeMap<String, Session>>,
}

impl SimulatorSessions {
    /// No sessions.
    pub(super) const fn new() -> Self {
        Self {
            sessions: Mutex::new(BTreeMap::new()),
        }
    }

    /// Opens a session named `name` in `workspace` for `target`, expiring ten minutes after `now`:
    /// `{ sessionId, name, expiresAt }`.
    ///
    /// # Errors
    ///
    /// `NOT_CONNECTED` when `checkTarget` refuses the target, `LIMIT_EXCEEDED` when the workspace
    /// already holds five unexpired sessions.
    pub(super) fn open(
        &self,
        workspace: &str,
        name: &str,
        target: &Value,
        now: SystemTime,
    ) -> AgentResult<Value> {
        if !is_local_simulator(target) {
            return Err(AgentError::new(
                NOT_CONNECTED,
                "Start a local simulator with an HMI web endpoint before connecting a simulation \
                 journey. Physical or remote targets are refused.",
            ));
        }
        let mut sessions = self.locked(now);
        let held = sessions
            .values()
            .filter(|session| session.workspace == workspace && session.unexpired(now))
            .count();
        if held >= MAX_SESSIONS {
            return Err(AgentError::limit(
                "Five simulator sessions already exist in this workspace; wait for one to expire \
                 or restart the editor.",
            ));
        }
        let id = new_uuid("a simulator session identifier")?;
        let session = Session {
            workspace: workspace.to_owned(),
            name: name.to_owned(),
            target: identity(target),
            expires: now + SESSION_LIFETIME,
            journey: None,
            early_cancels: Vec::new(),
        };
        let view = session.view(&id);
        sessions.insert(id, session);
        Ok(view)
    }

    /// Closes the session `id`, cancelling the journey it runs; whether it was open.
    pub(super) fn close(&self, id: &str) -> bool {
        close(&mut self.locked_now(), id)
    }

    /// The unexpired sessions of `workspace` at `now`, the first to expire first.
    pub(super) fn list(&self, workspace: &str, now: SystemTime) -> Vec<Value> {
        let sessions = self.locked(now);
        let mut listed: Vec<(&String, &Session)> = sessions
            .iter()
            .filter(|(_, session)| session.workspace == workspace && session.unexpired(now))
            .collect();
        listed.sort_by(|(left_id, left), (right_id, right)| {
            (left.expires, left_id).cmp(&(right.expires, right_id))
        });
        listed
            .into_iter()
            .map(|(id, session)| session.view(id))
            .collect()
    }

    /// Starts the journey `journey` on the session `id` of `workspace` for `target` at `now`. The
    /// journey ends when the answer is dropped.
    ///
    /// # Errors
    ///
    /// `EXPIRED_DRAFT` when the workspace holds no such unexpired session; `CONFLICT` when the
    /// target differs from the session's, which also closes the session and cancels the journey it
    /// runs, or when the session runs a journey already; `CANCELLED` when a cancel naming this
    /// journey arrived within the last two seconds, before it started.
    pub(super) fn begin(
        &self,
        workspace: &str,
        id: &str,
        journey: &str,
        target: &Value,
        now: SystemTime,
    ) -> AgentResult<RunningJourney<'_>> {
        let mut sessions = self.locked(now);
        let Some(session) = sessions
            .get_mut(id)
            .filter(|session| session.workspace == workspace && session.unexpired(now))
        else {
            return Err(AgentError::new(
                EXPIRED_DRAFT,
                "select an unexpired named simulator session in this workspace.",
            ));
        };
        if session.target != identity(target) {
            close(&mut sessions, id);
            return Err(AgentError::conflict(
                "the simulator target changed since this named session was connected, which closed \
                 the session; connect the simulator journey again.",
            ));
        }
        if session.journey.is_some() {
            return Err(AgentError::conflict(
                "this simulator journey is already running.",
            ));
        }
        forget_old_cancels(&mut session.early_cancels, now);
        let named = session
            .early_cancels
            .iter()
            .position(|(cancelled, _)| cancelled == journey);
        if let Some(index) = named {
            session.early_cancels.remove(index);
            return Err(AgentError::new(
                CANCELLED,
                "the simulator journey was cancelled before it started.",
            ));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        session.journey = Some((journey.to_owned(), Arc::clone(&cancel)));
        Ok(RunningJourney {
            sessions: self,
            id: id.to_owned(),
            name: session.name.clone(),
            cancel,
        })
    }

    /// Whether the session `id` is open and unexpired at `now`.
    pub(super) fn is_live(&self, id: &str, now: SystemTime) -> bool {
        self.locked_now()
            .get(id)
            .is_some_and(|session| session.unexpired(now))
    }

    /// Cancels the journey `journey` of the session `id` at `now`; whether the session runs it. A
    /// cancel that finds that journey not running refuses its start for two seconds
    /// ([`EARLY_CANCEL`]) and affects no other journey.
    pub(super) fn cancel(&self, id: &str, journey: &str, now: SystemTime) -> bool {
        let mut sessions = self.locked_now();
        let Some(session) = sessions.get_mut(id) else {
            return false;
        };
        match &session.journey {
            Some((running, cancel)) if running == journey => {
                cancel.store(true, Ordering::SeqCst);
                true
            }
            _ => {
                forget_old_cancels(&mut session.early_cancels, now);
                if session.early_cancels.len() == EARLY_CANCELS {
                    session.early_cancels.remove(0);
                }
                session.early_cancels.push((journey.to_owned(), now));
                false
            }
        }
    }

    /// Whether the simulator login for the session `id` may go to `target` at `now`: the session is
    /// open and unexpired, the target is the one it was opened for, and `checkTarget` accepts it.
    pub(super) fn admits(&self, id: &str, target: &Value, now: SystemTime) -> bool {
        is_local_simulator(target)
            && self
                .locked_now()
                .get(id)
                .is_some_and(|session| session.unexpired(now) && session.target == identity(target))
    }

    /// The sessions, without the expired ones that run no journey: one that runs a journey stays
    /// until the journey ends, so that it can still be cancelled.
    fn locked(&self, now: SystemTime) -> MutexGuard<'_, BTreeMap<String, Session>> {
        let mut sessions = self.locked_now();
        sessions.retain(|_, session| session.journey.is_some() || session.unexpired(now));
        sessions
    }

    fn locked_now(&self) -> MutexGuard<'_, BTreeMap<String, Session>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Removes the session `id`, cancelling the journey it runs; whether it was there.
fn close(sessions: &mut BTreeMap<String, Session>, id: &str) -> bool {
    let Some(session) = sessions.remove(id) else {
        return false;
    };
    if let Some((_, journey)) = session.journey {
        journey.store(true, Ordering::SeqCst);
    }
    true
}

/// Forgets the cancels of journeys that did not start within two seconds.
fn forget_old_cancels(cancels: &mut Vec<(String, SystemTime)>, now: SystemTime) {
    cancels.retain(|(_, at)| {
        now.duration_since(*at)
            .is_ok_and(|since| since <= EARLY_CANCEL)
    });
}

/// A journey running on a session: its name and its cancellation. Dropping it ends the journey, so
/// that the session can run the next one.
pub(super) struct RunningJourney<'a> {
    sessions: &'a SimulatorSessions,
    id: String,
    name: String,
    cancel: Arc<AtomicBool>,
}

impl RunningJourney<'_> {
    /// The session's name.
    pub(super) fn name(&self) -> &str {
        &self.name
    }

    /// Set when the journey is cancelled.
    pub(super) fn cancellation(&self) -> &Arc<AtomicBool> {
        &self.cancel
    }
}

impl Drop for RunningJourney<'_> {
    fn drop(&mut self) {
        let mut sessions = self.sessions.locked_now();
        if let Some(session) = sessions.get_mut(&self.id)
            && session
                .journey
                .as_ref()
                .is_some_and(|(_, journey)| Arc::ptr_eq(journey, &self.cancel))
        {
            session.journey = None;
        }
    }
}

/// The hash of a target's web address, control endpoint and access token: the identity the HMI
/// bridge pins its target by (`LiveSession`).
fn identity(target: &Value) -> String {
    let field = |key: &str| target.get(key).and_then(Value::as_str);
    let fields = json!([field("endpoint"), field("url"), field("token")]);
    sha256_hex(fields.to_string().as_bytes())
}

/// A time as JavaScript's `toISOString` writes it, such as `2026-11-04T10:00:00.000Z`.
pub(super) fn iso_time(time: SystemTime) -> String {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds = since.as_secs();
    let days = i64::try_from(seconds / 86_400).unwrap_or(i64::MAX);
    let (year, month, day) = civil_from_days(days);
    let of_day = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        of_day / 3_600,
        of_day / 60 % 60,
        of_day % 60,
        since.subsec_millis()
    )
}

/// The proleptic Gregorian year, month and day of a count of days since 1970-01-01 (Howard
/// Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
#[path = "simulator_sessions_tests.rs"]
mod tests;

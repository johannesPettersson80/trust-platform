//! HMI sessions (specification 39, §12.4, §12.8): a named user logs in with a password, and the
//! session ends after an idle time without a command or interaction, and after an absolute time
//! in any case. Five failed attempts refuse a user for a minute.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use smol_str::SmolStr;

use super::config::HmiAccessConfig;
use crate::security::AccessRole;

/// Failed attempts before a user is refused for a while.
pub const MAX_FAILURES: u32 = 5;
/// How long a user is refused after too many failed attempts.
pub const LOCKOUT: Duration = Duration::from_secs(60);
/// The most sessions held at once; the oldest ends first.
pub const MAX_SESSIONS: usize = 256;

/// What a session is, as the browser and the audit log see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    /// A short id for the audit log, not the secret token.
    pub id: SmolStr,
    /// The user's name as configured.
    pub user: SmolStr,
    /// What the user may do.
    pub role: AccessRole,
    /// Time left before the session ends without activity.
    pub idle_expires_in: Duration,
    /// Time left before the session ends in any case.
    pub expires_in: Duration,
}

/// Why a login was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginRefusal {
    /// The same answer for an unknown user and a wrong password.
    WrongUserOrPassword,
    /// Too many failed attempts.
    Locked {
        /// How long until the user may try again.
        retry_in: Duration,
    },
}

/// The runtime's sessions.
pub struct SessionStore {
    access: HmiAccessConfig,
    state: Mutex<State>,
    now: Arc<dyn Fn() -> Instant + Send + Sync>,
}

#[derive(Default)]
struct State {
    /// By secret token.
    sessions: HashMap<String, Session>,
    /// Failed attempts by user name in lower case, known or not.
    failures: HashMap<String, Failures>,
}

struct Session {
    id: SmolStr,
    user: SmolStr,
    role: AccessRole,
    started: Instant,
    active: Instant,
}

#[derive(Default)]
struct Failures {
    count: u32,
    locked_until: Option<Instant>,
}

impl std::fmt::Debug for SessionStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionStore").finish_non_exhaustive()
    }
}

impl SessionStore {
    /// Sessions of the configured users, on the system clock.
    pub fn new(access: HmiAccessConfig) -> Self {
        Self::with_clock(access, Arc::new(Instant::now))
    }

    /// Sessions on a given clock, for tests.
    pub fn with_clock(
        access: HmiAccessConfig,
        now: Arc<dyn Fn() -> Instant + Send + Sync>,
    ) -> Self {
        Self {
            access,
            state: Mutex::new(State::default()),
            now,
        }
    }

    /// Starts a session: the secret token for the cookie and what the session is.
    ///
    /// The password is checked outside the lock; an unknown user's attempt costs the same as a
    /// known one's, so the answer and its timing do not tell whether the user exists.
    pub fn login(&self, user: &str, password: &str) -> Result<(String, SessionInfo), LoginRefusal> {
        let key = user.trim().to_lowercase();
        let now = (self.now)();
        {
            let mut state = self.lock();
            if let Some(until) = state
                .failures
                .get(&key)
                .and_then(|failures| failures.locked_until)
            {
                if until > now {
                    return Err(LoginRefusal::Locked {
                        retry_in: until - now,
                    });
                }
                state.failures.remove(&key);
            }
        }
        let known = self
            .access
            .users
            .iter()
            .find(|candidate| candidate.name.to_lowercase() == key);
        let accepted = match known {
            Some(found) => found.password.verify(password),
            None => {
                if let Some(any) = self.access.users.first() {
                    let _ = any.password.verify(password);
                }
                false
            }
        };
        let mut state = self.lock();
        let Some(found) = known.filter(|_| accepted) else {
            let failures = state.failures.entry(key).or_default();
            failures.count += 1;
            if failures.count >= MAX_FAILURES {
                failures.locked_until = Some(now + LOCKOUT);
            }
            return Err(LoginRefusal::WrongUserOrPassword);
        };
        state.failures.remove(&key);
        state.sessions.retain(|_, session| self.alive(session, now));
        if state.sessions.len() >= MAX_SESSIONS
            && let Some(oldest) = state
                .sessions
                .iter()
                .min_by_key(|(_, session)| session.started)
                .map(|(token, _)| token.clone())
        {
            state.sessions.remove(&oldest);
        }
        let token = random_text(32);
        let session = Session {
            id: SmolStr::new(random_text(9)),
            user: found.name.clone(),
            role: found.role,
            started: now,
            active: now,
        };
        let info = self.info(&session, now);
        state.sessions.insert(token.clone(), session);
        Ok((token, info))
    }

    /// The session of a token, if it has not ended.
    pub fn session(&self, token: &str) -> Option<SessionInfo> {
        self.with_session(token, false)
    }

    /// Activity in a session (a command or an interaction): its idle time starts again.
    pub fn touch(&self, token: &str) -> Option<SessionInfo> {
        self.with_session(token, true)
    }

    /// Ends a session; whether it existed.
    pub fn logout(&self, token: &str) -> bool {
        let now = (self.now)();
        let mut state = self.lock();
        state
            .sessions
            .remove(token)
            .is_some_and(|session| self.alive(&session, now))
    }

    fn with_session(&self, token: &str, activity: bool) -> Option<SessionInfo> {
        let now = (self.now)();
        let mut state = self.lock();
        let alive = state
            .sessions
            .get(token)
            .is_some_and(|session| self.alive(session, now));
        if !alive {
            state.sessions.remove(token);
            return None;
        }
        let session = state.sessions.get_mut(token)?;
        if activity {
            session.active = now;
        }
        Some(self.info(session, now))
    }

    fn alive(&self, session: &Session, now: Instant) -> bool {
        // A session ends when its time is used up.
        now.duration_since(session.active) < self.access.idle
            && now.duration_since(session.started) < self.access.session
    }

    fn info(&self, session: &Session, now: Instant) -> SessionInfo {
        SessionInfo {
            id: session.id.clone(),
            user: session.user.clone(),
            role: session.role,
            idle_expires_in: self
                .access
                .idle
                .saturating_sub(now.duration_since(session.active)),
            expires_in: self
                .access
                .session
                .saturating_sub(now.duration_since(session.started)),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Random bytes as unpadded URL-safe base64.
fn random_text(bytes: usize) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use rand::TryRng;
    let mut buffer = vec![0_u8; bytes];
    rand::rngs::SysRng
        .try_fill_bytes(&mut buffer)
        .expect("the system's random source works");
    URL_SAFE_NO_PAD.encode(buffer)
}

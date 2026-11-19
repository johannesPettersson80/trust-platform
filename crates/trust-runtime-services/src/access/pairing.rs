//! Pairing (specification 11 §6.9.6; specification 34 S5): an Admin starts a six-digit code, and a
//! client claims a role-scoped token with it. These are the rules of every runtime, a computer's
//! over its file `pairings.json` and a chip's over a record in its flash: the platform supplies the
//! storage and the random numbers through a [`PairingPort`], and its wall clock through the time,
//! in Unix seconds, that every operation takes.
//!
//! - A runtime holds at most one pending code, and starting again replaces it. A code is six ASCII
//!   decimal digits, drawn from four random bytes, and is valid through its `expires_at`, 300
//!   seconds after the start. A claim trims the code it is given. A wrong code leaves a pending
//!   code that has not expired; a claim drops an expired code, and a claim with the right code
//!   takes it, whether the claim then stores a token or is refused.
//! - A claim stores one enabled token: 32 random bytes in URL-safe base64 without padding, with
//!   the id `pair-<now>-<its first 8 characters>`, numbered `-2`, `-3` and on when an earlier
//!   token has that id, the requested role (Operator when none is
//!   requested, Engineer for an Admin request, so that pairing never mints an Admin token), valid
//!   through 30 days after the claim. At most [`PairingPort::max_tokens`] enabled tokens exist.
//! - Expired rows leave the pairings when they open and before every operation, and the file is
//!   written without them when it can be.
//! - A claim, and a revoke or a revoke-all that disables a token, succeed only once the port has
//!   written the file. A write that fails leaves every token as it was. A file that the port could
//!   not read grants no token and is never replaced: its error refuses every later write.
//! - A storage that cannot hold a changed file, a chip's record of one flash sector
//!   ([`PairingPort::fits`]), makes room by removing disabled tokens, which grant nothing, the
//!   oldest first; a change that does not fit even then is refused with
//!   [`PairingError::NoRoom`].
//! - A service token (specification 48 RU-81) is a token that carries a service's name, which an
//!   Admin creates without a code ([`Pairings::service`]): Viewer unless another role is
//!   requested, with the id `service-<name>`. It never expires, and creating it again for a name
//!   that has one, compared without case, replaces that token in its place.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::fmt;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use super::role::{constant_time_eq, AccessRole};

/// How long a code stays valid after its start: 300 seconds.
pub const CODE_TTL_SECS: u64 = 300;

/// How long a token stays valid after its claim: 30 days.
pub const TOKEN_TTL_SECS: u64 = 30 * 24 * 60 * 60;

/// The most enabled tokens a runtime keeps: 256. A port whose storage holds fewer says so with
/// [`PairingPort::max_tokens`].
pub const MAX_TOKENS: usize = 256;

/// The random bytes a code is drawn from.
const CODE_BYTES: usize = 4;

/// The random bytes of one token.
const TOKEN_BYTES: usize = 32;

/// A started code, as `pair.start` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingCode {
    /// Six ASCII decimal digits.
    pub code: String,
    /// The last second, in Unix seconds, at which a claim takes the code.
    pub expires_at: u64,
}

/// One token as `pair.list` reports it: never the token itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PairingSummary {
    /// The token's id, which `pair.revoke` names.
    pub id: String,
    /// Whether the token grants its role; a revoked token is listed until it expires.
    pub enabled: bool,
    /// When the token was claimed, in Unix seconds.
    pub created_at: u64,
    /// The last second, in Unix seconds, at which the token grants its role; `None` for a service
    /// token, which does not expire.
    pub expires_at: Option<u64>,
    /// The role the token grants.
    pub role: AccessRole,
    /// The service a service token is for, `None` for a claimed token.
    pub service: Option<String>,
    /// `…` and the last four characters of the token, or all of a shorter one.
    pub tail: String,
}

/// A created service token, as `pair.service` reports it.
#[derive(Clone, PartialEq, Eq)]
pub struct ServiceToken {
    /// The token's id, `service-<name>`, which `pair.revoke` names.
    pub id: String,
    /// The role the token grants.
    pub role: AccessRole,
    /// The secret the service presents.
    pub token: String,
}

impl fmt::Debug for ServiceToken {
    /// Every field, with the token reduced to its tail, so that a log never holds a secret.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceToken")
            .field("id", &self.id)
            .field("role", &self.role)
            .field("token", &mask_tail(&self.token))
            .finish()
    }
}

/// One stored token: a row of the pairing file. The names and defaults are those of the
/// computer's `pairings.json`, so a file written before still loads.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingToken {
    /// The token's id.
    pub id: String,
    /// The secret a client presents.
    pub token: String,
    /// When the token was claimed, in Unix seconds.
    pub created_at: u64,
    /// Whether the token grants its role.
    pub enabled: bool,
    /// The role the token grants; Operator in a row that names none.
    #[serde(default = "default_role")]
    pub role: AccessRole,
    /// The last second, in Unix seconds, at which the token grants its role; 0 in a row written
    /// before tokens expired, which the pairings read as 30 days after `created_at`. A service
    /// token does not expire and keeps 0.
    #[serde(default)]
    pub expires_at: u64,
    /// The service a service token is for; absent in a claimed token's row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
}

impl fmt::Debug for PairingToken {
    /// Every field, with the token reduced to its tail, so that a log never holds a secret.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairingToken")
            .field("id", &self.id)
            .field("token", &mask_tail(&self.token))
            .field("created_at", &self.created_at)
            .field("enabled", &self.enabled)
            .field("role", &self.role)
            .field("expires_at", &self.expires_at)
            .field("service", &self.service)
            .finish()
    }
}

/// The pairing file: a JSON object with a `tokens` array, in storage order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingFile {
    /// The stored tokens.
    pub tokens: Vec<PairingToken>,
}

/// Why a pairing operation did not succeed. The display is the reply's error text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingError {
    /// No code is pending, the code is wrong, or it expired: `invalid or expired code`.
    InvalidCode,
    /// The enabled tokens are at the limit: `at most <max> pairing tokens may be enabled; revoke
    /// one first`.
    LimitReached {
        /// The port's limit of enabled tokens.
        max: usize,
    },
    /// The pairing file could not be read when the pairings opened, so no write may replace it:
    /// the port's text.
    Unreadable(String),
    /// The pairing file could not be written: the port's text.
    Save(String),
    /// The port gave no random bytes: the port's text.
    Random(String),
    /// The platform cannot reach its pairings now, for example behind a lock that a panic
    /// poisoned: `pairing unavailable`.
    Unavailable,
    /// The storage cannot hold the changed file even without its disabled tokens: `the device has
    /// no room for another pairing`.
    NoRoom,
    /// A service token's name is not an identifier: `invalid service name`.
    InvalidServiceName,
}

impl fmt::Display for PairingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCode => f.write_str("invalid or expired code"),
            Self::LimitReached { max } => write!(
                f,
                "at most {max} pairing tokens may be enabled; revoke one first"
            ),
            Self::Unreadable(text) | Self::Save(text) | Self::Random(text) => f.write_str(text),
            Self::Unavailable => f.write_str("pairing unavailable"),
            Self::NoRoom => f.write_str("the device has no room for another pairing"),
            Self::InvalidServiceName => f.write_str("invalid service name"),
        }
    }
}

impl core::error::Error for PairingError {}

/// What a platform supplies to its pairings: the storage of the pairing file and random numbers.
pub trait PairingPort {
    /// The stored pairing file; an absent file is an empty one.
    ///
    /// # Errors
    ///
    /// The text of a file that exists but cannot be read or is not a pairing file. The pairings
    /// then grant no token and never write the file.
    fn load(&mut self) -> Result<PairingFile, String>;

    /// Writes `file` whole and durably, so that the previous file stays readable until the new
    /// one has replaced it.
    ///
    /// # Errors
    ///
    /// The text of a write that did not happen or did not complete; it becomes the reply's error.
    fn save(&mut self, file: &PairingFile) -> Result<(), String>;

    /// Fills `bytes` with random bytes from a cryptographically secure source.
    ///
    /// # Errors
    ///
    /// The text of a source that gives none now; it becomes the reply's error.
    fn random(&mut self, bytes: &mut [u8]) -> Result<(), String>;

    /// The most enabled tokens the storage keeps: [`MAX_TOKENS`] unless the storage holds fewer.
    fn max_tokens(&self) -> usize {
        MAX_TOKENS
    }

    /// Whether the storage holds `file`: always, unless its room is bounded, as a chip's record of
    /// one flash sector is.
    fn fits(&self, _file: &PairingFile) -> bool {
        true
    }
}

/// The pairings of one runtime over its [`PairingPort`].
pub struct Pairings<P: PairingPort> {
    port: P,
    file: PairingFile,
    pending: Option<PairingCode>,
    /// The error of a file the port could not read when the pairings opened.
    unreadable: Option<String>,
}

impl<P: PairingPort> Pairings<P> {
    /// Opens the pairings of `port` at `now`: loads the file, gives each claimed row without an
    /// expiry (`expires_at` 0) its 30 days from `created_at`, saturating, and removes the rows that
    /// expired before `now`. A file that cannot be loaded grants no token, and its error refuses
    /// every later write.
    #[must_use]
    pub fn open(mut port: P, now: u64) -> Self {
        let (file, unreadable) = match port.load() {
            Ok(file) => (file, None),
            Err(error) => (PairingFile::default(), Some(error)),
        };
        let mut pairings = Self {
            port,
            file,
            pending: None,
            unreadable,
        };
        for row in &mut pairings.file.tokens {
            if row.expires_at == 0 && row.service.is_none() {
                row.expires_at = row.created_at.saturating_add(TOKEN_TTL_SECS);
            }
        }
        pairings.prune(now);
        pairings
    }

    /// Starts a pairing at `now`: a new code, valid through `now + 300`, which replaces any
    /// pending code.
    ///
    /// # Errors
    ///
    /// [`PairingError::Random`] when the port gives no random bytes; the pending code then stays
    /// as it was.
    pub fn start(&mut self, now: u64) -> Result<PairingCode, PairingError> {
        self.prune(now);
        let mut bytes = [0_u8; CODE_BYTES];
        self.port.random(&mut bytes).map_err(PairingError::Random)?;
        let value = u32::from_le_bytes(bytes) % 1_000_000;
        let code = PairingCode {
            code: format!("{value:06}"),
            expires_at: now.saturating_add(CODE_TTL_SECS),
        };
        self.pending = Some(code.clone());
        Ok(code)
    }

    /// Claims the pending code with `code` at `now` for the requested `role`, and returns the new
    /// token once the file holds it.
    ///
    /// # Errors
    ///
    /// [`PairingError::InvalidCode`] without a pending code, for an expired code and for a wrong
    /// one, which keeps the pending code; [`PairingError::LimitReached`] while the enabled tokens
    /// are at the port's limit; [`PairingError::Random`], [`PairingError::Unreadable`] or
    /// [`PairingError::Save`] when the token cannot be drawn or written. No refusal stores a
    /// token, and every refusal after the right code has taken the pending code.
    pub fn claim(
        &mut self,
        code: &str,
        role: Option<AccessRole>,
        now: u64,
    ) -> Result<String, PairingError> {
        self.prune(now);
        let pending = self.pending.take().ok_or(PairingError::InvalidCode)?;
        if pending.expires_at < now {
            return Err(PairingError::InvalidCode);
        }
        if !constant_time_eq(&pending.code, code.trim()) {
            self.pending = Some(pending);
            return Err(PairingError::InvalidCode);
        }
        let max = self.port.max_tokens();
        if self.enabled_count() >= max {
            return Err(PairingError::LimitReached { max });
        }
        let mut bytes = [0_u8; TOKEN_BYTES];
        self.port.random(&mut bytes).map_err(PairingError::Random)?;
        let token = URL_SAFE_NO_PAD.encode(bytes);
        let prefix: String = token.chars().take(8).collect();
        // Two claims of one second whose tokens begin alike would share an id; the later one is
        // numbered, so that each id names one token.
        let mut id = format!("pair-{now}-{prefix}");
        let mut number = 1_u32;
        while self.file.tokens.iter().any(|row| row.id == id) {
            number += 1;
            id = format!("pair-{now}-{prefix}-{number}");
        }
        let row = PairingToken {
            id,
            token: token.clone(),
            created_at: now,
            enabled: true,
            role: granted_role(role),
            expires_at: now.saturating_add(TOKEN_TTL_SECS),
            service: None,
        };
        self.commit(|rows| rows.push(row))?;
        Ok(token)
    }

    /// Creates the service token of the service `name` at `now` with the requested `role`, Viewer
    /// when none is requested and Engineer for an Admin request, and returns it once the file holds
    /// it. A service token that the name, compared without case, has already is replaced in its
    /// place, so that it no longer validates.
    ///
    /// # Errors
    ///
    /// [`PairingError::InvalidServiceName`] for a name that is not an identifier;
    /// [`PairingError::LimitReached`] when the token would enable one token more than the port's
    /// limit allows; [`PairingError::Random`], [`PairingError::Unreadable`],
    /// [`PairingError::Save`] or [`PairingError::NoRoom`] when the token cannot be drawn or
    /// written. No refusal changes a token.
    pub fn service(
        &mut self,
        name: &str,
        role: Option<AccessRole>,
        now: u64,
    ) -> Result<ServiceToken, PairingError> {
        if !is_service_name(name) {
            return Err(PairingError::InvalidServiceName);
        }
        self.prune(now);
        let earlier = self.file.tokens.iter().position(|row| {
            row.service
                .as_deref()
                .is_some_and(|service| service.eq_ignore_ascii_case(name))
        });
        let enables_one_more = earlier.is_none_or(|index| !self.file.tokens[index].enabled);
        let max = self.port.max_tokens();
        if enables_one_more && self.enabled_count() >= max {
            return Err(PairingError::LimitReached { max });
        }
        let mut bytes = [0_u8; TOKEN_BYTES];
        self.port.random(&mut bytes).map_err(PairingError::Random)?;
        let created = ServiceToken {
            id: format!("service-{name}"),
            role: granted_role(Some(role.unwrap_or(AccessRole::Viewer))),
            token: URL_SAFE_NO_PAD.encode(bytes),
        };
        let row = PairingToken {
            id: created.id.clone(),
            token: created.token.clone(),
            created_at: now,
            enabled: true,
            role: created.role,
            expires_at: 0,
            service: Some(name.into()),
        };
        self.commit(|rows| match earlier {
            Some(index) => rows[index] = row,
            None => rows.push(row),
        })?;
        Ok(created)
    }

    /// The role `token` grants at `now`: the stored role of an enabled, unexpired row whose token
    /// has exactly these bytes, or `None`.
    #[must_use]
    pub fn validate(&mut self, token: &str, now: u64) -> Option<AccessRole> {
        self.prune(now);
        self.file
            .tokens
            .iter()
            .find(|row| row.enabled && constant_time_eq(&row.token, token))
            .map(|row| row.role)
    }

    /// The tokens at `now` in storage order, each without its secret.
    #[must_use]
    pub fn list(&mut self, now: u64) -> Vec<PairingSummary> {
        self.prune(now);
        self.file.tokens.iter().map(PairingToken::summary).collect()
    }

    /// Disables the token with exactly the id `id` at `now`, and says whether a token has it. A
    /// token that is disabled already stays so, and the answer is still true.
    ///
    /// # Errors
    ///
    /// [`PairingError::Unreadable`] or [`PairingError::Save`] when the change cannot be written;
    /// the token then stays enabled.
    pub fn revoke(&mut self, id: &str, now: u64) -> Result<bool, PairingError> {
        self.prune(now);
        if !self.file.tokens.iter().any(|row| row.id == id) {
            return Ok(false);
        }
        if self
            .file
            .tokens
            .iter()
            .any(|row| row.id == id && row.enabled)
        {
            self.commit(|rows| {
                for row in rows.iter_mut().filter(|row| row.id == id) {
                    row.enabled = false;
                }
            })?;
        }
        Ok(true)
    }

    /// Disables every enabled token at `now`, and returns how many changed from enabled to
    /// disabled.
    ///
    /// # Errors
    ///
    /// [`PairingError::Unreadable`] or [`PairingError::Save`] when the change cannot be written;
    /// every token then stays as it was.
    pub fn revoke_all(&mut self, now: u64) -> Result<usize, PairingError> {
        self.prune(now);
        let count = self.enabled_count();
        if count > 0 {
            self.commit(|rows| {
                for row in rows {
                    row.enabled = false;
                }
            })?;
        }
        Ok(count)
    }

    fn enabled_count(&self) -> usize {
        self.file.tokens.iter().filter(|row| row.enabled).count()
    }

    /// Removes the rows that expired before `now`, and writes the file without them when the
    /// file was readable. A service token does not expire.
    fn prune(&mut self, now: u64) {
        let before = self.file.tokens.len();
        self.file
            .tokens
            .retain(|row| row.expiry().is_none_or(|expires_at| expires_at >= now));
        if self.file.tokens.len() != before && self.unreadable.is_none() {
            // An expired row grants nothing, so a write that fails only leaves it in the file
            // until a later write or open removes it.
            let _ = self.port.save(&self.file);
        }
    }

    /// Applies `change` to the rows and writes the file, after removing disabled rows, the oldest
    /// first, while the storage cannot hold it. A file that was unreadable, a file that does not fit
    /// even without its disabled rows, or a write that fails leaves the rows as they were.
    fn commit(&mut self, change: impl FnOnce(&mut Vec<PairingToken>)) -> Result<(), PairingError> {
        if let Some(error) = &self.unreadable {
            return Err(PairingError::Unreadable(error.clone()));
        }
        let before = self.file.tokens.clone();
        change(&mut self.file.tokens);
        while !self.port.fits(&self.file) {
            let Some(oldest) = self.file.tokens.iter().position(|row| !row.enabled) else {
                self.file.tokens = before;
                return Err(PairingError::NoRoom);
            };
            self.file.tokens.remove(oldest);
        }
        if let Err(error) = self.port.save(&self.file) {
            self.file.tokens = before;
            return Err(PairingError::Save(error));
        }
        Ok(())
    }
}

impl PairingToken {
    /// The last second at which the token grants its role, `None` for a service token.
    fn expiry(&self) -> Option<u64> {
        self.service.is_none().then_some(self.expires_at)
    }

    fn summary(&self) -> PairingSummary {
        PairingSummary {
            id: self.id.clone(),
            enabled: self.enabled,
            created_at: self.created_at,
            expires_at: self.expiry(),
            role: self.role,
            service: self.service.clone(),
            tail: mask_tail(&self.token),
        }
    }
}

/// The pairings of a runtime as the shared `pair.*` requests and its access check reach them:
/// [`Pairings`] behind the platform's own lock. Each method does what the method of the same name
/// on [`Pairings`] does, at `now` in Unix seconds from the platform's wall clock.
pub trait PairingService {
    /// Starts a pairing, as [`Pairings::start`].
    ///
    /// # Errors
    ///
    /// As [`Pairings::start`], and [`PairingError::Unavailable`] when the pairings cannot be
    /// reached.
    fn start(&self, now: u64) -> Result<PairingCode, PairingError>;

    /// Claims the pending code, as [`Pairings::claim`].
    ///
    /// # Errors
    ///
    /// As [`Pairings::claim`], and [`PairingError::Unavailable`] when the pairings cannot be
    /// reached.
    fn claim(&self, code: &str, role: Option<AccessRole>, now: u64)
        -> Result<String, PairingError>;

    /// Creates or replaces a service token, as [`Pairings::service`].
    ///
    /// # Errors
    ///
    /// As [`Pairings::service`], and [`PairingError::Unavailable`] when the pairings cannot be
    /// reached.
    fn service(
        &self,
        name: &str,
        role: Option<AccessRole>,
        now: u64,
    ) -> Result<ServiceToken, PairingError>;

    /// The role `token` grants, as [`Pairings::validate`]; `None` when the pairings cannot be
    /// reached.
    fn validate(&self, token: &str, now: u64) -> Option<AccessRole>;

    /// The tokens without their secrets, as [`Pairings::list`]; none when the pairings cannot be
    /// reached.
    fn list(&self, now: u64) -> Vec<PairingSummary>;

    /// Disables one token, as [`Pairings::revoke`].
    ///
    /// # Errors
    ///
    /// As [`Pairings::revoke`], and [`PairingError::Unavailable`] when the pairings cannot be
    /// reached.
    fn revoke(&self, id: &str, now: u64) -> Result<bool, PairingError>;

    /// Disables every enabled token, as [`Pairings::revoke_all`].
    ///
    /// # Errors
    ///
    /// As [`Pairings::revoke_all`], and [`PairingError::Unavailable`] when the pairings cannot
    /// be reached.
    fn revoke_all(&self, now: u64) -> Result<usize, PairingError>;
}

/// The pairings of a runtime that answers one request at a time, as a chip's loop does. A borrow
/// that is taken already cannot reach them.
impl<P: PairingPort> PairingService for RefCell<Pairings<P>> {
    fn start(&self, now: u64) -> Result<PairingCode, PairingError> {
        self.try_borrow_mut()
            .map_err(|_| PairingError::Unavailable)?
            .start(now)
    }

    fn claim(
        &self,
        code: &str,
        role: Option<AccessRole>,
        now: u64,
    ) -> Result<String, PairingError> {
        self.try_borrow_mut()
            .map_err(|_| PairingError::Unavailable)?
            .claim(code, role, now)
    }

    fn service(
        &self,
        name: &str,
        role: Option<AccessRole>,
        now: u64,
    ) -> Result<ServiceToken, PairingError> {
        self.try_borrow_mut()
            .map_err(|_| PairingError::Unavailable)?
            .service(name, role, now)
    }

    fn validate(&self, token: &str, now: u64) -> Option<AccessRole> {
        self.try_borrow_mut().ok()?.validate(token, now)
    }

    fn list(&self, now: u64) -> Vec<PairingSummary> {
        match self.try_borrow_mut() {
            Ok(mut pairings) => pairings.list(now),
            Err(_) => Vec::new(),
        }
    }

    fn revoke(&self, id: &str, now: u64) -> Result<bool, PairingError> {
        self.try_borrow_mut()
            .map_err(|_| PairingError::Unavailable)?
            .revoke(id, now)
    }

    fn revoke_all(&self, now: u64) -> Result<usize, PairingError> {
        self.try_borrow_mut()
            .map_err(|_| PairingError::Unavailable)?
            .revoke_all(now)
    }
}

/// The role of a row that names none.
fn default_role() -> AccessRole {
    AccessRole::Operator
}

/// The role a claim stores: the requested one, Operator when none is requested, and Engineer for
/// an Admin request.
fn granted_role(requested: Option<AccessRole>) -> AccessRole {
    match requested.unwrap_or(AccessRole::Operator) {
        AccessRole::Admin => AccessRole::Engineer,
        role => role,
    }
}

/// Whether `name` is a service's name: an identifier, as `trust.toml` names a service (specification
/// 48 RU-63), an ASCII letter or underscore and then ASCII letters, digits and underscores.
pub(crate) fn is_service_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
}

/// `…` and the last four characters of `token`, or all of a shorter one.
fn mask_tail(token: &str) -> String {
    let start = token
        .char_indices()
        .rev()
        .nth(3)
        .map_or(0, |(index, _)| index);
    let tail = &token[start..];
    format!("…{tail}")
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod service_tests;

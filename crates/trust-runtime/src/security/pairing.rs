//! Pairing token storage for authenticated runtime access.

#![allow(missing_docs)]

use std::fs;
use std::io;
#[cfg(unix)]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::TryRng;
use serde::{Deserialize, Serialize};

use crate::security::AccessRole;

const PAIRING_CODE_TTL_SECS: u64 = 300;
const PAIRING_TOKEN_TTL_SECS: u64 = 30 * 24 * 60 * 60;
const PAIRING_MAX_TOKENS: usize = 256;
const TOKEN_BYTES: usize = 32;

#[derive(Debug, Clone)]
pub struct PairingCode {
    pub code: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PairingSummary {
    pub id: String,
    pub enabled: bool,
    pub created_at: u64,
    pub expires_at: u64,
    pub role: AccessRole,
    pub tail: String,
}

pub struct PairingStore {
    path: PathBuf,
    state: Mutex<PairingState>,
    now: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl std::fmt::Debug for PairingStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PairingStore")
            .field("path", &self.path)
            .finish()
    }
}

#[derive(Debug, Default)]
struct PairingState {
    tokens: Vec<PairingToken>,
    pending: Option<PendingCode>,
}

#[derive(Debug, Clone)]
struct PendingCode {
    code: String,
    expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PairingToken {
    id: String,
    token: String,
    created_at: u64,
    enabled: bool,
    #[serde(default = "default_token_role")]
    role: AccessRole,
    #[serde(default)]
    expires_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PairingFile {
    tokens: Vec<PairingToken>,
}

impl PairingStore {
    #[must_use]
    pub fn load(path: PathBuf) -> Self {
        Self::with_clock(path, Arc::new(now_secs))
    }

    #[must_use]
    pub fn with_clock(path: PathBuf, now: Arc<dyn Fn() -> u64 + Send + Sync>) -> Self {
        let current_time = now();
        let mut tokens = load_tokens(&path).unwrap_or_default();
        normalize_loaded_tokens(&mut tokens, current_time);
        let state = PairingState {
            tokens,
            pending: None,
        };
        Self {
            path,
            state: Mutex::new(state),
            now,
        }
    }

    pub fn start_pairing(&self) -> PairingCode {
        let now = (self.now)();
        let code = generate_code();
        let pending = PendingCode {
            code: code.clone(),
            expires_at: now + PAIRING_CODE_TTL_SECS,
        };
        if let Ok(mut guard) = self.state.lock() {
            let changed = prune_expired_tokens(&mut guard.tokens, now);
            if changed {
                let _ = save_tokens(&self.path, &guard.tokens);
            }
            guard.pending = Some(pending.clone());
        }
        PairingCode {
            code,
            expires_at: pending.expires_at,
        }
    }

    pub fn claim(&self, code: &str, requested_role: Option<AccessRole>) -> Option<String> {
        let now = (self.now)();
        let mut guard = self.state.lock().ok()?;
        prune_expired_tokens(&mut guard.tokens, now);
        let pending = guard.pending.take()?;
        if pending.expires_at < now {
            return None;
        }
        if pending.code != code.trim() {
            guard.pending = Some(pending);
            return None;
        }
        if guard.tokens.iter().filter(|token| token.enabled).count() >= PAIRING_MAX_TOKENS {
            return None;
        }
        let role = sanitize_requested_role(requested_role);
        let token = generate_token();
        let id = format!("pair-{}", now);
        guard.tokens.push(PairingToken {
            id,
            token: token.clone(),
            created_at: now,
            enabled: true,
            role,
            expires_at: now + PAIRING_TOKEN_TTL_SECS,
        });
        let _ = save_tokens(&self.path, &guard.tokens);
        Some(token)
    }

    pub fn validate(&self, token: &str) -> bool {
        self.validate_with_role(token).is_some()
    }

    pub fn validate_with_role(&self, token: &str) -> Option<AccessRole> {
        let now = (self.now)();
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(_) => return None,
        };
        let changed = prune_expired_tokens(&mut guard.tokens, now);
        let role = guard
            .tokens
            .iter()
            .find(|entry| entry.enabled && entry.token == token)
            .map(|entry| entry.role);
        if changed {
            let _ = save_tokens(&self.path, &guard.tokens);
        }
        role
    }

    pub fn list(&self) -> Vec<PairingSummary> {
        let now = (self.now)();
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(_) => return Vec::new(),
        };
        let changed = prune_expired_tokens(&mut guard.tokens, now);
        let list = guard
            .tokens
            .iter()
            .map(|entry| PairingSummary {
                id: entry.id.clone(),
                enabled: entry.enabled,
                created_at: entry.created_at,
                expires_at: entry.expires_at,
                role: entry.role,
                tail: mask_tail(&entry.token),
            })
            .collect();
        if changed {
            let _ = save_tokens(&self.path, &guard.tokens);
        }
        list
    }

    pub fn revoke(&self, id: &str) -> bool {
        let now = (self.now)();
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        prune_expired_tokens(&mut guard.tokens, now);
        let mut changed = false;
        for token in guard.tokens.iter_mut() {
            if token.id == id {
                token.enabled = false;
                changed = true;
            }
        }
        if changed {
            let _ = save_tokens(&self.path, &guard.tokens);
        }
        changed
    }

    pub fn revoke_all(&self) -> usize {
        let now = (self.now)();
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(_) => return 0,
        };
        prune_expired_tokens(&mut guard.tokens, now);
        let mut count = 0;
        for token in guard.tokens.iter_mut() {
            if token.enabled {
                token.enabled = false;
                count += 1;
            }
        }
        if count > 0 {
            let _ = save_tokens(&self.path, &guard.tokens);
        }
        count
    }
}

fn generate_code() -> String {
    let mut buf = [0u8; 4];
    let mut rng = rand::rngs::SysRng;
    let _ = rng.try_fill_bytes(&mut buf);
    let value = u32::from_le_bytes(buf) % 1_000_000;
    format!("{value:06}")
}

fn generate_token() -> String {
    let mut buf = [0u8; TOKEN_BYTES];
    let mut rng = rand::rngs::SysRng;
    let _ = rng.try_fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

fn mask_tail(token: &str) -> String {
    let tail = token.chars().rev().take(4).collect::<String>();
    format!("…{}", tail.chars().rev().collect::<String>())
}

fn load_tokens(path: &Path) -> io::Result<Vec<PairingToken>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read_to_string(path)?;
    let file: PairingFile = serde_json::from_str(&data).unwrap_or_default();
    Ok(file.tokens)
}

fn save_tokens(path: &Path, tokens: &[PairingToken]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = PairingFile {
        tokens: tokens.to_vec(),
    };
    let data = serde_json::to_vec_pretty(&file).unwrap_or_default();
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(&data)?;
        file.sync_all()?;
        let mut perms = file.metadata()?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path, perms)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        fs::write(path, data)
    }
}

fn default_token_role() -> AccessRole {
    AccessRole::Operator
}

fn sanitize_requested_role(role: Option<AccessRole>) -> AccessRole {
    match role.unwrap_or(AccessRole::Operator) {
        AccessRole::Admin => AccessRole::Engineer,
        other => other,
    }
}

fn normalize_loaded_tokens(tokens: &mut [PairingToken], now: u64) {
    for token in tokens {
        if token.expires_at == 0 {
            let candidate = token.created_at.saturating_add(PAIRING_TOKEN_TTL_SECS);
            token.expires_at = if candidate <= now { now + 1 } else { candidate };
        }
    }
}

fn prune_expired_tokens(tokens: &mut Vec<PairingToken>, now: u64) -> bool {
    let before = tokens.len();
    tokens.retain(|entry| entry.expires_at >= now);
    before != tokens.len()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!("trust-pairing-{name}"));
        dir
    }

    #[test]
    fn pairing_claim_cycle() {
        let path = temp_file("cycle.json");
        let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1000));
        let code = store.start_pairing();
        let token = store.claim(&code.code, None);
        assert!(token.is_some());
        assert!(store.validate(token.as_ref().unwrap()));
        let list = store.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].role, AccessRole::Operator);
        assert!(list[0].expires_at > list[0].created_at);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn pairing_expiry_rejects() {
        let path = temp_file("expiry.json");
        let clock = Arc::new(std::sync::atomic::AtomicU64::new(1000));
        let clock_fn = {
            let clock = clock.clone();
            Arc::new(move || clock.load(std::sync::atomic::Ordering::SeqCst))
        };
        let store = PairingStore::with_clock(path.clone(), clock_fn);
        let code = store.start_pairing();
        clock.store(
            1000 + PAIRING_CODE_TTL_SECS + 1,
            std::sync::atomic::Ordering::SeqCst,
        );
        let token = store.claim(&code.code, None);
        assert!(token.is_none());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn pairing_token_expiry_disables_old_token() {
        let path = temp_file("token-expiry.json");
        let clock = Arc::new(std::sync::atomic::AtomicU64::new(1000));
        let clock_fn = {
            let clock = clock.clone();
            Arc::new(move || clock.load(std::sync::atomic::Ordering::SeqCst))
        };
        let store = PairingStore::with_clock(path.clone(), clock_fn);
        let code = store.start_pairing();
        let token = store.claim(&code.code, Some(AccessRole::Viewer));
        let token = token.expect("pair token");
        assert_eq!(store.validate_with_role(&token), Some(AccessRole::Viewer));
        clock.store(
            1000 + PAIRING_TOKEN_TTL_SECS + 2,
            std::sync::atomic::Ordering::SeqCst,
        );
        assert_eq!(store.validate_with_role(&token), None);
        let _ = fs::remove_file(path);
    }
}

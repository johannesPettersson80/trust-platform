//! The computer's pairing store (specification 11 §6.9.6): the shared pairing rules of
//! `trust_runtime_services::access::pairing` over the bundle's `pairings.json`, which is read whole
//! and replaced whole through a temporary file with mode 0600 on Unix, and the operating system's
//! random numbers.

#![allow(missing_docs)]

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use rand::TryRng;
pub use trust_runtime_services::access::pairing::{PairingCode, PairingSummary};
use trust_runtime_services::access::pairing::{
    PairingError, PairingFile, PairingPort, PairingService, Pairings,
};

use crate::deployment::program_store::publish_atomic;
use crate::security::AccessRole;

/// The pairings of one runtime, which its control and web surfaces share. The methods of the
/// store itself take their time from the store's clock; the shared `pair.*` requests reach it
/// through [`PairingService`] with the time of the runtime's wall clock.
pub struct PairingStore {
    path: PathBuf,
    pairings: Mutex<Pairings<FilePort>>,
    now: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl std::fmt::Debug for PairingStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PairingStore")
            .field("path", &self.path)
            .finish()
    }
}

impl PairingStore {
    /// Opens the pairings of the file at `path` at the system clock.
    #[must_use]
    pub fn load(path: PathBuf) -> Self {
        Self::with_clock(path, Arc::new(now_secs))
    }

    /// Opens the pairings of the file at `path`; `now` gives the time, in Unix seconds, of the
    /// store's own methods.
    #[must_use]
    pub fn with_clock(path: PathBuf, now: Arc<dyn Fn() -> u64 + Send + Sync>) -> Self {
        let pairings = Pairings::open(FilePort { path: path.clone() }, now());
        Self {
            path,
            pairings: Mutex::new(pairings),
            now,
        }
    }

    /// Starts a pairing: a new six-digit code, valid for 300 seconds. A start that fails, because
    /// the operating system gives no random numbers or the store's lock is poisoned, is logged and
    /// answers an empty code, which no claim takes.
    pub fn start_pairing(&self) -> PairingCode {
        let now = (self.now)();
        PairingService::start(self, now).unwrap_or_else(|error| {
            tracing::warn!("pairing did not start: {error}");
            PairingCode {
                code: String::new(),
                expires_at: now,
            }
        })
    }

    /// Claims the pending code for `requested_role` and returns the new token, or `None` when the
    /// claim is refused or the token cannot be written.
    pub fn claim(&self, code: &str, requested_role: Option<AccessRole>) -> Option<String> {
        PairingService::claim(self, code, requested_role, (self.now)()).ok()
    }

    pub fn validate(&self, token: &str) -> bool {
        self.validate_with_role(token).is_some()
    }

    /// The role `token` grants, or `None`.
    pub fn validate_with_role(&self, token: &str) -> Option<AccessRole> {
        PairingService::validate(self, token, (self.now)())
    }

    /// The tokens without their secrets.
    pub fn list(&self) -> Vec<PairingSummary> {
        PairingService::list(self, (self.now)())
    }

    /// Disables the token `id`; false when no token has the id or the change cannot be written.
    pub fn revoke(&self, id: &str) -> bool {
        PairingService::revoke(self, id, (self.now)()).unwrap_or(false)
    }

    /// Disables every enabled token and returns how many changed; 0 when the change cannot be
    /// written, since nothing then changed.
    pub fn revoke_all(&self) -> usize {
        PairingService::revoke_all(self, (self.now)()).unwrap_or(0)
    }

    /// The time of the store's clock, in Unix seconds, at which its own methods act.
    #[must_use]
    pub fn now_secs(&self) -> u64 {
        (self.now)()
    }

    /// The pairings behind the lock; a poisoned lock reaches nothing.
    fn locked(&self) -> Result<MutexGuard<'_, Pairings<FilePort>>, PairingError> {
        self.pairings.lock().map_err(|_| PairingError::Unavailable)
    }
}

impl PairingService for PairingStore {
    fn start(&self, now: u64) -> Result<PairingCode, PairingError> {
        self.locked()?.start(now)
    }

    fn claim(
        &self,
        code: &str,
        role: Option<AccessRole>,
        now: u64,
    ) -> Result<String, PairingError> {
        self.locked()?.claim(code, role, now)
    }

    fn validate(&self, token: &str, now: u64) -> Option<AccessRole> {
        self.locked().ok()?.validate(token, now)
    }

    fn list(&self, now: u64) -> Vec<PairingSummary> {
        match self.locked() {
            Ok(mut pairings) => pairings.list(now),
            Err(_) => Vec::new(),
        }
    }

    fn revoke(&self, id: &str, now: u64) -> Result<bool, PairingError> {
        self.locked()?.revoke(id, now)
    }

    fn revoke_all(&self, now: u64) -> Result<usize, PairingError> {
        self.locked()?.revoke_all(now)
    }
}

/// The computer's pairing file, read and written whole.
struct FilePort {
    path: PathBuf,
}

impl PairingPort for FilePort {
    /// An absent file is an empty one. A file that cannot be read, or is not a pairing file, is an
    /// error: it grants no token and is kept for its owner to repair.
    fn load(&mut self) -> Result<PairingFile, String> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(PairingFile::default());
            }
            Err(error) => {
                return Err(format!(
                    "read pairing file '{}': {error}",
                    self.path.display()
                ));
            }
        };
        serde_json::from_str(&text)
            .map_err(|error| format!("parse pairing file '{}': {error}", self.path.display()))
    }

    fn save(&mut self, file: &PairingFile) -> Result<(), String> {
        write_file(&self.path, file)
            .map_err(|error| format!("write pairing file '{}': {error}", self.path.display()))
    }

    fn random(&mut self, bytes: &mut [u8]) -> Result<(), String> {
        let mut rng = rand::rngs::SysRng;
        rng.try_fill_bytes(bytes)
            .map_err(|error| format!("the operating system gave no random numbers: {error}"))
    }
}

/// Writes `file` as pretty JSON through a temporary file beside `path`, synced and renamed over
/// it, so that a reader or a power cut finds the previous file or the whole new one
/// (specification 11 §6.9.6). On Unix the temporary file is made 0600 before a token is written
/// into it, and the replaced file keeps that mode.
fn write_file(path: &Path, file: &PairingFile) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_vec_pretty(file).map_err(io::Error::other)?;
    publish_atomic(path, |out| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            out.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        out.write_all(&data)
    })
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// The pairing rules and their tests are shared runtime code
// (`trust_runtime_services::access::pairing`); these tests prove the computer's file.
#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::json;

    use super::*;

    static NEXT_FOLDER: AtomicU64 = AtomicU64::new(1);

    /// A pairing file in a folder of its own, which is removed when the test ends.
    struct TempFile {
        path: PathBuf,
    }

    impl TempFile {
        fn new(label: &str) -> Self {
            let folder = std::env::temp_dir().join(format!(
                "trust-pairing-file-{}-{label}-{}",
                std::process::id(),
                NEXT_FOLDER.fetch_add(1, Ordering::Relaxed)
            ));
            Self {
                path: folder.join("pairings.json"),
            }
        }

        fn folder(&self) -> &Path {
            self.path.parent().expect("the file's folder")
        }

        fn write(&self, text: &str) {
            fs::create_dir_all(self.folder()).expect("create the folder");
            fs::write(&self.path, text).expect("write the pairing file");
        }

        fn read(&self) -> String {
            fs::read_to_string(&self.path).expect("read the pairing file")
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.folder());
        }
    }

    /// The store of `file` at 1,000 s.
    fn store(file: &TempFile) -> PairingStore {
        PairingStore::with_clock(file.path.clone(), Arc::new(|| 1_000))
    }

    /// A file in the computer's format loads: a row without `role` grants Operator and a row
    /// without `expires_at` expires 30 days after `created_at` (100 + 2,592,000 = 2,592,100). A
    /// claim at 1,000 writes it back as pretty JSON with every member, its own row expiring at
    /// 1,000 + 2,592,000 = 2,593,000. Catches a file written before that no longer loads, and a
    /// write in another shape.
    #[test]
    fn a_file_in_the_computers_format_is_read_and_written_back_in_it() {
        let file = TempFile::new("format");
        file.write(
            r#"{
  "tokens": [
    {"id": "pair-100-legacyAA", "token": "legacy-token", "created_at": 100, "enabled": true},
    {"id": "pair-900-viewerAA", "token": "viewer-token", "created_at": 900, "enabled": false,
     "role": "viewer", "expires_at": 5000}
  ]
}"#,
        );
        let store = store(&file);
        assert_eq!(
            store.validate_with_role("legacy-token"),
            Some(AccessRole::Operator)
        );
        assert_eq!(store.validate_with_role("viewer-token"), None);
        let code = store.start_pairing();
        let token = store
            .claim(&code.code, Some(AccessRole::Engineer))
            .expect("claim");

        let text = file.read();
        assert!(
            text.starts_with("{\n  \"tokens\": [\n    {\n      \"id\": "),
            "{text}"
        );
        let written: serde_json::Value = serde_json::from_str(&text).expect("JSON");
        assert_eq!(
            written,
            json!({"tokens": [
                {"id": "pair-100-legacyAA", "token": "legacy-token", "created_at": 100,
                 "enabled": true, "role": "operator", "expires_at": 2_592_100},
                {"id": "pair-900-viewerAA", "token": "viewer-token", "created_at": 900,
                 "enabled": false, "role": "viewer", "expires_at": 5_000},
                {"id": format!("pair-1000-{}", &token[..8]), "token": token, "created_at": 1_000,
                 "enabled": true, "role": "engineer", "expires_at": 2_593_000},
            ]})
        );
    }

    /// An absent file is an empty store, and the first claim creates its folder and the file.
    /// Catches an absent file refused as unreadable.
    #[test]
    fn an_absent_file_is_an_empty_store_until_a_claim_writes_it() {
        let file = TempFile::new("absent");
        let store = store(&file);
        assert!(store.list().is_empty());
        assert!(!file.path.exists());
        let code = store.start_pairing();
        let token = store.claim(&code.code, None).expect("claim");
        assert!(store.validate(&token));
        let written: serde_json::Value = serde_json::from_str(&file.read()).expect("JSON");
        assert_eq!(written["tokens"].as_array().map(Vec::len), Some(1));
    }

    /// The pairing file is replaced with mode 0600, whatever the mode of the file before, and no
    /// temporary file is left beside it. Catches a credential file readable by other users.
    #[cfg(unix)]
    #[test]
    fn the_file_is_replaced_with_mode_0600() {
        use std::os::unix::fs::PermissionsExt;

        let file = TempFile::new("mode");
        file.write(r#"{"tokens": []}"#);
        fs::set_permissions(&file.path, fs::Permissions::from_mode(0o644)).expect("chmod");
        let store = store(&file);
        let code = store.start_pairing();
        assert!(store.claim(&code.code, None).is_some());

        let mode = fs::metadata(&file.path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        let names: Vec<_> = fs::read_dir(file.folder())
            .expect("read the folder")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        assert_eq!(names, [std::ffi::OsString::from("pairings.json")]);
    }

    /// A malformed file grants no token, and a claim neither stores a token nor replaces the file:
    /// it answers the file's error. Catches tokens granted from a broken file, and a claim that
    /// overwrites it with only its own token.
    #[test]
    fn a_malformed_file_grants_nothing_and_is_kept() {
        let file = TempFile::new("malformed");
        file.write("{ definitely not json");
        let store = store(&file);
        assert!(!store.validate("anything"));
        assert!(store.list().is_empty());

        let code = store.start_pairing();
        assert_eq!(code.code.len(), 6);
        let refused = PairingService::claim(&store, &code.code, None, 1_000);
        let prefix = format!("parse pairing file '{}': ", file.path.display());
        assert!(
            matches!(&refused, Err(PairingError::Unreadable(text)) if text.starts_with(&prefix)),
            "{refused:?}"
        );
        assert_eq!(file.read(), "{ definitely not json");
    }

    /// A file that cannot be written, here because a plain file stands where its folder should be
    /// once the store has opened the absent file, fails the claim, which then grants nothing.
    /// Catches a write error that the file port swallows, so that a token would grant a role that
    /// no file holds.
    #[test]
    fn a_claim_whose_file_cannot_be_written_grants_nothing() {
        let file = TempFile::new("unwritable");
        let store = store(&file);
        let folder = file.folder().to_path_buf();
        fs::write(&folder, b"not a folder").expect("block the folder");

        let code = store.start_pairing();
        let refused = PairingService::claim(&store, &code.code, None, 1_000);
        let prefix = format!("write pairing file '{}': ", file.path.display());
        assert!(
            matches!(&refused, Err(PairingError::Save(text)) if text.starts_with(&prefix)),
            "{refused:?}"
        );
        assert!(store.list().is_empty());
        let _ = fs::remove_file(&folder);
    }

    /// The store's debug form names its file and never a code or a token.
    #[test]
    fn the_debug_form_shows_the_path_and_no_secret() {
        let file = TempFile::new("debug");
        let store = store(&file);
        let code = store.start_pairing();
        let token = store.claim(&code.code, None).expect("claim");

        let debug = format!("{store:?}");
        assert!(debug.contains("PairingStore") && debug.contains("path"));
        assert!(!debug.contains(&code.code));
        assert!(!debug.contains(&token));
    }
}

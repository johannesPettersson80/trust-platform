//! The pairing rules of specification 11 §6.9.6 over a fake port: an in-memory file, scripted
//! random bytes, and a load or a save that fails on request. Every expected value is worked out by
//! hand from the specification; the derivations are in the tests' comments.
//!
//! Two tokens recur. 32 bytes 0x00 are 256 zero bits: 42 base64 digits of six bits and one of the
//! last four bits padded with two zero bits, each `A`, so 43 `A` without padding. 32 bytes 0xFF are
//! 42 digits 0b111111 = 63, `_` in the URL-safe alphabet, and 0b111100 = 60, `8`.

use alloc::collections::VecDeque;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::{RefCell, RefMut};

use serde_json::json;

use super::*;

/// The clock at which the tests start, in Unix seconds.
const T0: u64 = 1_000;

/// When a token claimed at [`T0`] expires: 1,000 + 30 × 86,400 = 2,593,000.
const T0_TOKEN_EXPIRY: u64 = 2_593_000;

/// The token of 32 bytes 0x00.
fn zeros() -> String {
    "A".repeat(43)
}

/// The token of 32 bytes 0xFF.
fn ones() -> String {
    format!("{}8", "_".repeat(42))
}

/// What the fake port holds. The test keeps a handle on it after the port moved into the
/// pairings.
struct Store {
    /// The file as stored: `load` reads it, a successful `save` replaces it.
    file: PairingFile,
    /// The error `load` gives instead of the file.
    load_error: Option<&'static str>,
    /// The error every `save` gives while it is set.
    save_error: Option<&'static str>,
    /// How many saves succeeded.
    saves: usize,
    /// The random bytes still to give, in order; a request for more than are left fails.
    random: VecDeque<u8>,
    /// The port's limit of enabled tokens.
    max_tokens: usize,
    /// The most rows the storage holds, as a chip's record of one sector holds a bounded number.
    room: usize,
}

#[derive(Clone)]
struct FakePort(Rc<RefCell<Store>>);

impl FakePort {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(Store {
            file: PairingFile::default(),
            load_error: None,
            save_error: None,
            saves: 0,
            random: VecDeque::new(),
            max_tokens: MAX_TOKENS,
            room: usize::MAX,
        })))
    }

    fn store(&self) -> RefMut<'_, Store> {
        self.0.borrow_mut()
    }

    /// Queues the four bytes of the code drawn as `value` (little-endian).
    fn code(&self, value: u32) {
        self.store().random.extend(value.to_le_bytes());
    }

    /// Queues the 32 bytes of one token, each `byte`.
    fn token(&self, byte: u8) {
        self.store().random.extend([byte; TOKEN_BYTES]);
    }

    fn open(&self, now: u64) -> Pairings<FakePort> {
        Pairings::open(self.clone(), now)
    }
}

impl PairingPort for FakePort {
    fn load(&mut self) -> Result<PairingFile, String> {
        let store = self.0.borrow();
        match store.load_error {
            Some(error) => Err(error.to_string()),
            None => Ok(store.file.clone()),
        }
    }

    fn save(&mut self, file: &PairingFile) -> Result<(), String> {
        let mut store = self.0.borrow_mut();
        if let Some(error) = store.save_error {
            return Err(error.to_string());
        }
        store.file.clone_from(file);
        store.saves += 1;
        Ok(())
    }

    fn random(&mut self, bytes: &mut [u8]) -> Result<(), String> {
        let mut store = self.0.borrow_mut();
        if store.random.len() < bytes.len() {
            return Err("no random bytes".to_string());
        }
        for byte in bytes {
            *byte = store.random.pop_front().unwrap_or_default();
        }
        Ok(())
    }

    fn max_tokens(&self) -> usize {
        self.0.borrow().max_tokens
    }

    fn fits(&self, file: &PairingFile) -> bool {
        file.tokens.len() <= self.0.borrow().room
    }
}

/// Starts a pairing at `now` and claims it at once for `role`, with the token of 32 bytes `byte`;
/// returns the token.
fn pair(
    fake: &FakePort,
    pairings: &mut Pairings<FakePort>,
    role: Option<AccessRole>,
    byte: u8,
    now: u64,
) -> String {
    fake.code(42);
    fake.token(byte);
    let started = pairings.start(now).expect("start");
    pairings
        .claim(&started.code, role, now)
        .expect("claim the started code")
}

/// A stored row claimed at [`T0`], valid through 5,000.
fn row(id: &str, token: &str, enabled: bool) -> PairingToken {
    PairingToken {
        id: id.to_string(),
        token: token.to_string(),
        created_at: T0,
        enabled,
        role: AccessRole::Operator,
        expires_at: 5_000,
    }
}

/// A code is six decimal digits from four random bytes, valid through `start + 300` inclusive:
/// 42 is `000042`; 4,294,967,295 mod 1,000,000 is `967295`; 1,000,000 mod 1,000,000 is `000000`.
/// Catches a code without its leading zeros, a modulus other than 10^6, a lifetime other than 300
/// seconds, a code refused at its exact `expires_at`, and an expired code left claimable.
#[test]
fn a_code_is_six_digits_and_valid_through_its_three_hundredth_second() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);

    fake.code(42);
    assert_eq!(
        pairings.start(T0),
        Ok(PairingCode {
            code: "000042".into(),
            expires_at: 1_300
        })
    );
    fake.token(0x00);
    assert_eq!(pairings.claim("000042", None, 1_300), Ok(zeros()));

    fake.code(u32::MAX);
    assert_eq!(
        pairings.start(2_000),
        Ok(PairingCode {
            code: "967295".into(),
            expires_at: 2_300
        })
    );
    assert_eq!(
        pairings.claim("967295", None, 2_301),
        Err(PairingError::InvalidCode)
    );
    assert_eq!(
        pairings.claim("967295", None, 2_300),
        Err(PairingError::InvalidCode),
        "the expired claim took the code"
    );

    fake.code(1_000_000);
    assert_eq!(
        pairings.start(3_000).map(|started| started.code),
        Ok("000000".into())
    );
    assert_eq!(
        PairingError::InvalidCode.to_string(),
        "invalid or expired code"
    );
}

/// A runtime holds one pending code: starting again replaces it. Catches two codes claimable at
/// once, or the first code kept.
#[test]
fn starting_again_replaces_the_pending_code() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);
    fake.code(1);
    fake.code(2);
    assert_eq!(
        pairings.start(T0).map(|started| started.code),
        Ok("000001".into())
    );
    assert_eq!(
        pairings.start(T0).map(|started| started.code),
        Ok("000002".into())
    );

    assert_eq!(
        pairings.claim("000001", None, T0),
        Err(PairingError::InvalidCode)
    );
    fake.token(0x00);
    assert_eq!(pairings.claim("000002", None, T0), Ok(zeros()));
}

/// A wrong code does not take the pending code; the right one, with surrounding whitespace
/// trimmed, takes it once. Without a started code nothing is claimable. Catches a wrong guess that
/// burns the Admin's code, an untrimmed comparison, and a code that claims twice.
#[test]
fn a_wrong_code_keeps_the_pending_code_and_the_right_one_claims_once() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);
    assert_eq!(
        pairings.claim("000042", None, T0),
        Err(PairingError::InvalidCode)
    );

    fake.code(42);
    pairings.start(T0).expect("start");
    assert_eq!(
        pairings.claim("000043", None, T0),
        Err(PairingError::InvalidCode)
    );
    assert_eq!(pairings.claim("", None, T0), Err(PairingError::InvalidCode));
    fake.token(0x00);
    assert_eq!(pairings.claim(" \t000042\n", None, T0), Ok(zeros()));
    assert_eq!(
        pairings.claim("000042", None, T0),
        Err(PairingError::InvalidCode)
    );
}

/// The default role is Operator; Viewer, Operator and Engineer are kept; Admin becomes Engineer, in
/// the reply of the token and in the file. Catches pairing that mints an Admin token, or another
/// default role.
#[test]
fn the_default_role_is_operator_and_an_admin_request_becomes_engineer() {
    for (requested, stored) in [
        (None, AccessRole::Operator),
        (Some(AccessRole::Viewer), AccessRole::Viewer),
        (Some(AccessRole::Operator), AccessRole::Operator),
        (Some(AccessRole::Engineer), AccessRole::Engineer),
        (Some(AccessRole::Admin), AccessRole::Engineer),
    ] {
        let fake = FakePort::new();
        let mut pairings = fake.open(T0);
        let token = pair(&fake, &mut pairings, requested, 0x00, T0);
        assert_eq!(pairings.validate(&token, T0), Some(stored), "{requested:?}");
        assert_eq!(fake.store().file.tokens[0].role, stored, "{requested:?}");
    }
}

/// At the limit of enabled tokens a claim is refused with its text and writes nothing; a revoked
/// token no longer counts, so a later claim succeeds. The fake's limit is 2; the default is 256.
/// Catches a claim past the limit, a refusal that says `invalid or expired code`, and disabled
/// tokens counted against the limit.
#[test]
fn a_claim_at_the_limit_is_refused_until_a_token_is_revoked() {
    let fake = FakePort::new();
    fake.store().max_tokens = 2;
    let mut pairings = fake.open(T0);
    pair(&fake, &mut pairings, None, 0x00, T0);
    pair(&fake, &mut pairings, None, 0xFF, T0);

    fake.code(42);
    let started = pairings.start(T0).expect("start");
    let refused = pairings.claim(&started.code, None, T0);
    assert_eq!(refused, Err(PairingError::LimitReached { max: 2 }));
    assert_eq!(
        PairingError::LimitReached { max: 2 }.to_string(),
        "at most 2 pairing tokens may be enabled; revoke one first"
    );
    assert_eq!(pairings.list(T0).len(), 2);
    assert_eq!(fake.store().saves, 2, "the refused claim wrote nothing");

    assert_eq!(pairings.revoke("pair-1000-AAAAAAAA", T0), Ok(true));
    let token = pair(&fake, &mut pairings, None, 0x11, T0);
    assert_eq!(pairings.validate(&token, T0), Some(AccessRole::Operator));
    assert_eq!(pairings.list(T0).len(), 3);
}

/// The trait's limit is the specification's 256. Catches another default.
#[test]
fn a_port_keeps_256_enabled_tokens_unless_it_says_fewer() {
    struct Plain;
    impl PairingPort for Plain {
        fn load(&mut self) -> Result<PairingFile, String> {
            Ok(PairingFile::default())
        }
        fn save(&mut self, _file: &PairingFile) -> Result<(), String> {
            Ok(())
        }
        fn random(&mut self, _bytes: &mut [u8]) -> Result<(), String> {
            Ok(())
        }
    }
    assert_eq!(Plain.max_tokens(), 256);
}

/// A token claimed at 1,000 expires at 1,000 + 2,592,000 = 2,593,000: it grants its role at that
/// second, not one second later, and is then removed from the list and the file. Catches an
/// exclusive expiry, a lifetime other than 30 days, and an expired row kept in the file.
#[test]
fn a_token_is_valid_through_its_expires_at_and_removed_after() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);
    let token = pair(&fake, &mut pairings, None, 0x00, T0);

    assert_eq!(
        pairings.validate(&token, T0_TOKEN_EXPIRY),
        Some(AccessRole::Operator)
    );
    assert_eq!(pairings.validate(&token, T0_TOKEN_EXPIRY + 1), None);
    assert_eq!(pairings.list(T0_TOKEN_EXPIRY + 1), Vec::new());
    assert!(
        fake.store().file.tokens.is_empty(),
        "the file is written without the expired row"
    );
}

/// Validation needs the exact bytes of an enabled token. Catches a prefix or suffix match and an
/// empty token that validates.
#[test]
fn validation_needs_the_exact_token() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);
    let token = pair(&fake, &mut pairings, None, 0x00, T0);
    let longer = format!("{token}A");
    let other_case = "a".repeat(43);
    for presented in [&token[..42], longer.as_str(), "", other_case.as_str()] {
        assert_eq!(pairings.validate(presented, T0), None, "{presented:?}");
    }
    assert_eq!(pairings.validate(&token, T0), Some(AccessRole::Operator));
}

/// The list keeps the storage order and shows each token only as `…` and its last four
/// characters, with the ids `pair-1000-` and the first eight characters of each token. Catches a
/// list that exposes a token, a reordered list, and ids that collide within one second.
#[test]
fn the_list_keeps_the_storage_order_and_shows_only_each_tokens_tail() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);
    assert_eq!(
        pair(&fake, &mut pairings, Some(AccessRole::Viewer), 0x00, T0),
        zeros()
    );
    assert_eq!(
        pair(&fake, &mut pairings, Some(AccessRole::Engineer), 0xFF, T0),
        ones()
    );

    let listed = pairings.list(T0);
    assert_eq!(
        listed,
        vec![
            PairingSummary {
                id: "pair-1000-AAAAAAAA".into(),
                enabled: true,
                created_at: 1_000,
                expires_at: T0_TOKEN_EXPIRY,
                role: AccessRole::Viewer,
                tail: "…AAAA".into(),
            },
            PairingSummary {
                id: "pair-1000-________".into(),
                enabled: true,
                created_at: 1_000,
                expires_at: T0_TOKEN_EXPIRY,
                role: AccessRole::Engineer,
                tail: "…___8".into(),
            },
        ]
    );
    assert_eq!(
        serde_json::to_value(&listed[0]).expect("summary"),
        json!({
            "id": "pair-1000-AAAAAAAA",
            "enabled": true,
            "created_at": 1_000,
            "expires_at": 2_593_000,
            "role": "viewer",
            "tail": "…AAAA",
        })
    );
    for (token, tail) in [
        ("abcdefgh", "…efgh"),
        ("abcd", "…abcd"),
        ("abc", "…abc"),
        ("", "…"),
        ("åäö日本語", "…ö日本語"),
    ] {
        assert_eq!(mask_tail(token), tail, "{token:?}");
    }
}

/// Revoking disables only the row with exactly that id and is false for an absent id; revoking
/// all counts only the tokens that changed from enabled to disabled; disabled rows stay listed.
/// Catches a revoke that matches loosely, a count that includes tokens disabled before, and
/// revoked rows dropped from the list or left enabled in the file.
#[test]
fn revoke_disables_one_exact_id_and_revoke_all_counts_the_enabled_tokens() {
    let rows = vec![
        row("one", "token-one", true),
        row("two", "token-two", false),
        row("three", "token-three", true),
    ];

    let fake = FakePort::new();
    fake.store().file.tokens.clone_from(&rows);
    let mut pairings = fake.open(T0);
    assert_eq!(pairings.revoke("missing", T0), Ok(false));
    assert_eq!(pairings.revoke("ONE", T0), Ok(false));
    assert_eq!(pairings.revoke("one", T0), Ok(true));
    assert_eq!(pairings.validate("token-one", T0), None);
    assert_eq!(
        pairings.validate("token-three", T0),
        Some(AccessRole::Operator)
    );
    assert_eq!(
        pairings.revoke("one", T0),
        Ok(true),
        "a revoked id is still present"
    );
    assert!(!fake.store().file.tokens[0].enabled);
    assert!(fake.store().file.tokens[2].enabled);

    let fake = FakePort::new();
    fake.store().file.tokens.clone_from(&rows);
    let mut pairings = fake.open(T0);
    assert_eq!(pairings.revoke_all(T0), Ok(2));
    assert_eq!(pairings.revoke_all(T0), Ok(0));
    let listed = pairings.list(T0);
    assert_eq!(listed.len(), 3);
    assert!(listed.iter().all(|summary| !summary.enabled));
    assert!(fake
        .store()
        .file
        .tokens
        .iter()
        .all(|stored| !stored.enabled));
}

/// A row without an expiry (0) expires 30 days after its creation, saturating: created at 100 it
/// expires at 2,592,100; created at 1, at 2,592,001, so at 2,592,050 it is removed when the
/// pairings open rather than revived; created at 2^64 − 2, at 2^64 − 1. Catches a legacy row that
/// never expires, one revived until its next check, and an overflow.
#[test]
fn a_row_without_an_expiry_expires_thirty_days_after_its_creation() {
    let legacy = |id: &str, created_at: u64| PairingToken {
        id: id.to_string(),
        token: format!("token-{id}"),
        created_at,
        enabled: true,
        role: AccessRole::Operator,
        expires_at: 0,
    };
    let fake = FakePort::new();
    fake.store().file.tokens = vec![
        legacy("live", 100),
        legacy("dead", 1),
        legacy("far", u64::MAX - 1),
    ];
    let now = 2_592_050;
    let mut pairings = fake.open(now);

    assert_eq!(pairings.validate("token-dead", now), None);
    assert_eq!(
        pairings.validate("token-live", now),
        Some(AccessRole::Operator)
    );
    let expiries: Vec<(String, u64)> = pairings
        .list(now)
        .into_iter()
        .map(|summary| (summary.id, summary.expires_at))
        .collect();
    assert_eq!(
        expiries,
        vec![("live".into(), 2_592_100), ("far".into(), u64::MAX)]
    );
    let written: Vec<u64> = fake
        .store()
        .file
        .tokens
        .iter()
        .map(|stored| stored.expires_at)
        .collect();
    assert_eq!(
        written,
        vec![2_592_100, u64::MAX],
        "the open writes the file without the expired row"
    );
}

/// The file is the computer's `pairings.json`: a `tokens` array of rows with `id`, `token`,
/// `created_at`, `enabled`, `role` (Operator when absent) and `expires_at` (0 when absent); other
/// members are ignored, and a file without `tokens` or a row without `enabled` is not a pairing
/// file. Catches a renamed member, a lost default, and a change of the role's spelling.
#[test]
fn the_file_reads_and_writes_the_computers_format() {
    let file: PairingFile = serde_json::from_str(
        r#"{"tokens": [
            {"id": "legacy", "token": "legacy-token", "created_at": 100, "enabled": true},
            {"id": "pair-900-abcdefgh", "token": "t", "created_at": 900, "enabled": false,
             "role": "viewer", "expires_at": 5000, "note": "ignored"}
        ]}"#,
    )
    .expect("a file written before");
    assert_eq!(
        file.tokens,
        vec![
            PairingToken {
                id: "legacy".into(),
                token: "legacy-token".into(),
                created_at: 100,
                enabled: true,
                role: AccessRole::Operator,
                expires_at: 0,
            },
            PairingToken {
                id: "pair-900-abcdefgh".into(),
                token: "t".into(),
                created_at: 900,
                enabled: false,
                role: AccessRole::Viewer,
                expires_at: 5_000,
            },
        ]
    );
    assert_eq!(
        serde_json::to_value(file).expect("file"),
        json!({"tokens": [
            {"id": "legacy", "token": "legacy-token", "created_at": 100, "enabled": true,
             "role": "operator", "expires_at": 0},
            {"id": "pair-900-abcdefgh", "token": "t", "created_at": 900, "enabled": false,
             "role": "viewer", "expires_at": 5_000},
        ]})
    );
    assert!(serde_json::from_str::<PairingFile>("{}").is_err());
    assert!(serde_json::from_str::<PairingFile>(
        r#"{"tokens": [{"id": "x", "token": "y", "created_at": 1}]}"#
    )
    .is_err());
}

/// What a claim and a revoke write is what the next open reads: the claimed token with its role,
/// then the same token disabled. Catches authority kept only in memory, and a revocation that a
/// restart undoes.
#[test]
fn a_claim_and_a_revoke_are_read_back_after_a_reopen() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);
    let token = pair(&fake, &mut pairings, Some(AccessRole::Engineer), 0x00, T0);

    let mut reopened = fake.open(T0 + 1);
    assert_eq!(
        reopened.validate(&token, T0 + 1),
        Some(AccessRole::Engineer)
    );
    assert_eq!(reopened.revoke("pair-1000-AAAAAAAA", T0 + 1), Ok(true));

    let mut reopened = fake.open(T0 + 2);
    assert_eq!(reopened.validate(&token, T0 + 2), None);
    let listed = reopened.list(T0 + 2);
    assert_eq!(listed.len(), 1);
    assert!(!listed[0].enabled);
}

/// A save that fails leaves every token as it was: the claim answers the port's text and stores no
/// token, and a revoke or a revoke-all disables nothing; once the port writes again they succeed.
/// Catches a token that grants a role without being in the file, and a revocation reported or
/// applied that the file does not hold.
#[test]
fn a_save_that_fails_leaves_every_token_as_it_was() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);
    let kept = pair(&fake, &mut pairings, None, 0x00, T0);

    fake.store().save_error = Some("disk full");
    fake.code(42);
    fake.token(0xFF);
    let started = pairings.start(T0).expect("a start writes nothing");
    let refused = pairings.claim(&started.code, None, T0);
    assert_eq!(refused, Err(PairingError::Save("disk full".into())));
    assert_eq!(
        PairingError::Save("disk full".into()).to_string(),
        "disk full"
    );
    assert_eq!(pairings.validate(&ones(), T0), None);
    assert_eq!(pairings.list(T0).len(), 1);

    let disk_full = PairingError::Save("disk full".into());
    assert_eq!(
        pairings.revoke("pair-1000-AAAAAAAA", T0),
        Err(disk_full.clone())
    );
    assert_eq!(pairings.validate(&kept, T0), Some(AccessRole::Operator));
    assert_eq!(pairings.revoke_all(T0), Err(disk_full));
    assert_eq!(pairings.validate(&kept, T0), Some(AccessRole::Operator));
    assert_eq!(fake.store().file.tokens.len(), 1);
    assert!(fake.store().file.tokens[0].enabled);

    fake.store().save_error = None;
    assert_eq!(pairings.revoke_all(T0), Ok(1));
    assert_eq!(pairings.validate(&kept, T0), None);
}

/// A file the port cannot read grants no token, and no write replaces it: the claim answers the
/// read's error. Catches tokens granted from a file that could not be read, and a claim that
/// overwrites the unreadable file with only its own token.
#[test]
fn a_file_that_cannot_be_read_grants_nothing_and_is_never_written() {
    let fake = FakePort::new();
    fake.store().file.tokens = vec![row("one", "token-one", true)];
    fake.store().load_error = Some("unreadable file");
    let mut pairings = fake.open(T0);

    assert_eq!(pairings.validate("token-one", T0), None);
    assert_eq!(pairings.list(T0), Vec::new());
    fake.code(42);
    fake.token(0x00);
    let started = pairings.start(T0).expect("start");
    assert_eq!(
        pairings.claim(&started.code, None, T0),
        Err(PairingError::Unreadable("unreadable file".into()))
    );
    assert_eq!(
        PairingError::Unreadable("unreadable file".into()).to_string(),
        "unreadable file"
    );
    assert_eq!(pairings.revoke_all(T0), Ok(0));
    assert_eq!(fake.store().saves, 0);
    assert_eq!(
        fake.store().file.tokens,
        vec![row("one", "token-one", true)],
        "the file is kept for its owner to repair"
    );
}

/// Without random bytes a start is refused and keeps the pending code, and a claim stores no
/// token. Catches a code or a token drawn from bytes the source never gave.
#[test]
fn without_random_bytes_no_code_and_no_token_is_made() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);
    let no_bytes = PairingError::Random("no random bytes".into());
    assert_eq!(pairings.start(T0), Err(no_bytes.clone()));

    fake.code(42);
    pairings.start(T0).expect("start");
    assert_eq!(pairings.start(T0), Err(no_bytes.clone()));
    assert_eq!(pairings.claim("000042", None, T0), Err(no_bytes));
    assert_eq!(pairings.list(T0), Vec::new());
    assert_eq!(fake.store().saves, 0);
}

/// A runtime that answers one request at a time keeps its pairings in a `RefCell`; a borrow that
/// is taken already answers `pairing unavailable` and grants nothing. Catches a service that
/// differs from the pairings it wraps, and one that panics on a taken borrow.
#[test]
fn a_refcell_serves_the_pairings_one_request_at_a_time() {
    let fake = FakePort::new();
    let cell = RefCell::new(fake.open(T0));
    let service: &dyn PairingService = &cell;
    fake.code(42);
    fake.token(0x00);
    let started = service.start(T0).expect("start");
    let token = service.claim(&started.code, None, T0).expect("claim");
    assert_eq!(service.validate(&token, T0), Some(AccessRole::Operator));
    assert_eq!(service.list(T0).len(), 1);

    let held = cell.borrow();
    assert_eq!(service.start(T0), Err(PairingError::Unavailable));
    assert_eq!(PairingError::Unavailable.to_string(), "pairing unavailable");
    assert_eq!(service.validate(&token, T0), None);
    assert_eq!(service.list(T0), Vec::new());
    assert_eq!(service.revoke_all(T0), Err(PairingError::Unavailable));
    drop(held);

    assert_eq!(service.revoke("pair-1000-AAAAAAAA", T0), Ok(true));
    assert_eq!(service.revoke_all(T0), Ok(0));
}

/// Specification 11 §6.9.6: a claim that the storage cannot take first removes disabled tokens, the
/// oldest first, since they grant nothing. With room for 2 rows, token A (0x00) is claimed and
/// revoked, then B (0xFF) is claimed: 2 rows. Claiming C (0x11) removes A, the one disabled row, and
/// stores B and C. A fourth claim finds both rows enabled and is refused with `the device has no room
/// for another pairing`, writing nothing and keeping B and C. Catches a store that refuses while a
/// disabled row could make room, one that removes an enabled token, and one that removes a newer
/// disabled row before an older one.
#[test]
fn a_claim_the_storage_cannot_take_removes_disabled_tokens_first() {
    let fake = FakePort::new();
    fake.store().room = 2;
    let mut pairings = fake.open(T0);
    pair(&fake, &mut pairings, None, 0x00, T0);
    assert_eq!(pairings.revoke("pair-1000-AAAAAAAA", T0), Ok(true));
    let b = pair(&fake, &mut pairings, None, 0xFF, T0);
    let c = pair(&fake, &mut pairings, None, 0x11, T0);
    let ids: Vec<String> = pairings.list(T0).into_iter().map(|row| row.id).collect();
    assert_eq!(ids, ["pair-1000-________", "pair-1000-ERERERER"]);
    assert_eq!(pairings.validate(&b, T0), Some(AccessRole::Operator));
    assert_eq!(pairings.validate(&c, T0), Some(AccessRole::Operator));

    let saves = fake.store().saves;
    fake.code(42);
    fake.token(0x22);
    let started = pairings.start(T0).expect("start");
    assert_eq!(
        pairings.claim(&started.code, None, T0),
        Err(PairingError::NoRoom)
    );
    assert_eq!(
        PairingError::NoRoom.to_string(),
        "the device has no room for another pairing"
    );
    assert_eq!(fake.store().saves, saves, "the refused claim wrote nothing");
    assert_eq!(pairings.list(T0).len(), 2);
    assert_eq!(fake.store().file.tokens.len(), 2);
}

/// Specification 11 §6.9.6: each claim gets a unique id, even several in one clock second. Two
/// claims at 1,000 whose tokens begin with the same eight characters (both 32 bytes 0x00) get
/// `pair-1000-AAAAAAAA` and `pair-1000-AAAAAAAA-2`, and revoking the first leaves the second
/// enabled. Catches two tokens under one id, which a revoke of one would disable together.
#[test]
fn claims_in_one_second_get_ids_of_their_own() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);
    pair(&fake, &mut pairings, None, 0x00, T0);
    pair(&fake, &mut pairings, None, 0x00, T0);
    let ids: Vec<String> = pairings.list(T0).into_iter().map(|row| row.id).collect();
    assert_eq!(ids, ["pair-1000-AAAAAAAA", "pair-1000-AAAAAAAA-2"]);
    assert_eq!(pairings.revoke("pair-1000-AAAAAAAA", T0), Ok(true));
    let enabled: Vec<bool> = pairings
        .list(T0)
        .into_iter()
        .map(|row| row.enabled)
        .collect();
    assert_eq!(enabled, [false, true]);
}

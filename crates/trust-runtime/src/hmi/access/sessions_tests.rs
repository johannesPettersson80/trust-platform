//! Specification 39, §12.4 and §12.8: logins and session expiry.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::config::{HmiAccessConfig, HmiUser};
use super::password::{hash_password, PasswordHash};
use super::sessions::{LoginRefusal, SessionStore, LOCKOUT, MAX_FAILURES};
use crate::security::AccessRole;

struct Clock(Arc<Mutex<Instant>>);

impl Clock {
    fn advance(&self, by: Duration) {
        *self.0.lock().expect("clock") += by;
    }
}

fn store() -> (SessionStore, Clock) {
    let user = |name: &str, role| HmiUser {
        name: name.into(),
        role,
        password: PasswordHash::parse(&hash_password("secret", 10_000, b"0123456789abcdef"))
            .expect("hash"),
    };
    let access = HmiAccessConfig {
        idle: Duration::from_secs(60),
        session: Duration::from_secs(600),
        users: vec![
            user("anna", AccessRole::Operator),
            user("erik", AccessRole::Engineer),
        ],
        write: Vec::new(),
    };
    let time = Arc::new(Mutex::new(Instant::now()));
    let shared = time.clone();
    let store = SessionStore::with_clock(access, Arc::new(move || *shared.lock().expect("clock")));
    (store, Clock(time))
}

#[test]
fn the_right_password_starts_a_session_with_the_users_role() {
    let (store, _clock) = store();
    let (token, info) = store.login("anna", "secret").expect("logged in");
    assert!(token.len() >= 40, "a long random token");
    assert_eq!(
        (info.user.as_str(), info.role),
        ("anna", AccessRole::Operator)
    );
    assert_eq!(info.idle_expires_in, Duration::from_secs(60));
    assert_eq!(info.expires_in, Duration::from_secs(600));
    assert_ne!(info.id.as_str(), token, "the audit id is not the secret");
    assert_eq!(store.session(&token), Some(info));
    let (other, _) = store.login("ANNA", "secret").expect("names ignore case");
    assert_ne!(other, token, "every login gets its own token");
}

#[test]
fn a_wrong_password_and_an_unknown_user_get_the_same_answer() {
    let (store, _clock) = store();
    assert_eq!(
        store.login("anna", "Secret"),
        Err(LoginRefusal::WrongUserOrPassword)
    );
    assert_eq!(
        store.login("nobody", "secret"),
        Err(LoginRefusal::WrongUserOrPassword)
    );
    assert_eq!(store.session("not a token"), None);
}

#[test]
fn five_failures_refuse_the_user_for_a_minute() {
    let (store, clock) = store();
    for _ in 0..MAX_FAILURES {
        assert_eq!(
            store.login("erik", "wrong"),
            Err(LoginRefusal::WrongUserOrPassword)
        );
    }
    assert_eq!(
        store.login("erik", "secret"),
        Err(LoginRefusal::Locked { retry_in: LOCKOUT })
    );
    clock.advance(Duration::from_secs(20));
    assert_eq!(
        store.login("erik", "secret"),
        Err(LoginRefusal::Locked {
            retry_in: Duration::from_secs(40)
        })
    );
    assert!(
        store.login("anna", "secret").is_ok(),
        "other users are not refused"
    );
    clock.advance(Duration::from_secs(41));
    assert!(store.login("erik", "secret").is_ok());
}

#[test]
fn a_session_ends_after_its_idle_time_and_activity_keeps_it() {
    let (store, clock) = store();
    let (token, _) = store.login("anna", "secret").expect("logged in");
    clock.advance(Duration::from_secs(50));
    let touched = store.touch(&token).expect("still on");
    assert_eq!(
        touched.idle_expires_in,
        Duration::from_secs(60),
        "the idle time starts again"
    );
    assert_eq!(touched.expires_in, Duration::from_secs(550));
    clock.advance(Duration::from_secs(59));
    assert!(store.session(&token).is_some());
    clock.advance(Duration::from_secs(2));
    assert_eq!(store.session(&token), None, "61 s without activity");
    assert_eq!(
        store.touch(&token),
        None,
        "an ended session cannot be touched back"
    );
}

#[test]
fn a_session_ends_after_its_absolute_time_whatever_the_activity() {
    let (store, clock) = store();
    let (token, _) = store.login("anna", "secret").expect("logged in");
    for _ in 0..12 {
        clock.advance(Duration::from_secs(50));
        store.touch(&token);
    }
    assert_eq!(store.session(&token), None, "600 s have passed");
}

#[test]
fn logging_out_ends_the_session() {
    let (store, _clock) = store();
    let (token, _) = store.login("anna", "secret").expect("logged in");
    assert!(store.logout(&token));
    assert_eq!(store.session(&token), None);
    assert!(!store.logout(&token));
}

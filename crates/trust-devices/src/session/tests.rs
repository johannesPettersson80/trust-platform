//! The session's request numbers and its idle expiry (design §1.5).

use std::time::{Duration, Instant};

use super::{Sessions, IDLE_EXPIRY};

#[test]
fn session_answers_only_the_newest_seq_and_expires_when_idle() {
    let start = Instant::now();
    let mut sessions = Sessions::default();
    let session = sessions.session("dc-1", start);
    assert!(session.accept(5));
    assert!(
        !session.accept(4),
        "an older request is answered from the cache"
    );
    assert!(!session.accept(5), "a repeated request too");
    assert!(session.accept(6));

    sessions.session("dc-2", start);
    // Ten minutes of idleness keep a session open; one second more closes it.
    sessions.session("dc-1", start + IDLE_EXPIRY);
    assert!(sessions.is_open("dc-2"));
    sessions.session("dc-1", start + IDLE_EXPIRY + Duration::from_secs(1));
    assert!(!sessions.is_open("dc-2"));
    assert!(sessions.is_open("dc-1"), "a session in use stays open");

    assert!(sessions.close("dc-1"));
    assert!(!sessions.close("dc-1"));
    assert!(
        sessions.session("dc-1", start).accept(1),
        "a reopened session starts again"
    );
}

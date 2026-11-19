//! Service tokens (specification 11 §6.9.6; specification 48 RU-81) over the fake port of the
//! pairing tests. Every expected value is worked out by hand from the specification.
//!
//! Besides the tokens of 32 bytes 0x00 (43 `A`) and 0xFF (42 `_` and `8`), two claimed tokens
//! recur by their ids, `pair-1000-` and the token's first eight characters: three bytes 0x01 are
//! the six-bit groups 0, 16, 4 and 1, `AQEB`, so a token of 0x01 begins `AQEBAQEB`; three bytes 0x02
//! are 0, 32, 8 and 2, `AgIC`, so a token of 0x02 begins `AgICAgIC`.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use serde_json::json;

use super::tests::{ones, pair, zeros, FakePort, T0, T0_TOKEN_EXPIRY};
use super::*;

/// The service token of `name` at [`T0`] with the token of 32 bytes `byte`.
fn create(
    fake: &FakePort,
    pairings: &mut Pairings<FakePort>,
    name: &str,
    role: Option<AccessRole>,
    byte: u8,
) -> ServiceToken {
    fake.token(byte);
    pairings
        .service(name, role, T0)
        .unwrap_or_else(|error| panic!("the service token of {name}: {error}"))
}

/// The ids and enabled flags of the listed tokens at `now`.
fn listed(pairings: &mut Pairings<FakePort>, now: u64) -> Vec<(String, bool)> {
    pairings
        .list(now)
        .into_iter()
        .map(|summary| (summary.id, summary.enabled))
        .collect()
}

/// A service token is Viewer when no role is asked for, as a service that only reads needs no
/// more, keeps an asked role, and is Engineer for an Admin request. Catches a service token with a
/// claim's default role, Operator, and one that mints an Admin.
#[test]
fn a_service_token_is_a_viewer_unless_asked_otherwise_and_never_an_admin() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);

    assert_eq!(
        create(&fake, &mut pairings, "recipe", None, 0x00),
        ServiceToken {
            id: "service-recipe".into(),
            role: AccessRole::Viewer,
            token: zeros(),
        }
    );
    assert_eq!(
        create(&fake, &mut pairings, "mixer", Some(AccessRole::Admin), 0xFF),
        ServiceToken {
            id: "service-mixer".into(),
            role: AccessRole::Engineer,
            token: ones(),
        }
    );
    assert_eq!(
        create(
            &fake,
            &mut pairings,
            "dosing",
            Some(AccessRole::Operator),
            0x01
        )
        .role,
        AccessRole::Operator
    );
    assert_eq!(pairings.validate(&zeros(), T0), Some(AccessRole::Viewer));
    assert_eq!(pairings.validate(&ones(), T0), Some(AccessRole::Engineer));
}

/// A token claimed at 1,000 grants nothing at 2,593,001, past its 30 days; the service token
/// created beside it still grants Viewer then, and also after the pairings open again from the
/// stored file at the last second there is, where the row's `expires_at` 0 must not be read as a
/// row written before tokens expired (30 days after its creation). Catches a service token that
/// expires, now or when the pairings open.
#[test]
fn a_service_token_does_not_expire_and_a_claimed_one_does() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);
    let claimed = pair(&fake, &mut pairings, None, 0xFF, T0);
    create(&fake, &mut pairings, "recipe", None, 0x00);

    let later = T0_TOKEN_EXPIRY + 1;
    assert_eq!(pairings.validate(&claimed, later), None);
    assert_eq!(pairings.validate(&zeros(), later), Some(AccessRole::Viewer));

    let mut reopened = fake.open(u64::MAX);
    assert_eq!(
        reopened.validate(&zeros(), u64::MAX),
        Some(AccessRole::Viewer)
    );
    assert_eq!(
        reopened.list(u64::MAX),
        vec![PairingSummary {
            id: "service-recipe".into(),
            enabled: true,
            created_at: T0,
            expires_at: None,
            role: AccessRole::Viewer,
            service: Some("recipe".into()),
            tail: "…AAAA".into(),
        }]
    );
}

/// Between two claimed tokens, the service token of `recipe` is created again for `RECIPE` at
/// 1,005: it is the same service, so its row is replaced in its place, with the new spelling in its
/// id, the asked role and the new token; the earlier token grants nothing from then on. Catches a
/// rotation that leaves the earlier token valid, one that adds a row, and names compared with case.
#[test]
fn creating_a_service_token_again_replaces_it_in_its_place() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);
    pair(&fake, &mut pairings, None, 0x01, T0);
    create(&fake, &mut pairings, "recipe", None, 0x00);
    pair(&fake, &mut pairings, None, 0x02, T0);

    fake.token(0xFF);
    let rotated = pairings
        .service("RECIPE", Some(AccessRole::Engineer), T0 + 5)
        .expect("rotated");
    assert_eq!(
        rotated,
        ServiceToken {
            id: "service-RECIPE".into(),
            role: AccessRole::Engineer,
            token: ones(),
        }
    );
    assert_eq!(pairings.validate(&zeros(), T0 + 5), None);
    assert_eq!(
        pairings.validate(&ones(), T0 + 5),
        Some(AccessRole::Engineer)
    );
    assert_eq!(
        listed(&mut pairings, T0 + 5),
        vec![
            ("pair-1000-AQEBAQEB".into(), true),
            ("service-RECIPE".into(), true),
            ("pair-1000-AgICAgIC".into(), true),
        ]
    );
    let stored = &fake.store().file.tokens[1];
    assert_eq!(
        (stored.created_at, stored.service.as_deref()),
        (T0 + 5, Some("RECIPE"))
    );
}

/// A revoked service token grants nothing and stays listed, disabled, ten times 30 days later,
/// since it never expires, until it is created again; revoke-all counts it as one token. Catches a
/// revoked service token that still validates, one that vanishes, and a revoke-all that skips
/// service tokens.
#[test]
fn a_revoked_service_token_stays_listed_until_it_is_created_again() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);
    create(&fake, &mut pairings, "recipe", None, 0x00);

    assert_eq!(pairings.revoke("service-recipe", T0), Ok(true));
    assert_eq!(pairings.validate(&zeros(), T0), None);
    let far = T0_TOKEN_EXPIRY * 10;
    assert_eq!(
        listed(&mut pairings, far),
        vec![("service-recipe".into(), false)]
    );

    fake.token(0xFF);
    pairings
        .service("recipe", None, far)
        .expect("created again");
    assert_eq!(pairings.validate(&ones(), far), Some(AccessRole::Viewer));
    assert_eq!(pairings.revoke_all(far), Ok(1));
    assert_eq!(pairings.validate(&ones(), far), None);
}

/// With a limit of two enabled tokens: `a` and `b` take it, so `c` is refused and nothing is
/// stored; `A` replaces the enabled token of `a`, which enables none more, so it passes. After `b`
/// is revoked `c` takes its place in the count, and `b`, whose row is disabled, would enable a
/// third: refused. Catches a limit that skips service tokens, a replacement refused at the limit,
/// and a disabled row re-enabled past it.
#[test]
fn the_limit_counts_service_tokens_and_a_replacement_of_an_enabled_one_passes() {
    let fake = FakePort::new();
    fake.store().max_tokens = 2;
    let mut pairings = fake.open(T0);
    create(&fake, &mut pairings, "a", None, 0x01);
    create(&fake, &mut pairings, "b", None, 0x02);

    fake.token(0x03);
    assert_eq!(
        pairings.service("c", None, T0),
        Err(PairingError::LimitReached { max: 2 })
    );
    assert_eq!(fake.store().file.tokens.len(), 2);
    pairings
        .service("A", None, T0)
        .expect("a replacement enables no token more");

    assert_eq!(pairings.revoke("service-b", T0), Ok(true));
    create(&fake, &mut pairings, "c", None, 0x04);
    fake.token(0x05);
    assert_eq!(
        pairings.service("b", None, T0),
        Err(PairingError::LimitReached { max: 2 })
    );
    assert_eq!(
        listed(&mut pairings, T0),
        vec![
            ("service-A".into(), true),
            ("service-b".into(), false),
            ("service-c".into(), true),
        ]
    );
}

/// A service's name is an identifier: an ASCII letter or underscore, then ASCII letters, digits
/// and underscores. The empty name, a leading digit, a hyphen, a space, a letter outside ASCII and
/// a line end are refused before a token is drawn, and nothing is stored; `_recipe` and `Recipe_2`
/// are names. Catches a name that the program's heartbeat `<name>_heartbeat` could not carry.
#[test]
fn a_name_that_is_not_an_identifier_is_refused() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);
    for name in [
        "",
        "1recipe",
        "re-cipe",
        "recipe service",
        "réception",
        "recipe\n",
    ] {
        assert_eq!(
            pairings.service(name, None, T0),
            Err(PairingError::InvalidServiceName),
            "{name:?}"
        );
    }
    assert_eq!(fake.store().file.tokens, Vec::<PairingToken>::new());
    assert_eq!(
        PairingError::InvalidServiceName.to_string(),
        "invalid service name"
    );
    for name in ["_recipe", "Recipe_2"] {
        create(&fake, &mut pairings, name, None, 0x00);
    }
}

/// A rotation whose file cannot be written is refused with the port's text and keeps the earlier
/// token, and a file that could not be read refuses a service token with its read error. Catches a
/// rotation that takes effect before it is stored.
#[test]
fn a_service_token_that_cannot_be_written_changes_nothing() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);
    create(&fake, &mut pairings, "recipe", None, 0x00);

    fake.store().save_error = Some("disk full");
    fake.token(0xFF);
    assert_eq!(
        pairings.service("recipe", None, T0),
        Err(PairingError::Save("disk full".into()))
    );
    assert_eq!(pairings.validate(&zeros(), T0), Some(AccessRole::Viewer));
    assert_eq!(pairings.validate(&ones(), T0), None);

    let unreadable = FakePort::new();
    unreadable.store().load_error = Some("parse pairing file: broken");
    let mut refused = unreadable.open(T0);
    unreadable.token(0x00);
    assert_eq!(
        refused.service("recipe", None, T0),
        Err(PairingError::Unreadable(
            "parse pairing file: broken".into()
        ))
    );
}

/// The stored rows: a claimed token's row as before, without a `service` key, so a chip's record
/// holds as many claimed tokens as it did; a service token's row with its name and `expires_at` 0.
/// `pair.list`'s entry for the service token gives its name and `expires_at` `null`. Catches a
/// claimed row that grows, a service row without its name, and a list that shows an expiry.
#[test]
fn the_rows_and_the_list_carry_the_service_name() {
    let fake = FakePort::new();
    let mut pairings = fake.open(T0);
    pair(&fake, &mut pairings, Some(AccessRole::Viewer), 0x00, T0);
    create(&fake, &mut pairings, "recipe", None, 0xFF);

    assert_eq!(
        serde_json::to_value(&fake.store().file).expect("file"),
        json!({"tokens": [
            {
                "id": "pair-1000-AAAAAAAA",
                "token": zeros(),
                "created_at": 1_000,
                "enabled": true,
                "role": "viewer",
                "expires_at": 2_593_000,
            },
            {
                "id": "service-recipe",
                "token": ones(),
                "created_at": 1_000,
                "enabled": true,
                "role": "viewer",
                "expires_at": 0,
                "service": "recipe",
            },
        ]})
    );
    let summaries = pairings.list(T0);
    assert_eq!(
        serde_json::to_value(&summaries[1]).expect("summary"),
        json!({
            "id": "service-recipe",
            "enabled": true,
            "created_at": 1_000,
            "expires_at": null,
            "role": "viewer",
            "service": "recipe",
            "tail": "…___8",
        })
    );
}

/// A created service token's debug form shows its tail, never the token. Catches a token written
/// to a log.
#[test]
fn a_service_tokens_debug_form_hides_the_token() {
    let created = ServiceToken {
        id: "service-recipe".into(),
        role: AccessRole::Viewer,
        token: zeros(),
    };
    let shown = format!("{created:?}");
    assert!(shown.contains("…AAAA"), "{shown}");
    assert!(!shown.contains(&zeros()), "{shown}");
}

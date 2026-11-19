//! Specification 11 §6.9.6: a control token is URL-safe base64, whose alphabet holds `-`, so one
//! token in 64 begins with `-` and one in 4,096 with `--`; `trust-dev test --token` takes such a
//! token as its value (checklist G221).

use clap::Parser;

use crate::{Cli, Command};

#[test]
fn test_takes_a_device_token_that_begins_with_a_hyphen() {
    // Catches a `--token` that reads the token as options and refuses the command line, as
    // `trust-runtime ctl` did on #624's macOS run.
    for token in [
        "-4xRLpoHxGOxXyjZDnejFuxClTtgz4odVPI5rguGbcA",
        "--hRLpoHxGOxXyjZDnejFuxClTtgz4odVPI5rguGbcA",
    ] {
        let cli = Cli::try_parse_from([
            "trust-dev",
            "test",
            "--device",
            "/dev/ttyACM0",
            "--target",
            "esp32c6",
            "--token",
            token,
        ])
        .unwrap_or_else(|error| panic!("{token}: {error}"));
        let Command::Test { token: parsed, .. } = cli.command else {
            panic!("trust-dev test");
        };
        assert_eq!(parsed.as_deref(), Some(token));
    }
}

#[cfg(test)]
mod token_tests {
    //! Specification 11 §6.9.6: a pairing token, a service token among them, is 32 random bytes in
    //! URL-safe base64 without padding, whose alphabet (RFC 4648 §5) holds `-`. One token in 64
    //! begins with `-` and one in 4,096 with `--`, and every `--token` takes such a token as its
    //! value (checklist G221: #624's macOS run refused `ctl --token -4… pause` with "unexpected
    //! argument '-4' found").

    use super::*;
    use clap::{CommandFactory, Parser};

    /// Tokens as `ctl service-token` writes them, 43 characters: one beginning with `-`, one with
    /// `--`.
    const TOKENS: [&str; 2] = [
        "-4xRLpoHxGOxXyjZDnejFuxClTtgz4odVPI5rguGbcA",
        "--hRLpoHxGOxXyjZDnejFuxClTtgz4odVPI5rguGbcA",
    ];

    #[test]
    fn ctl_takes_a_token_that_begins_with_a_hyphen() {
        // The command line that failed, with `--token` before the subcommand and after it, as both
        // are written. Catches a `--token` that reads the token as options, which clap refused
        // with "unexpected argument '-4' found" or "a value is required for '--token <TOKEN>'".
        for token in TOKENS {
            for arguments in [
                ["trust-runtime", "ctl", "--token", token, "pause"],
                ["trust-runtime", "ctl", "pause", "--token", token],
            ] {
                let cli = Cli::try_parse_from(arguments)
                    .unwrap_or_else(|error| panic!("{arguments:?}: {error}"));
                match cli.command.expect("command") {
                    Command::Ctl { token: parsed, .. } => {
                        assert_eq!(parsed.as_deref(), Some(token), "{arguments:?}");
                    }
                    other => panic!("expected ctl, got {other:?}"),
                }
            }
        }
    }

    /// Every option `--token` of `command` and of its subcommands, with the subcommands' path.
    fn token_options(command: &clap::Command, path: &str, found: &mut Vec<(String, clap::Arg)>) {
        for arg in command.get_arguments() {
            if arg.get_long() == Some("token") {
                found.push((path.to_string(), arg.clone()));
            }
        }
        for subcommand in command.get_subcommands() {
            token_options(subcommand, &format!("{path} {}", subcommand.get_name()), found);
        }
    }

    #[test]
    fn every_token_option_takes_a_token_that_begins_with_a_hyphen() {
        // Each `--token` of the command tree on its own, so that its command's other arguments
        // need no values. Catches an option, of today or added later, that reads the token as
        // options. The twelve options of today are among those found, so that the walk cannot
        // pass by finding none.
        let mut found = Vec::new();
        token_options(&Cli::command(), "trust-runtime", &mut found);
        let paths: Vec<&str> = found.iter().map(|(path, _)| path.as_str()).collect();
        for known in [
            "trust-runtime ui",
            "trust-runtime ctl",
            "trust-runtime discover",
            "trust-runtime identify",
            "trust-runtime ads server status",
            "trust-runtime ads server symbols",
            "trust-runtime ads server doctor",
            "trust-runtime registry init",
            "trust-runtime registry publish",
            "trust-runtime registry download",
            "trust-runtime registry verify",
            "trust-runtime registry list",
        ] {
            assert!(paths.contains(&known), "{known} is not among {paths:?}");
        }
        for (path, arg) in found {
            for token in TOKENS {
                let matches = clap::Command::new("probe")
                    .arg(arg.clone())
                    .try_get_matches_from(["probe", "--token", token])
                    .unwrap_or_else(|error| panic!("{path} --token {token}: {error}"));
                assert_eq!(
                    matches.get_one::<String>("token").map(String::as_str),
                    Some(token),
                    "{path}"
                );
            }
        }
    }
}

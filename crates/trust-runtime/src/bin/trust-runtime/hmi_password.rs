//! `trust-runtime hmi-password` (specification 39, §12.8): the stored hash of an HMI user's
//! password, read from standard input, for `[[runtime.web.hmi.user]]` in `runtime.toml`.

use std::io::Read;

use anyhow::{bail, Result};
use trust_runtime::hmi::access::password::{hash_password, MIN_ITERATIONS};

/// Bytes of a new salt.
const SALT_BYTES: usize = 16;

pub fn run_hmi_password(iterations: u32) -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let mut salt = [0_u8; SALT_BYTES];
    getrandom::fill(&mut salt).map_err(|error| anyhow::anyhow!("no random source: {error}"))?;
    println!("{}", hash_line(&input, iterations, &salt)?);
    Ok(())
}

/// The hash of the password on the first line of `input`.
fn hash_line(input: &str, iterations: u32, salt: &[u8]) -> Result<String> {
    let password = input.lines().next().unwrap_or_default();
    if password.is_empty() {
        bail!("give the password on standard input, for example: printf '%s' \"$PASSWORD\" | trust-runtime hmi-password");
    }
    if iterations < MIN_ITERATIONS {
        bail!("--iterations must be at least {MIN_ITERATIONS}");
    }
    Ok(hash_password(password, iterations, salt))
}

#[cfg(test)]
mod tests {
    use super::hash_line;
    use trust_runtime::hmi::access::password::PasswordHash;

    #[test]
    fn the_first_line_is_the_password_and_its_hash_verifies() {
        let hash = hash_line("secret\n", 10_000, b"0123456789abcdef").expect("a hash");
        assert!(hash.starts_with("pbkdf2-sha256:10000:"), "{hash}");
        assert!(PasswordHash::parse(&hash).expect("valid").verify("secret"));
        assert!(hash_line("secret\r\n", 10_000, b"0123456789abcdef").is_ok());
    }

    #[test]
    fn an_empty_password_or_too_few_iterations_are_refused() {
        assert!(hash_line("\n", 10_000, b"0123456789abcdef").is_err());
        assert!(hash_line("secret", 1_000, b"0123456789abcdef").is_err());
    }
}

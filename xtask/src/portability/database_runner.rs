//! Bridge the disposable runner's generated settings to its existing native gate.
use anyhow::{bail, ensure, Context, Result};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

const PRODUCTS: [&str; 6] = [
    "POSTGRES",
    "TIMESCALE",
    "MYSQL",
    "MARIADB",
    "SQLSERVER",
    "INFLUX",
];

fn allowed_name(name: &str) -> bool {
    matches!(
        name,
        "OPENOT_DATABASE_RUNNER_STATE_DIR"
            | "OPENOT_DATABASE_RUNNER_PREFIX"
            | "TRUST_TEST_OPENOT_INFLUX_HOST"
            | "TRUST_TEST_OPENOT_INFLUX_TOKEN"
    ) || PRODUCTS.iter().any(|product| {
        ["CA_PEM", "CONTAINER"]
            .iter()
            .any(|field| name == format!("TRUST_TEST_OPENOT_{product}_{field}"))
            || (*product != "INFLUX" && name == format!("TRUST_TEST_OPENOT_{product}_URL"))
    })
}

fn decode(text: &str) -> Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let (name, value) = if let Some((name, value)) = line.split_once('=') {
            (name, value.to_owned())
        } else if let Some((name, delimiter)) = line.split_once("<<") {
            ensure!(!delimiter.is_empty(), "empty environment delimiter");
            let mut content = Vec::new();
            loop {
                let next = lines.next().context("unterminated environment value")?;
                if next == delimiter {
                    break;
                }
                content.push(next);
            }
            (name, content.join("\n"))
        } else {
            bail!("malformed environment entry");
        };
        ensure!(allowed_name(name), "unexpected environment name");
        ensure!(
            values.insert(name.to_owned(), value).is_none(),
            "duplicate environment name"
        );
    }
    Ok(values)
}

pub(super) fn run(root: &Path, input: &str) -> Result<()> {
    let mut values =
        decode(&fs::read_to_string(input).context("read generated runner environment")?)?;
    let state = values
        .get("OPENOT_DATABASE_RUNNER_STATE_DIR")
        .context("missing runner state directory")?;
    let ca = Path::new(state).join("tls/ca.pem");
    ensure!(ca.is_file(), "runner CA is missing");
    let ca = ca
        .to_str()
        .context("runner CA path is not UTF-8")?
        .to_owned();
    for product in PRODUCTS {
        values.insert(format!("TRUST_TEST_OPENOT_{product}_CA"), ca.clone());
    }
    let status = Command::new("bash")
        .arg("scripts/openot_real_database_gate.sh")
        .current_dir(root)
        .envs(values)
        .status()
        .context("launch existing database gate")?;
    if !status.success() {
        bail!("database gate failed: {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_environment_is_data_and_preserves_values() {
        let values = decode("TRUST_TEST_OPENOT_POSTGRES_URL=a=b;$(command)<<text\nTRUST_TEST_OPENOT_POSTGRES_CA_PEM<<END\nline one\nline two\nEND\n").unwrap();
        assert_eq!(
            values["TRUST_TEST_OPENOT_POSTGRES_URL"],
            "a=b;$(command)<<text"
        );
        assert_eq!(
            values["TRUST_TEST_OPENOT_POSTGRES_CA_PEM"],
            "line one\nline two"
        );
    }

    #[test]
    fn malformed_duplicate_or_unexpected_environment_is_rejected_without_values() {
        for input in [
            "TRUST_TEST_OPENOT_POSTGRES_URL=secret\nTRUST_TEST_OPENOT_POSTGRES_URL=secret\n",
            "TRUST_TEST_OPENOT_POSTGRES_CA_PEM<<END\nsecret\n",
            "TRUST_TEST_OPENOT_POSTGRES_CA_PEM<<\nsecret\n",
            "PATH=secret\n",
            "TRUST_TEST_OPENOT_UNKNOWN_SETTING=secret\n",
            "TRUST_TEST_OPENOT_BAD-NAME=secret\n",
            "secret\n",
        ] {
            let error = decode(input).unwrap_err().to_string();
            assert!(!error.contains("secret"));
        }
    }
}

#[cfg(all(test, unix))]
#[path = "database_runner_tls_tests.rs"]
mod tls_tests;

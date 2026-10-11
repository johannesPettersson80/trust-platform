//! Bounded, observable browser subprocesses for rendered test wiring.
#[cfg(not(unix))]
pub(super) fn run(_args: Vec<String>) -> anyhow::Result<()> {
    anyhow::bail!("browser-command requires a Unix host for owned process-group cleanup; no browser was launched")
}

#[cfg(unix)]
pub(super) use host::run;

#[cfg(unix)]
mod host {
    use anyhow::{bail, Context, Result};
    use std::{
        io::{Read, Write},
        process::{Child, Command, Stdio},
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };

    const MAX_STDOUT_BYTES: u64 = 64 * 1024 * 1024;

    pub(crate) fn run(args: Vec<String>) -> Result<()> {
        let [stage, executable, rest @ ..] = args.as_slice() else {
            bail!("browser-command requires probe|render, executable and browser arguments");
        };
        let timeout = match stage.as_str() {
            "probe" => Duration::from_secs(5),
            "render" => Duration::from_secs(45),
            _ => bail!("unknown browser subprocess stage {stage:?}"),
        };
        eprintln!("[browser-command] stage={stage} executable={executable:?} timeout={timeout:?} args={rest:?}");
        let output = execute(Command::new(executable).args(rest), timeout)?;
        std::io::stdout()
            .write_all(&output)
            .context("write browser output")
    }

    fn stop_group(child: &mut Child) -> Result<()> {
        // Always attempt direct-child termination and reap, even if group cleanup errors.
        let group = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{}", child.id())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let direct = match child.try_wait() {
            Ok(Some(_)) => Ok(()),
            Ok(None) | Err(_) => child.kill(),
        };
        let reaped = child.wait();
        let status = group.context("terminate browser process group")?;
        direct.context("terminate browser child")?;
        reaped.context("reap browser child")?;
        if !status.success() {
            let live = Command::new("/bin/kill")
                .args(["-0", "--", &format!("-{}", child.id())])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .context("verify browser process group cleanup")?;
            if live.success() {
                bail!("browser process group survived cleanup");
            }
        }
        Ok(())
    }

    struct OwnedBrowser {
        child: Child,
        cleaned: bool,
    }

    impl OwnedBrowser {
        fn cleanup(&mut self) -> Result<()> {
            let result = stop_group(&mut self.child);
            self.cleaned = true;
            result
        }
    }

    impl Drop for OwnedBrowser {
        fn drop(&mut self) {
            if !self.cleaned {
                let _ = stop_group(&mut self.child);
            }
        }
    }

    fn execute(command: &mut Command, timeout: Duration) -> Result<Vec<u8>> {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        use std::os::unix::process::CommandExt;
        command.process_group(0);
        let mut owned = OwnedBrowser {
            child: command.spawn().context("start browser subprocess")?,
            cleaned: false,
        };
        let stdout = owned
            .child
            .stdout
            .take()
            .context("browser stdout pipe missing")?;
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout.take(MAX_STDOUT_BYTES + 1).read_to_end(&mut bytes);
            let _ = sender.send(result.map(|_| bytes));
        });
        let start = Instant::now();
        let status = loop {
            match owned.child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if start.elapsed() < timeout => thread::sleep(Duration::from_millis(20)),
                Ok(None) => {
                    owned.cleanup()?;
                    bail!(
                        "browser subprocess timed out after {timeout:?}; process group terminated"
                    );
                }
                Err(error) => {
                    owned.cleanup()?;
                    return Err(error).context("poll browser subprocess");
                }
            }
        };
        owned.cleanup()?;
        if !status.success() {
            bail!("browser subprocess failed: {status}");
        }
        let bytes = receiver
            .recv_timeout(Duration::from_secs(1))
            .context("browser stdout did not close after process cleanup")?
            .context("read browser output")?;
        if bytes.len() as u64 > MAX_STDOUT_BYTES {
            bail!("browser output exceeds {MAX_STDOUT_BYTES} bytes");
        }
        Ok(bytes)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn browser_success_preserves_output_and_nonzero_is_failure() {
            assert_eq!(
                execute(
                    Command::new("/bin/sh").args(["-c", "printf 'rendered-dom'"]),
                    Duration::from_secs(1)
                )
                .unwrap(),
                b"rendered-dom"
            );
            assert!(execute(
                Command::new("/bin/sh").args(["-c", "exit 17"]),
                Duration::from_secs(1)
            )
            .is_err());
        }

        #[test]
        fn browser_timeout_terminates_descendants_and_returns_promptly() {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let marker = std::env::temp_dir().join(format!(
                "trust-browser-descendant-{}-{nonce}",
                std::process::id()
            ));
            let start = Instant::now();
            let result = execute(
                Command::new("/bin/sh")
                    .args([
                        "-c",
                        "(sleep 1; printf escaped > \"$1\") & wait",
                        "browser-test",
                    ])
                    .arg(&marker),
                Duration::from_millis(100),
            );
            assert!(result.unwrap_err().to_string().contains("timed out"));
            assert!(start.elapsed() < Duration::from_secs(1));
            thread::sleep(Duration::from_millis(1100));
            assert!(!marker.exists(), "descendant survived timeout cleanup");
        }

        #[test]
        fn successful_browser_leader_exit_cleans_its_live_descendant() {
            let start = Instant::now();
            let output = execute(
                Command::new("/bin/sh").args(["-c", "sleep 20 & printf dom"]),
                Duration::from_secs(1),
            )
            .unwrap();
            assert_eq!(output, b"dom");
            assert!(
                start.elapsed() < Duration::from_secs(2),
                "descendant kept stdout open"
            );
        }

        #[test]
        fn browser_output_limit_is_a_failure_and_reaps_the_writer() {
            let start = Instant::now();
            let result = execute(
                Command::new("/bin/sh").args([
                    "-c",
                    "dd if=/dev/zero bs=1048576 count=65 2>/dev/null; exit 0",
                ]),
                Duration::from_secs(5),
            );
            assert!(result.unwrap_err().to_string().contains("exceeds"));
            assert!(start.elapsed() < Duration::from_secs(5));
        }
    }
}

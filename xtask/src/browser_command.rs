//! Bounded, observable browser subprocesses for rendered test wiring.
pub(super) use host::run;

mod host {
    use anyhow::{bail, Context, Result};
    use std::{
        io::{Read, Write},
        process::{ChildStdout, Command, Stdio},
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };

    const MAX_STDOUT_BYTES: u64 = 64 * 1024 * 1024;

    #[cfg(unix)]
    type BrowserChild = std::process::Child;
    #[cfg(windows)]
    type BrowserChild = Box<dyn process_wrap::std::ChildWrapper>;

    fn spawn_browser(command: &mut Command) -> Result<BrowserChild> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
            command.spawn().context("start browser subprocess")
        }
        #[cfg(windows)]
        {
            use process_wrap::std::{CommandWrap, JobObject};
            let command = std::mem::replace(command, Command::new(""));
            let mut wrapped = CommandWrap::from(command);
            wrapped.wrap(JobObject);
            wrapped.spawn().context("start browser subprocess")
        }
    }

    fn child_stdout(child: &mut BrowserChild) -> Result<ChildStdout> {
        #[cfg(unix)]
        {
            child.stdout.take().context("browser stdout pipe missing")
        }
        #[cfg(windows)]
        {
            child
                .inner_mut()
                .stdout()
                .take()
                .context("browser stdout pipe missing")
        }
    }

    #[cfg(unix)]
    fn child_id(child: &BrowserChild) -> u32 {
        child.id()
    }

    fn child_try_wait(
        child: &mut BrowserChild,
    ) -> std::io::Result<Option<std::process::ExitStatus>> {
        #[cfg(unix)]
        {
            child.try_wait()
        }
        #[cfg(windows)]
        {
            child.inner_mut().try_wait()
        }
    }

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

    #[cfg(unix)]
    fn terminate_process_tree(child_id: u32) -> Result<std::process::ExitStatus> {
        Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{child_id}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("terminate browser process group")
    }

    #[cfg(unix)]
    fn stop_group(child: &mut BrowserChild) -> Result<()> {
        // Always attempt group termination and direct-child reap, even if cleanup errors.
        let group = terminate_process_tree(child_id(child));
        let direct = match child_try_wait(child) {
            Ok(Some(_)) => Ok(()),
            Ok(None) | Err(_) => child.kill(),
        };
        let reaped = child.wait();
        let status = group?;
        direct.context("terminate browser child")?;
        reaped.context("reap browser child")?;
        if !status.success() {
            let live = Command::new("/bin/kill")
                .args(["-0", "--", &format!("-{}", child_id(child))])
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

    #[cfg(windows)]
    fn stop_group(child: &mut BrowserChild) -> Result<()> {
        let termination = child.start_kill();
        let direct = if termination.is_err() {
            child.inner_mut().kill()
        } else {
            Ok(())
        };
        // JobObjectChild::wait drains one completion-port message, not a
        // documented ACTIVE_PROCESS_ZERO signal. Keep the job wrapper alive,
        // but reap only the owned leader through its safe inner child API.
        let reaped = child.inner_mut().wait();
        termination.context("terminate browser process job")?;
        direct.context("terminate browser child")?;
        reaped.context("reap browser child")?;
        Ok(())
    }

    struct OwnedBrowser {
        child: BrowserChild,
        cleaned: bool,
    }

    impl OwnedBrowser {
        fn cleanup(&mut self) -> Result<()> {
            let result = stop_group(&mut self.child);
            self.cleaned = result.is_ok();
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
        let mut owned = OwnedBrowser {
            child: spawn_browser(command)?,
            cleaned: false,
        };
        let stdout = child_stdout(&mut owned.child)?;
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout.take(MAX_STDOUT_BYTES + 1).read_to_end(&mut bytes);
            let _ = sender.send(result.map(|_| bytes));
        });
        let start = Instant::now();
        let status = loop {
            match child_try_wait(&mut owned.child) {
                Ok(Some(status)) => break status,
                Ok(None) if start.elapsed() < timeout => thread::sleep(Duration::from_millis(20)),
                Ok(None) => {
                    owned.cleanup()?;
                    bail!(
                        "browser subprocess timed out after {timeout:?}; process tree terminated"
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

        #[cfg(unix)]
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

        #[cfg(unix)]
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

        #[cfg(unix)]
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

        #[cfg(unix)]
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

        #[cfg(windows)]
        #[test]
        fn windows_browser_success_preserves_output_and_nonzero_is_failure() {
            // `set /p` with no input sets ERRORLEVEL 1, so the command ends with `exit 0`;
            // no space before `&`, which `set /p` would print.
            let output = execute(
                Command::new("cmd.exe").args(["/C", "<nul set /p=rendered-dom& exit 0"]),
                Duration::from_secs(1),
            )
            .unwrap();
            assert_eq!(output, b"rendered-dom");
            assert!(execute(
                Command::new("cmd.exe").args(["/C", "exit /b 17"]),
                Duration::from_secs(1)
            )
            .is_err());
        }

        #[cfg(windows)]
        #[test]
        fn windows_successful_leader_exit_cleans_its_live_descendant() {
            use std::os::windows::process::CommandExt;

            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let marker = std::env::temp_dir().join(format!(
                "trust-browser-descendant-{}-{nonce}",
                std::process::id()
            ));
            // cmd.exe does not parse the quoting Rust applies to ordinary arguments, so the
            // command line is passed verbatim; `exit 0` overrides `set /p`'s ERRORLEVEL 1.
            let command = format!(
                "start \"\" /B cmd.exe /C \"ping -n 4 127.0.0.1 > nul & echo escaped > \"{}\"\" & <nul set /p=dom& exit 0",
                marker.display()
            );
            let start = Instant::now();
            let output = execute(
                Command::new("cmd.exe").arg("/C").raw_arg(&command),
                Duration::from_secs(2),
            )
            .unwrap();
            assert_eq!(output, b"dom");
            assert!(start.elapsed() < Duration::from_secs(2));
            thread::sleep(Duration::from_secs(4));
            assert!(
                !marker.exists(),
                "browser descendant survived successful leader cleanup"
            );
        }

        #[cfg(windows)]
        #[test]
        fn windows_browser_timeout_terminates_descendants() {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let marker = std::env::temp_dir().join(format!(
                "trust-browser-descendant-{}-{nonce}",
                std::process::id()
            ));
            let command = format!(
                "start \"\" /B cmd.exe /C \"ping -n 3 127.0.0.1 > nul & echo escaped > \"{}\"\" & ping -n 20 127.0.0.1 > nul",
                marker.display()
            );
            let result = execute(
                Command::new("cmd.exe").args(["/C", &command]),
                Duration::from_millis(100),
            );
            assert!(result.unwrap_err().to_string().contains("timed out"));
            thread::sleep(Duration::from_secs(3));
            assert!(
                !marker.exists(),
                "browser descendant survived timeout cleanup"
            );
        }
    }
}

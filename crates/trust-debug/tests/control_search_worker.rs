//! The DAP host must expose the same isolated worker entry as trust-runtime, held to the worker's
//! 1 GiB memory limit on every platform (protocol annex E8.1): the address-space limit on Linux,
//! the program allocator's budget elsewhere. Catches a platform whose worker refuses to run or
//! reports a limit other than 1 GiB.
use std::io::Write;
use std::process::{Command, Stdio};
#[test]
fn debug_host_dispatches_isolated_control_search_worker() {
    let executable = std::env::var_os("TRUST_SEARCH_DEBUG_TEST_BIN")
        .or_else(|| std::env::var_os("CARGO_BIN_EXE_trust-debug"))
        .expect("reviewed trust-debug executable");
    let mut child = Command::new(executable)
        .arg("control-search-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch debug host worker");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(include_bytes!("fixtures/control_search_worker.json"))
        .expect("write bounded worker fixture");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("debug host worker did not finish within its integration budget");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "debug host must dispatch worker instead of DAP: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let result = text
        .lines()
        .find_map(|line| line.strip_prefix("TRUST_SEARCH_RESULT "))
        .expect("bounded worker JSON result");
    assert!(result.contains("\"status\":\"qualified\""), "{result}");
    assert!(result.contains("\"evidence\":\"ST-executed\""), "{result}");
    assert!(
        result.contains("\"worker_memory_limit_bytes\":1073741824"),
        "{result}"
    );
}

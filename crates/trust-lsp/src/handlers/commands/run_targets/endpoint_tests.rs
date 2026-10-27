//! What an endpoint's text says: the credential channel (specification 11, the credential
//! channel) and the short names of a runtime (specification 25 §5.1).

use super::{credential_channel, remote_label, sentence_label};

/// A loopback TCP endpoint and a Unix socket are this computer; any other TCP endpoint is a plain
/// network; a blank, unreadable or serial endpoint, and a TCP endpoint without a host or a usable
/// port, carry no credential. Catches `[::1]` read as a remote host (its brackets), `127.0.0.2`, a
/// loopback address, read as remote, and a token offered to an endpoint that cannot be reached.
#[test]
fn the_credential_channel_follows_the_endpoint() {
    for (endpoint, channel) in [
        ("tcp://127.0.0.1:9901", "trusted_same_host"),
        ("tcp://127.0.0.2:9901", "trusted_same_host"),
        ("tcp://localhost:9901", "trusted_same_host"),
        ("tcp://LOCALHOST:9901", "trusted_same_host"),
        ("tcp://[::1]:9901", "trusted_same_host"),
        ("tcp://192.168.10.20:9901", "untrusted_remote_plain_tcp"),
        ("tcp://plc.example:9901", "untrusted_remote_plain_tcp"),
        ("", "unavailable"),
        ("   ", "unavailable"),
        ("http://127.0.0.1:9901", "unavailable"),
        ("tcp://localhost", "unavailable"),
        ("tcp://localhost:0", "unavailable"),
        ("tcp://:9222", "unavailable"),
        ("tcp://localhost:65536", "unavailable"),
    ] {
        assert_eq!(credential_channel(endpoint), channel, "{endpoint}");
    }
    #[cfg(unix)]
    {
        assert_eq!(
            credential_channel("unix:///tmp/trust.sock"),
            "trusted_same_host"
        );
        assert_eq!(credential_channel("serial:///dev/ttyACM0"), "unavailable");
    }
}

/// A runtime's name from its endpoint keeps the port, so that two runtimes on one host stay apart
/// (specification 25 §5.1); a Unix socket keeps its path; a blank endpoint is `runtime`. Catches a
/// name without its port.
#[test]
fn a_remote_label_keeps_the_port() {
    for (endpoint, label) in [
        ("tcp://127.0.0.1:9901", "127.0.0.1:9901"),
        (" tcp://10.0.0.2:9902/x ", "10.0.0.2:9902"),
        ("TCP://plc:1", "plc:1"),
        ("unix:///tmp/trust.sock", "/tmp/trust.sock"),
        ("plc:9901", "plc:9901"),
        ("", "runtime"),
    ] {
        assert_eq!(remote_label(endpoint), label, "{endpoint}");
    }
}

/// The endpoint as a sentence names it. Catches a raw `unix://` path in a user-facing message.
#[test]
fn the_sentence_label_of_an_endpoint() {
    assert_eq!(sentence_label("tcp://10.0.0.2:9901"), "10.0.0.2:9901");
    assert_eq!(
        sentence_label("unix:///tmp/a.sock"),
        "the local control socket"
    );
    assert_eq!(sentence_label(" "), "the configured endpoint");
    assert_eq!(sentence_label("tcp://"), "the configured endpoint");
    assert_eq!(sentence_label("serial://COM5"), "serial://COM5");
}

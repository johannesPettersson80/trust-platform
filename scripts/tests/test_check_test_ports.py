"""A test never reserves a port and releases it (specification 22): the guard finds a bind of port 0 in test code whose
listener is dropped while its port goes on, and nothing else. Each expected finding follows from the source text the
test writes."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import check_test_ports as check  # noqa: E402

TEST_FILE = "crates/x/tests/ports.rs"
TESTS = {TEST_FILE}

CHAINED = """fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("address")
        .port()
}
"""

RETURNED = """fn reserve_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.local_addr().expect("address").port()
}
"""


class TestReleasedPorts(unittest.TestCase):
    """What the guard refuses: a listener dropped while its port goes on."""

    def test_a_listener_read_in_one_expression_is_refused(self) -> None:
        """`bind` of port 0 read with `.local_addr()` in the same expression drops its listener at the end of the
        statement; the finding is the `bind`'s line, 2. Catches a guard that misses the chained form, the most
        common reservation."""
        self.assertEqual(check.violations(TEST_FILE, CHAINED, TESTS), [(2, 'TcpListener::bind("127.0.0.1:0")')])

    def test_a_function_that_returns_a_port_of_its_own_listener_is_refused(self) -> None:
        """A function that binds port 0 into a local and returns a `u16` drops the listener when it returns; the
        finding is line 2. Catches a guard that takes a named listener for a kept one."""
        self.assertEqual(
            check.violations(TEST_FILE, RETURNED, TESTS),
            [(2, 'let listener = TcpListener::bind("127.0.0.1:0").expect("bind");')],
        )

    def test_every_spelling_of_port_zero_and_every_unwrapping_step_is_read(self) -> None:
        """The address as a tuple, as `SocketAddr::from` and as text, with `?`, `.await` and `.unwrap()` between the
        bind and the read: three findings, lines 2, 3 and 4. Catches a guard that knows only the text form or only
        `.expect`."""
        source = (
            "async fn ports() -> std::io::Result<()> {\n"
            '    let a = TcpListener::bind(("127.0.0.1", 0)).await?.local_addr()?.port();\n'
            "    let b = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap().local_addr();\n"
            '    let c = TcpListener::bind("[::1]:0")?.local_addr()?;\n'
            "    Ok(())\n"
            "}\n"
        )
        self.assertEqual([line for line, _ in check.violations(TEST_FILE, source, TESTS)], [2, 3, 4])


class TestKeptListeners(unittest.TestCase):
    """What the guard leaves alone: a listener that is kept, a concrete port, product code, the marker."""

    def test_a_kept_listener_is_not_refused(self) -> None:
        """A listener named and served, one returned with its port, and a socket returned: none is dropped while its
        port goes on. Catches a guard that refuses every bind of port 0, which most servers in tests rightly do."""
        source = (
            "fn serve() {\n"
            '    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");\n'
            "    let port = listener.local_addr().expect(\"address\").port();\n"
            "    thread::spawn(move || listener.accept());\n"
            "    connect(port);\n"
            "}\n"
            "fn held() -> (TcpListener, u16) {\n"
            '    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");\n'
            "    let port = listener.local_addr().expect(\"address\").port();\n"
            "    (listener, port)\n"
            "}\n"
        )
        self.assertEqual(check.violations(TEST_FILE, source, TESTS), [])

    def test_a_concrete_port_and_text_that_only_looks_like_a_bind_are_not_refused(self) -> None:
        """A bind of a concrete port read at once, and a bind of port 0 in a comment and in a string: no finding.
        Catches a guard that reads comments and strings as code, or every port as port 0."""
        source = (
            "fn other() -> u16 {\n"
            '    let fixed = TcpListener::bind("127.0.0.1:8080").expect("bind").local_addr().expect("a").port();\n'
            '    // TcpListener::bind("127.0.0.1:0").unwrap().local_addr()\n'
            '    let text = "TcpListener::bind(\\"127.0.0.1:0\\").unwrap().local_addr()";\n'
            "    fixed\n"
            "}\n"
        )
        self.assertEqual(check.violations(TEST_FILE, source, TESTS), [])

    def test_product_code_is_not_refused_and_its_test_module_is(self) -> None:
        """The same reservation in a product file's function and in its `#[cfg(test)]` module: only the module's,
        line 9, is a finding. Catches a guard that refuses product code (a product may probe a port) and one that
        misses an inline test module."""
        source = (
            "fn probe() -> u16 {\n"
            '    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()\n'
            "}\n"
            "\n"
            "#[cfg(test)]\n"
            "mod tests {\n"
            "    use super::*;\n"
            "    fn free_port() -> u16 {\n"
            '        TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()\n'
            "    }\n"
            "}\n"
        )
        self.assertEqual([line for line, _ in check.violations("crates/x/src/probe.rs", source, set())], [9])

    def test_the_marker_lets_a_helper_whose_callers_try_again_pass(self) -> None:
        """The marker on the line before the function, or on the `bind`'s own line, lets a helper pass; on another
        line it does not. Catches a guard that ignores the marker, which would leave no way to keep a reservation
        that a program's concrete port needs, and one that takes it from anywhere in the file."""
        before_function = "// port-retry-ok: the start tries again on a taken port.\n" + CHAINED
        on_the_bind = CHAINED.replace(
            'TcpListener::bind("127.0.0.1:0")', 'TcpListener::bind("127.0.0.1:0") // port-retry-ok: retried'
        )
        too_far = "// port-retry-ok: the start tries again on a taken port.\n\n" + CHAINED
        self.assertEqual(check.violations(TEST_FILE, before_function, TESTS), [])
        self.assertEqual(check.violations(TEST_FILE, on_the_bind, TESTS), [])
        self.assertEqual([line for line, _ in check.violations(TEST_FILE, too_far, TESTS)], [4])

    def test_the_test_support_crate_is_not_refused(self) -> None:
        """`trust-test-support` holds the helpers that replace a reservation, so its own binds are not findings.
        Catches a guard that refuses the crate the rule sends tests to."""
        helper = "crates/trust-test-support/src/lib.rs"
        self.assertEqual(check.violations(helper, CHAINED, {helper}), [])


class TestMain(unittest.TestCase):
    """The command over a tree: its exit status and the findings it prints."""

    def test_the_command_fails_on_a_reservation_and_passes_without_one(self) -> None:
        """A tree with one reservation in an integration test fails with its place; without it, it passes. Catches a
        command that reads no files or always passes."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            test = root / "crates/x/tests/ports.rs"
            test.parent.mkdir(parents=True)
            test.write_text(CHAINED, encoding="utf-8")
            self.assertEqual(check.main(["check_test_ports.py", str(root)]), 1)
            test.write_text("fn none() {}\n", encoding="utf-8")
            self.assertEqual(check.main(["check_test_ports.py", str(root)]), 0)


if __name__ == "__main__":
    unittest.main()

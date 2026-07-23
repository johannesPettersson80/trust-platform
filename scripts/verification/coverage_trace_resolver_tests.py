"""Focused tests for mechanical scanner-to-nextest identity resolution."""

from __future__ import annotations

import unittest

from scripts.verification.coverage_trace_resolver import (
    mechanical_rust_target,
    resolve_rust_tests,
)


def _fact(
    discovery_id: str,
    *,
    name: str = "same_name",
    path: str = "crates/example/src/lib.rs",
    source_kind: str = "rust_unit_test",
) -> dict[str, object]:
    return {
        "discovery_id": discovery_id,
        "discovery_source_kind": source_kind,
        "package": "example",
        "name": name,
        "path": path,
        "ignore_state": "not_ignored",
    }


def _list(*test_names: str) -> dict[str, object]:
    return {
        "test-count": len(test_names),
        "rust-build-meta": {},
        "rust-suites": {
            "example": {
                "package-name": "example",
                "binary-id": "example",
                "binary-name": "example",
                "binary-path": "/target/example",
                "kind": "lib",
                "testcases": {
                    name: {"ignored": False} for name in test_names
                },
            }
        },
    }


class CoverageTraceResolverTests(unittest.TestCase):
    def test_mechanical_target_selection_never_expands_to_all_package_targets(self) -> None:
        cases = [
            (
                _fact(
                    "DISC_LIB",
                    path="crates/example/src/nested/tests.rs",
                    source_kind="rust_unit_test",
                ),
                ("lib", None, ("--lib",)),
            ),
            (
                _fact(
                    "DISC_INTEGRATION",
                    path="crates/example/tests/api_smoke.rs",
                    source_kind="rust_integration_test",
                ),
                ("test", "api_smoke", ("--test", "api_smoke")),
            ),
            (
                _fact(
                    "DISC_BIN",
                    path="crates/example/src/bin/worker/tests.rs",
                    source_kind="rust_unit_test",
                ),
                ("bin", "worker", ("--bin", "worker")),
            ),
        ]

        for fact, expected in cases:
            with self.subTest(path=fact["path"]):
                target = mechanical_rust_target(fact)
                self.assertEqual(expected[0], target.get("kind"))
                self.assertEqual(expected[1], target.get("name"))
                self.assertEqual(expected[2], tuple(target.get("cargo_args", ())))

    def test_binary_only_package_unit_test_uses_its_single_binary_target(self) -> None:
        fact = _fact(
            "DISC_BIN_ONLY",
            path="crates/example/src/commands.rs",
            source_kind="rust_unit_test",
        )
        fact["package_has_library"] = False
        fact["package_binary_names"] = ["example"]

        target = mechanical_rust_target(fact)

        self.assertEqual("bin", target["kind"])
        self.assertEqual("example", target["name"])
        self.assertEqual(("--bin", "example"), target["cargo_args"])

    def test_binary_only_package_with_multiple_targets_fails_closed(self) -> None:
        fact = _fact(
            "DISC_BIN_AMBIGUOUS",
            path="crates/example/src/shared.rs",
            source_kind="rust_unit_test",
        )
        fact["package_has_library"] = False
        fact["package_binary_names"] = ["one", "two"]

        with self.assertRaisesRegex(ValueError, "no unique binary target"):
            mechanical_rust_target(fact)

    def test_unique_package_and_terminal_name_resolves_exact_identity(self) -> None:
        resolved, unsupported = resolve_rust_tests(
            facts=[_fact("DISC_ONE")],
            nextest_document=_list("module::same_name"),
        )

        self.assertEqual([], unsupported)
        self.assertEqual("module::same_name", resolved[0]["nextest_test_name"])
        self.assertEqual("example", resolved[0]["nextest_binary_id"])
        self.assertEqual("verified_synthesized", resolved[0]["command_authority"])

    def test_ambiguous_terminal_name_fails_closed(self) -> None:
        resolved, unsupported = resolve_rust_tests(
            facts=[_fact("DISC_ONE")],
            nextest_document=_list("one::same_name", "two::same_name"),
        )

        self.assertEqual([], resolved)
        self.assertEqual("ambiguous_nextest_identity", unsupported[0]["reason"])
        self.assertEqual(
            ["one::same_name", "two::same_name"],
            unsupported[0]["candidate_test_names"],
        )

    def test_missing_and_non_rust_facts_are_explicitly_unsupported(self) -> None:
        resolved, unsupported = resolve_rust_tests(
            facts=[
                _fact("DISC_MISSING", name="missing"),
                _fact(
                    "DISC_VSCODE",
                    source_kind="vscode_test",
                    path="editors/vscode/src/test/suite/example.ts",
                ),
            ],
            nextest_document=_list("module::other"),
        )

        self.assertEqual([], resolved)
        self.assertEqual(
            ["no_nextest_identity", "source_kind_not_rust"],
            sorted(row["reason"] for row in unsupported),
        )


if __name__ == "__main__":
    unittest.main()

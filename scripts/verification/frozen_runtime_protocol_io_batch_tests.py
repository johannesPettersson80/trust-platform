"""Focused tests for the runtime protocol/I/O batch executor."""

from __future__ import annotations

import unittest

from scripts.verification.frozen_runtime_protocol_io_batch import (
    build_batch_command,
    normalize_results,
    project_raw_results,
    resolve_nextest_listing,
)


class FrozenRuntimeProtocolIoBatchTests(unittest.TestCase):
    def test_listing_resolves_bare_names_to_exact_binary_and_full_name(self) -> None:
        resolved = resolve_nextest_listing(
            listing={
                "rust-suites": {
                    "trust-runtime": {
                        "testcases": {
                            "io::tests::one": {"ignored": False},
                        }
                    },
                    "trust-runtime::contract": {
                        "testcases": {"two": {"ignored": False}},
                    },
                }
            },
            selected_names={"one", "two"},
        )
        self.assertEqual("trust-runtime", resolved["one"]["binary_id"])
        self.assertEqual("io::tests::one", resolved["one"]["full_name"])
        command = build_batch_command(resolved)
        self.assertIn("binary_id(=trust-runtime)", command[-1])
        self.assertIn("test(=two)", command[-1])

    def test_duplicate_bare_name_across_binaries_fails_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            resolve_nextest_listing(
                listing={
                    "rust-suites": {
                        "a": {"testcases": {"module::same": {}}},
                        "b": {"testcases": {"same": {}}},
                    }
                },
                selected_names={"same"},
            )

    def test_projection_and_normalization_require_exact_passing_set(self) -> None:
        raw = "\n".join(
            [
                '{"type":"test","event":"started","name":"a$module::one"}',
                '{"type":"test","event":"ok","name":"a$module::one","exec_time":0.001}',
                '{"type":"test","event":"started","name":"b$two"}',
                '{"type":"test","event":"ok","name":"b$two","exec_time":0.002}',
            ]
        )
        resolved = {
            "one": {
                "binary_id": "a",
                "full_name": "module::one",
                "encoded_name": "a$module::one",
            },
            "two": {
                "binary_id": "b",
                "full_name": "two",
                "encoded_name": "b$two",
            },
        }
        projected = project_raw_results(
            raw=raw,
            resolved=resolved,
            path_by_name={"one": "src/lib.rs", "two": "tests/two.rs"},
        )
        self.assertEqual(
            [{"title": "one", "duration": 1, "state": "passed"}],
            normalize_results(
                projected["src/lib.rs"],
                expected={"one": "a$module::one"},
            ),
        )


if __name__ == "__main__":
    unittest.main()

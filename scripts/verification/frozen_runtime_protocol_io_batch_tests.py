"""Focused tests for the runtime protocol/I/O batch executor."""

from __future__ import annotations

import unittest

from scripts.verification.frozen_runtime_protocol_io_batch import (
    build_batch_command,
    combined_batch_command,
    normalize_results,
    project_raw_results,
    resolve_nextest_listing,
    selected_feature_shards,
    selected_target_args,
)


class FrozenRuntimeProtocolIoBatchTests(unittest.TestCase):
    def test_listing_resolves_bare_names_to_exact_binary_and_full_name(self) -> None:
        resolved = resolve_nextest_listing(
            listing={
                "rust-suites": {
                    "trust-runtime::trust_runtime": {
                        "binary-id": "trust-runtime",
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
        self.assertEqual(
            "trust-runtime::trust_runtime$io::tests::one",
            resolved["one"]["encoded_name"],
        )
        command = build_batch_command(
            resolved,
            target_args=["--lib", "--test", "contract"],
        )
        self.assertIn("binary_id(=trust-runtime)", command[-1])
        self.assertIn("test(=two)", command[-1])
        self.assertIn("--lib", command)
        self.assertIn("contract", command)

    def test_target_selection_excludes_unrelated_runtime_binaries(self) -> None:
        self.assertEqual(
            ["--lib", "--test", "connectors_status", "--test", "io_cycle"],
            selected_target_args(
                [
                    {"path": "crates/trust-runtime/src/io/interface.rs"},
                    {"path": "crates/trust-runtime/tests/io_cycle.rs"},
                    {"path": "crates/trust-runtime/tests/connectors_status.rs"},
                ]
            ),
        )

    def test_source_cfg_splits_no_ads_wire_tests_from_default(self) -> None:
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "tests.rs"
            source.write_text(
                """
#[test]
fn default_case() {}

#[cfg(not(feature = "ads-wire"))]
#[test]
fn disabled_case() {}
"""
            )
            shards = selected_feature_shards(
                root=root,
                selection=[
                    {"path": "tests.rs", "name": "default_case"},
                    {"path": "tests.rs", "name": "disabled_case"},
                ],
            )
            self.assertEqual(
                [
                    (
                        "no-ads-wire",
                        [{"path": "tests.rs", "name": "disabled_case"}],
                        True,
                    ),
                    (
                        "default",
                        [{"path": "tests.rs", "name": "default_case"}],
                        False,
                    ),
                ],
                shards,
            )
            command = combined_batch_command(
                [
                    ["cargo", "nextest", "run", "--no-default-features"],
                    ["cargo", "nextest", "run"],
                ]
            )
            self.assertEqual(["sh", "-c"], command[:2])
            self.assertIn("--no-default-features", command[2])
            self.assertIn(" && ", command[2])

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

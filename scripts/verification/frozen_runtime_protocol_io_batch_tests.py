"""Focused tests for the runtime protocol/I/O batch executor."""

from __future__ import annotations

import unittest

from scripts.verification.frozen_runtime_protocol_io_batch import (
    _mixed_target_profile,
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
                        "binary-name": "trust_runtime",
                        "package-name": "trust-runtime",
                        "testcases": {
                            "io::tests::one": {"ignored": False},
                        }
                    },
                    "trust-runtime::contract": {
                        "binary-name": "contract",
                        "package-name": "trust-runtime",
                        "testcases": {"two": {"ignored": False}},
                    },
                }
            },
            selected_names={"one", "two"},
        )
        self.assertEqual("trust-runtime", resolved["one"]["binary_id"])
        self.assertEqual("trust-runtime", resolved["one"]["package"])
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

    def test_listing_uses_reviewed_package_to_disambiguate_reexported_test(self) -> None:
        listing = {
            "rust-suites": {
                package: {
                    "binary-id": package,
                    "binary-name": package.replace("-", "_"),
                    "package-name": package,
                    "testcases": {
                        "value::tests::shared_name": {"ignored": False},
                    },
                }
                for package in ("trust-runtime", "trust-runtime-core")
            }
        }

        resolved = resolve_nextest_listing(
            listing=listing,
            selected_names={"shared_name"},
            expected_packages={"shared_name": "trust-runtime-core"},
        )

        self.assertEqual("trust-runtime-core", resolved["shared_name"]["package"])

    def test_mixed_batch_command_uses_explicit_package_selection_once(self) -> None:
        command = build_batch_command(
            {
                "debug_case": {
                    "binary_id": "trust-debug",
                    "full_name": "adapter::tests::debug_case",
                    "encoded_name": (
                        "trust-debug::trust_debug$"
                        "adapter::tests::debug_case"
                    ),
                    "package": "trust-debug",
                }
            },
            target_args=[
                "-p",
                "trust-debug",
                "-p",
                "trust-runtime-core",
                "--lib",
            ],
            include_package=False,
        )

        self.assertEqual(2, command.count("-p"))
        self.assertEqual(1, command.count("trust-debug"))
        self.assertIn("binary_id(=trust-debug)", command[-1])

    def test_mixed_profile_resolves_runtime_integration_target(self) -> None:
        profile = _mixed_target_profile(
            "crates/trust-runtime/tests/stdlib_select.rs"
        )

        self.assertEqual("trust-runtime", profile.package)
        self.assertEqual(("--test", "stdlib_select"), profile.cargo_target_args)

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

    def test_target_selection_includes_runtime_binary_unit_tests(self) -> None:
        self.assertEqual(
            ["--bin", "trust-runtime"],
            selected_target_args(
                [
                    {
                        "path": (
                            "crates/trust-runtime/src/bin/trust-runtime/"
                            "cli/tests.rs"
                        )
                    }
                ]
            ),
        )

    def test_target_selection_routes_nested_module_to_owning_integration_target(
        self,
    ) -> None:
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "crates/trust-runtime/tests/bytecode_vm_core.rs"
            target.parent.mkdir(parents=True)
            target.write_text("mod bytecode_vm_core;\n")

            self.assertEqual(
                ["--test", "bytecode_vm_core"],
                selected_target_args(
                    [
                        {
                            "path": (
                                "crates/trust-runtime/tests/"
                                "bytecode_vm_core/ref_validation.rs"
                            )
                        }
                    ],
                    root=root,
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

    def test_source_cfg_keeps_opcua_wire_test_in_default_shard(self) -> None:
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "tests.rs"
            source.write_text(
                """
#[cfg(feature = "opcua-wire")]
#[test]
fn opcua_case() {}
"""
            )
            self.assertEqual(
                [
                    (
                        "default",
                        [{"path": "tests.rs", "name": "opcua_case"}],
                        False,
                    )
                ],
                selected_feature_shards(
                    root=root,
                    selection=[{"path": "tests.rs", "name": "opcua_case"}],
                ),
            )

    def test_source_cfg_keeps_unix_test_in_default_shard(self) -> None:
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "tests.rs"
            source.write_text(
                """
#[cfg(unix)]
#[test]
fn unix_case() {}
"""
            )
            self.assertEqual(
                [
                    (
                        "default",
                        [{"path": "tests.rs", "name": "unix_case"}],
                        False,
                    )
                ],
                selected_feature_shards(
                    root=root,
                    selection=[{"path": "tests.rs", "name": "unix_case"}],
                ),
            )

    def test_duplicate_bare_name_across_binaries_fails_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            resolve_nextest_listing(
                listing={
                    "rust-suites": {
                        "a": {
                            "binary-name": "a",
                            "package-name": "package",
                            "testcases": {"module::same": {}},
                        },
                        "b": {
                            "binary-name": "b",
                            "package-name": "package",
                            "testcases": {"same": {}},
                        },
                    }
                },
                selected_names={"same"},
            )

    def test_projection_and_normalization_require_exact_passing_set(self) -> None:
        raw = "\n".join(
            [
                '{"type":"test","event":"started","name":"a$unselected"}',
                '{"type":"test","event":"ignored","name":"a$unselected"}',
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
        with self.assertRaisesRegex(ValueError, "unexpected runtime test event"):
            project_raw_results(
                raw=raw
                + '\n{"type":"test","event":"ok","name":"a$unselected",'
                + '"exec_time":0.001}',
                resolved=resolved,
                path_by_name={
                    "one": "src/lib.rs",
                    "two": "tests/two.rs",
                },
            )


if __name__ == "__main__":
    unittest.main()

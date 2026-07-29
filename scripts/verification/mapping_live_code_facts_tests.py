"""Focused tests for exact-path Rust code-fact validation."""

from __future__ import annotations

import json
import subprocess
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

from scripts.verification.code_spec_common import sha256_digest, stable_code_id
from scripts.verification.mapping_live_code_facts import (
    _governed_rust_path,
    validated_live_code_facts,
)


class MappingLiveCodeFactsTests(unittest.TestCase):
    def test_covered_file_uses_live_topology_and_single_file_extractor(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            relative = "crates/example/src/bin/trust-runtime/ads.rs"
            root_relative = "crates/example/src/bin/trust-runtime.rs"
            source = b"pub fn exact() {}\n"
            source_path = root / relative
            source_path.parent.mkdir(parents=True)
            source_path.write_bytes(source)
            (root / "crates/example/Cargo.toml").write_text(
                "[package]\nname = \"example\"\nversion = \"0.0.0\"\n"
            )
            (root / root_relative).write_text(
                '#[path = "trust-runtime/ads.rs"]\nmod ads;\n'
            )
            signature = "pub fn exact()"
            qualified_symbol = "trust_runtime::ads::exact"
            code_id = stable_code_id(
                source_kind="rust",
                path=relative,
                qualified_symbol=qualified_symbol,
                item_kind="function",
                signature=signature,
            )
            document = {
                "schema_version": 1,
                "facts": [
                    {
                        "qualified_symbol": qualified_symbol,
                        "item_kind": "function",
                        "visibility": "pub",
                        "signature": signature,
                        "body_start": 0,
                        "body_end": len(source),
                        "line": 1,
                        "risk_signals": [],
                    }
                ],
                "diagnostics": [],
            }
            mappings = {
                code_id: {
                    "code_id": code_id,
                    "source_kind": "rust",
                    "path": relative,
                    "qualified_symbol": qualified_symbol,
                    "item_kind": "function",
                    "signature_digest": sha256_digest(signature.encode()),
                    "reviewed_body_digest": sha256_digest(source),
                    "area": "compiler_iec",
                }
            }
            trace = {
                "tests": [
                    {
                        "discovery_id": "DISC_" + "A" * 20,
                        "covered_code_ids": [code_id],
                    }
                ]
            }

            topology = {
                "schema_version": 1,
                "selections": [
                    {
                        "schema_version": 1,
                        "selected_path": relative,
                        "bindings": [
                            {
                                "path": relative,
                                "module": "trust_runtime::ads",
                                "topology_paths": [
                                    root_relative,
                                    relative,
                                ],
                            }
                        ],
                        "diagnostics": [],
                    }
                ],
            }
            responses = [
                subprocess.CompletedProcess(
                    args=[],
                    returncode=0,
                    stdout=json.dumps(topology),
                    stderr="",
                ),
                subprocess.CompletedProcess(
                    args=[],
                    returncode=0,
                    stdout=json.dumps(document),
                    stderr="",
                ),
            ]
            with mock.patch(
                "scripts.verification.mapping_live_code_facts.subprocess.run",
                side_effect=responses,
            ) as run:
                facts = validated_live_code_facts(root, trace, mappings)

            self.assertEqual([code_id], list(facts))
            self.assertEqual(2, run.call_count)
            topology_command = run.call_args_list[0].args[0]
            self.assertIn("--topology-for", topology_command)
            command = run.call_args_list[1].args[0]
            self.assertEqual(
                [
                    "cargo",
                    "run",
                    "-q",
                    "-p",
                    "xtask",
                    "--",
                    "code-spec-rust",
                    "--path",
                    relative,
                    "--module",
                    "trust_runtime::ads",
                ],
                command,
            )
            self.assertNotIn("code-spec-rust-all", command)

    def test_live_body_mismatch_rejects_stale_code_map(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            relative = "crates/example/src/lib.rs"
            source = b"fn exact() {}\n"
            source_path = root / relative
            source_path.parent.mkdir(parents=True)
            source_path.write_bytes(source)
            (root / "crates/example/Cargo.toml").write_text(
                "[package]\nname = \"example\"\nversion = \"0.0.0\"\n"
            )
            signature = "fn exact()"
            qualified_symbol = "example::exact"
            code_id = stable_code_id(
                source_kind="rust",
                path=relative,
                qualified_symbol=qualified_symbol,
                item_kind="function",
                signature=signature,
            )
            document = {
                "schema_version": 1,
                "facts": [
                    {
                        "qualified_symbol": qualified_symbol,
                        "item_kind": "function",
                        "visibility": "private",
                        "signature": signature,
                        "body_start": 0,
                        "body_end": len(source),
                        "line": 1,
                        "risk_signals": [],
                    }
                ],
                "diagnostics": [],
            }
            mappings = {
                code_id: {
                    "code_id": code_id,
                    "source_kind": "rust",
                    "path": relative,
                    "qualified_symbol": qualified_symbol,
                    "item_kind": "function",
                    "signature_digest": sha256_digest(signature.encode()),
                    "reviewed_body_digest": "sha256:" + "0" * 64,
                    "area": "compiler_iec",
                }
            }
            trace = {"tests": [{"covered_code_ids": [code_id]}]}
            topology = {
                "schema_version": 1,
                "selections": [
                    {
                        "schema_version": 1,
                        "selected_path": relative,
                        "bindings": [
                            {
                                "path": relative,
                                "module": "example",
                                "topology_paths": [relative],
                            }
                        ],
                        "diagnostics": [],
                    }
                ],
            }
            responses = [
                subprocess.CompletedProcess(
                    args=[],
                    returncode=0,
                    stdout=json.dumps(topology),
                    stderr="",
                ),
                subprocess.CompletedProcess(
                    args=[],
                    returncode=0,
                    stdout=json.dumps(document),
                    stderr="",
                ),
            ]

            with mock.patch(
                "scripts.verification.mapping_live_code_facts.subprocess.run",
                side_effect=responses,
            ), self.assertRaisesRegex(
                ValueError,
                "reviewed_body_digest differs from code map",
            ):
                validated_live_code_facts(root, trace, mappings)

    def test_include_fragment_resolves_parent_module_from_live_topology(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            relative = "crates/example/src/database/database_part_01.rs"
            root_relative = "crates/example/src/lib.rs"
            parent_relative = "crates/example/src/database.rs"
            source = b"impl Database { pub fn new() {} }\n"
            source_path = root / relative
            source_path.parent.mkdir(parents=True)
            source_path.write_bytes(source)
            (root / "crates/example/Cargo.toml").write_text(
                "[package]\nname = \"example\"\nversion = \"0.0.0\"\n"
            )
            (root / root_relative).write_text("mod database;\n")
            (root / parent_relative).write_text(
                'include!("database/database_part_01.rs");\n'
            )
            signature = "pub fn new()"
            actual_module = "example::database"
            expected_symbol = f"{actual_module}::Database::new"
            code_id = stable_code_id(
                source_kind="rust",
                path=relative,
                qualified_symbol=expected_symbol,
                item_kind="method",
                signature=signature,
            )
            mappings = {
                code_id: {
                    "code_id": code_id,
                    "source_kind": "rust",
                    "path": relative,
                    "qualified_symbol": expected_symbol,
                    "item_kind": "method",
                    "signature_digest": sha256_digest(signature.encode()),
                    "reviewed_body_digest": sha256_digest(source),
                    "area": "compiler_iec",
                }
            }
            trace = {"tests": [{"covered_code_ids": [code_id]}]}

            def extractor(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess:
                if "--topology-for" in command:
                    self.assertEqual(
                        [root_relative, "example", relative],
                        [
                            command[command.index("--path") + 1],
                            command[command.index("--module") + 1],
                            command[command.index("--topology-for") + 1],
                        ],
                    )
                    topology = {
                        "schema_version": 1,
                        "selections": [
                            {
                                "schema_version": 1,
                                "selected_path": relative,
                                "bindings": [
                                    {
                                        "path": relative,
                                        "module": actual_module,
                                        "topology_paths": [
                                            root_relative,
                                            parent_relative,
                                            relative,
                                        ],
                                    }
                                ],
                                "diagnostics": [],
                            }
                        ],
                    }
                    return subprocess.CompletedProcess(
                        args=command,
                        returncode=0,
                        stdout=json.dumps(topology),
                        stderr="",
                    )
                module = command[command.index("--module") + 1]
                document = {
                    "schema_version": 1,
                    "facts": [
                        {
                            "qualified_symbol": f"{module}::Database::new",
                            "item_kind": "method",
                            "visibility": "pub",
                            "signature": signature,
                            "body_start": 0,
                            "body_end": len(source),
                            "line": 1,
                            "risk_signals": [],
                        }
                    ],
                    "diagnostics": [],
                }
                return subprocess.CompletedProcess(
                    args=command,
                    returncode=0,
                    stdout=json.dumps(document),
                    stderr="",
                )

            with mock.patch(
                "scripts.verification.mapping_live_code_facts.subprocess.run",
                side_effect=extractor,
            ) as run:
                facts = validated_live_code_facts(root, trace, mappings)

            self.assertEqual([code_id], list(facts))
            self.assertEqual(2, run.call_count)
            extract_command = run.call_args_list[1].args[0]
            self.assertEqual(
                actual_module,
                extract_command[extract_command.index("--module") + 1],
            )

    def test_include_fragment_resolves_module_inside_trait_impl_symbol(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            relative = "crates/example/src/database/database_part_02.rs"
            source = b"impl SemanticDatabase for Database { fn analyze() {} }\n"
            source_path = root / relative
            source_path.parent.mkdir(parents=True)
            source_path.write_bytes(source)
            root_relative = "crates/example/src/lib.rs"
            parent_relative = "crates/example/src/database.rs"
            (root / "crates/example/Cargo.toml").write_text(
                "[package]\nname = \"example\"\nversion = \"0.0.0\"\n"
            )
            (root / root_relative).write_text("mod database;\n")
            (root / parent_relative).write_text(
                'include!("database/database_part_02.rs");\n'
            )
            signature = "fn analyze()"
            actual_module = "example::database"

            def symbol(module: str) -> str:
                return f"<{module}::Database as SemanticDatabase>::analyze"

            expected_symbol = symbol(actual_module)
            code_id = stable_code_id(
                source_kind="rust",
                path=relative,
                qualified_symbol=expected_symbol,
                item_kind="trait_method",
                signature=signature,
            )
            mappings = {
                code_id: {
                    "code_id": code_id,
                    "source_kind": "rust",
                    "path": relative,
                    "qualified_symbol": expected_symbol,
                    "item_kind": "trait_method",
                    "signature_digest": sha256_digest(signature.encode()),
                    "reviewed_body_digest": sha256_digest(source),
                    "area": "compiler_iec",
                }
            }

            def extractor(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess:
                if "--topology-for" in command:
                    topology = {
                        "schema_version": 1,
                        "selections": [
                            {
                                "schema_version": 1,
                                "selected_path": relative,
                                "bindings": [
                                    {
                                        "path": relative,
                                        "module": actual_module,
                                        "topology_paths": [
                                            root_relative,
                                            parent_relative,
                                            relative,
                                        ],
                                    }
                                ],
                                "diagnostics": [],
                            }
                        ],
                    }
                    return subprocess.CompletedProcess(
                        args=command,
                        returncode=0,
                        stdout=json.dumps(topology),
                        stderr="",
                    )
                module = command[command.index("--module") + 1]
                document = {
                    "schema_version": 1,
                    "facts": [
                        {
                            "qualified_symbol": symbol(module),
                            "item_kind": "trait_method",
                            "visibility": "private",
                            "signature": signature,
                            "body_start": 0,
                            "body_end": len(source),
                            "line": 1,
                            "risk_signals": [],
                        }
                    ],
                    "diagnostics": [],
                }
                return subprocess.CompletedProcess(
                    args=command,
                    returncode=0,
                    stdout=json.dumps(document),
                    stderr="",
                )

            with mock.patch(
                "scripts.verification.mapping_live_code_facts.subprocess.run",
                side_effect=extractor,
            ):
                facts = validated_live_code_facts(
                    root,
                    {"tests": [{"covered_code_ids": [code_id]}]},
                    mappings,
                )

            self.assertEqual([code_id], list(facts))

    def test_multiple_paths_share_one_live_target_topology_scan(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            crate = root / "crates/example"
            source_root = crate / "src"
            source_root.mkdir(parents=True)
            (crate / "Cargo.toml").write_text(
                "[package]\nname = \"example\"\nversion = \"0.0.0\"\n"
            )
            root_relative = "crates/example/src/lib.rs"
            paths = [
                "crates/example/src/first.rs",
                "crates/example/src/second.rs",
            ]
            (root / root_relative).write_text("mod first;\nmod second;\n")
            sources = {
                paths[0]: b"pub fn first() {}\n",
                paths[1]: b"pub fn second() {}\n",
            }
            mappings = {}
            covered = []
            for relative, source in sources.items():
                (root / relative).write_bytes(source)
                name = Path(relative).stem
                signature = f"pub fn {name}()"
                qualified_symbol = f"example::{name}::{name}"
                code_id = stable_code_id(
                    source_kind="rust",
                    path=relative,
                    qualified_symbol=qualified_symbol,
                    item_kind="function",
                    signature=signature,
                )
                covered.append(code_id)
                mappings[code_id] = {
                    "code_id": code_id,
                    "source_kind": "rust",
                    "path": relative,
                    "qualified_symbol": qualified_symbol,
                    "item_kind": "function",
                    "signature_digest": sha256_digest(signature.encode()),
                    "reviewed_body_digest": sha256_digest(source),
                    "area": "compiler_iec",
                }

            def extractor(
                command: list[str], **_kwargs: object
            ) -> subprocess.CompletedProcess:
                if "--topology-for" in command:
                    selected = [
                        command[index + 1]
                        for index, value in enumerate(command)
                        if value == "--topology-for"
                    ]
                    topology = {
                        "schema_version": 1,
                        "selections": [
                            {
                                "schema_version": 1,
                                "selected_path": relative,
                                "bindings": [
                                    {
                                        "path": relative,
                                        "module": (
                                            f"example::{Path(relative).stem}"
                                        ),
                                        "topology_paths": [
                                            root_relative,
                                            relative,
                                        ],
                                    }
                                ],
                                "diagnostics": [],
                            }
                            for relative in selected
                        ],
                    }
                    return subprocess.CompletedProcess(
                        args=command,
                        returncode=0,
                        stdout=json.dumps(topology),
                        stderr="",
                    )
                relative = command[command.index("--path") + 1]
                module = command[command.index("--module") + 1]
                source = sources[relative]
                name = Path(relative).stem
                document = {
                    "schema_version": 1,
                    "facts": [
                        {
                            "qualified_symbol": f"{module}::{name}",
                            "item_kind": "function",
                            "visibility": "pub",
                            "signature": f"pub fn {name}()",
                            "body_start": 0,
                            "body_end": len(source),
                            "line": 1,
                            "risk_signals": [],
                        }
                    ],
                    "diagnostics": [],
                }
                return subprocess.CompletedProcess(
                    args=command,
                    returncode=0,
                    stdout=json.dumps(document),
                    stderr="",
                )

            with mock.patch(
                "scripts.verification.mapping_live_code_facts.subprocess.run",
                side_effect=extractor,
            ) as run:
                facts = validated_live_code_facts(
                    root,
                    {"tests": [{"covered_code_ids": covered}]},
                    mappings,
                )

            self.assertEqual(set(covered), set(facts))
            topology_calls = [
                call
                for call in run.call_args_list
                if "--topology-for" in call.args[0]
            ]
            self.assertEqual(1, len(topology_calls))
            self.assertEqual(3, run.call_count)

    def test_governed_path_validation_does_not_infer_module_authority(self) -> None:
        self.assertEqual(
            "crates/trust-hir/src/types/resolve.rs",
            _governed_rust_path(
                "crates/trust-hir/src/types/resolve.rs"
            ).as_posix(),
        )
        self.assertEqual(
            "crates/trust-runtime/src/bin/worker/protocol.rs",
            _governed_rust_path(
                "crates/trust-runtime/src/bin/worker/protocol.rs"
            ).as_posix(),
        )


if __name__ == "__main__":
    unittest.main()

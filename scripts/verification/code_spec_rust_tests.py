"""Focused tests for the Phase 17 Rust extractor adapter."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.verification.code_spec_common import sha256_digest, stable_code_id
from scripts.verification.code_spec_rust import CodeSpecRustError, rust_inventory_from_scan


def matrix(*, areas: tuple[str, ...] = ("runtime_safety",)) -> dict[str, object]:
    return {
        "areas": [
            {"id": area, "owner": "trust-runtime" if area == "runtime_safety" else "language"}
            for area in areas
        ],
        "code_areas": [
            {
                "id": "fixture_route",
                "match_kind": "path",
                "area_ids": list(areas),
                "path_globs": ["crates/example/**"],
                "required_test_classes": [],
                "suite_tiers": ["pr"],
                "conditional_suite_tiers": [],
            }
        ],
    }


def scan(path: str, body: bytes, *, diagnostics: list[str] | None = None) -> dict[str, object]:
    return {
        "schema_version": 1,
        "cargo_targets": [
            {
                "package": "example",
                "target": "example",
                "target_kind": "lib",
                "path": path,
                "module": "example",
            }
        ],
        "sources": [
            {
                "path": path,
                "module": "example",
                "cargo_bindings": ["example:example:lib"],
                "facts": [
                    {
                        "qualified_symbol": "example::run",
                        "item_kind": "rust_function",
                        "visibility": "private",
                        "signature": "fn run()",
                        "body_start": 0,
                        "body_end": len(body),
                        "line": 1,
                        "risk_signals": [],
                    }
                ],
            }
        ],
        "diagnostics": diagnostics or [],
    }


class RustAdapterTests(unittest.TestCase):
    def test_exact_source_bytes_become_canonical_live_fact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = "crates/example/src/lib.rs"
            body = "fn run() { café(); }".encode()
            source = root / path
            source.parent.mkdir(parents=True)
            source.write_bytes(body)

            inventory = rust_inventory_from_scan(root, scan(path, body), matrix())

        self.assertEqual(1, inventory.target_count)
        self.assertEqual(1, inventory.source_count)
        self.assertEqual((path,), inventory.source_paths)
        self.assertEqual(1, len(inventory.facts))
        code_fact = next(iter(inventory.facts.values()))
        self.assertEqual(
            stable_code_id(
                source_kind="rust",
                path=path,
                qualified_symbol="example::run",
                item_kind="rust_function",
                signature="fn run()",
            ),
            code_fact.code_id,
        )
        self.assertEqual(sha256_digest(body), code_fact.body_digest)
        self.assertEqual(sha256_digest(b"fn run()"), code_fact.signature_digest)
        self.assertEqual("runtime_safety", code_fact.area)
        self.assertEqual("trust-runtime", code_fact.owner)
        self.assertEqual(("example:example:lib",), inventory.cargo_bindings[code_fact.code_id])

    def test_unsafe_fact_gets_closed_risk_signal(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = "crates/example/src/lib.rs"
            body = b"unsafe { raw(); }"
            source = root / path
            source.parent.mkdir(parents=True)
            source.write_bytes(body)
            payload = scan(path, body)
            payload["sources"][0]["facts"][0]["item_kind"] = "rust_unsafe_site"
            payload["sources"][0]["facts"][0]["signature"] = "unsafe"
            payload["sources"][0]["facts"][0]["risk_signals"] = ["unsafe_or_ffi"]

            inventory = rust_inventory_from_scan(root, payload, matrix())

        code_fact = next(iter(inventory.facts.values()))
        self.assertEqual(("unsafe_or_ffi",), code_fact.risk_signals)

    def test_diagnostics_and_empty_facts_block_while_exact_binding_resolves_ambiguous_area(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = "crates/example/src/lib.rs"
            body = b"fn run() {}"
            source = root / path
            source.parent.mkdir(parents=True)
            source.write_bytes(body)

            with self.assertRaisesRegex(CodeSpecRustError, "extractor diagnostic: unsupported"):
                rust_inventory_from_scan(root, scan(path, body, diagnostics=["unsupported"]), matrix())
            empty = scan(path, body)
            empty["sources"][0]["facts"] = []
            with self.assertRaisesRegex(CodeSpecRustError, "zero Rust facts"):
                rust_inventory_from_scan(root, empty, matrix())
            ambiguous = rust_inventory_from_scan(
                root,
                scan(path, body),
                matrix(areas=("runtime_safety", "compiler_iec")),
            )
            code_id = next(iter(ambiguous.facts))
            self.assertEqual("unmapped", ambiguous.facts[code_id].area)
            self.assertEqual((code_id,), ambiguous.unmapped_code_ids)

            bound = rust_inventory_from_scan(
                root,
                scan(path, body),
                matrix(areas=("runtime_safety", "compiler_iec")),
                area_bindings={code_id: "compiler_iec"},
            )
            self.assertEqual("compiler_iec", bound.facts[code_id].area)
            self.assertEqual("language", bound.facts[code_id].owner)
            self.assertEqual((), bound.unmapped_code_ids)
            with self.assertRaisesRegex(CodeSpecRustError, "not permitted by path routing"):
                rust_inventory_from_scan(
                    root,
                    scan(path, body),
                    matrix(areas=("runtime_safety", "compiler_iec")),
                    area_bindings={code_id: "release"},
                )

    def test_duplicate_identity_invalid_range_escape_and_symlink_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = "crates/example/src/lib.rs"
            body = b"fn run() {}"
            source = root / path
            source.parent.mkdir(parents=True)
            source.write_bytes(body)

            duplicate = scan(path, body)
            duplicate["sources"][0]["facts"].append(dict(duplicate["sources"][0]["facts"][0]))
            with self.assertRaisesRegex(CodeSpecRustError, "duplicate live code identity"):
                rust_inventory_from_scan(root, duplicate, matrix())

            invalid_range = scan(path, body)
            invalid_range["sources"][0]["facts"][0]["body_end"] = len(body) + 1
            with self.assertRaisesRegex(CodeSpecRustError, "invalid body byte range"):
                rust_inventory_from_scan(root, invalid_range, matrix())

            escaping = scan("../outside.rs", body)
            with self.assertRaisesRegex(CodeSpecRustError, "repository-relative"):
                rust_inventory_from_scan(root, escaping, matrix())

            target = root / "actual.rs"
            target.write_bytes(body)
            source.unlink()
            source.symlink_to(target)
            with self.assertRaisesRegex(CodeSpecRustError, "symlinked source"):
                rust_inventory_from_scan(root, scan(path, body), matrix())


if __name__ == "__main__":
    unittest.main()

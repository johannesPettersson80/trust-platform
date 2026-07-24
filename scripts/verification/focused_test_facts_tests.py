"""Focused tests for exact per-file discovery fact reconstruction."""

from __future__ import annotations

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from scripts.verification.focused_test_facts import resolve_focused_rust_facts
from scripts.verification.test_catalog_common import stable_discovery_id


class FocusedTestFactsTests(unittest.TestCase):
    def test_resolves_only_requested_live_rust_file_from_denominator(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "crates/example/src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text("#[test]\nfn exact_case() { assert!(true); }\n")
            manifest = root / "crates/example/Cargo.toml"
            manifest.write_text('[package]\nname = "example"\nversion = "0.1.0"\n')
            discovery_id = stable_discovery_id(
                source_kind="rust_unit_test",
                package="example",
                native_id="crates/example/src/lib.rs#exact_case",
            )
            denominator = _denominator(
                discovery_id=discovery_id,
                source_kind="rust_unit_test",
                path="crates/example/src/lib.rs",
                name="exact_case",
            )

            facts = resolve_focused_rust_facts(
                root=root,
                discovery_ids=[discovery_id],
                denominator=denominator,
            )

            self.assertEqual([discovery_id], [fact.stable_id for fact in facts])
            self.assertEqual("example", facts[0].package)
            self.assertEqual("exact_case", facts[0].name)

    def test_rejects_stale_review_instead_of_falling_back_to_full_scan(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "crates/example/src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text("#[test]\nfn live_case() {}\n")
            (root / "crates/example/Cargo.toml").write_text(
                '[package]\nname = "example"\nversion = "0.1.0"\n'
            )
            discovery_id = stable_discovery_id(
                source_kind="rust_unit_test",
                package="example",
                native_id="crates/example/src/lib.rs#stale_case",
            )

            with self.assertRaisesRegex(
                ValueError,
                "absent from its reviewed live source file",
            ):
                resolve_focused_rust_facts(
                    root=root,
                    discovery_ids=[discovery_id],
                    denominator=_denominator(
                        discovery_id=discovery_id,
                        source_kind="rust_unit_test",
                        path="crates/example/src/lib.rs",
                        name="stale_case",
                    ),
                )

    def test_rejects_non_rust_fact_on_rust_focused_route(self) -> None:
        discovery_id = "DISC_" + "A" * 20
        with TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(ValueError, "is not a Rust test"):
                resolve_focused_rust_facts(
                    root=Path(temporary),
                    discovery_ids=[discovery_id],
                    denominator=_denominator(
                        discovery_id=discovery_id,
                        source_kind="vscode_test",
                        path="editors/vscode/src/test/example.test.ts",
                        name="example",
                    ),
                )


def _denominator(
    *,
    discovery_id: str,
    source_kind: str,
    path: str,
    name: str,
) -> dict[str, object]:
    return {
        "reviews": [
            {
                "discovery_id": discovery_id,
                "discovery_source_kind": source_kind,
                "path": path,
                "name": name,
                "ignore_state": "not_ignored",
                "disposition": "reviewed_nonmapping",
                "reason_code": "no_reviewed_spec_or_invariant_binding",
                "last_reviewed": "2026-07-24",
            }
        ]
    }


if __name__ == "__main__":
    unittest.main()

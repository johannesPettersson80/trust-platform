from __future__ import annotations

import unittest

from scripts.verification.frozen_catalog_identity import digest_document
from scripts.verification.frozen_invariant_behavior_binding import (
    validate_invariant_behavior_digests,
)


class FrozenInvariantBehaviorBindingTests(unittest.TestCase):
    def test_rejects_changed_behavior_rows(self) -> None:
        invariants = {
            "INV_ONE": {
                "behavior": [{"outcome": "accept_value"}],
            }
        }
        plan = {
            "invariant_behavior_digests": {
                "INV_ONE": digest_document(
                    {"behavior": invariants["INV_ONE"]["behavior"]}
                )
            }
        }

        validate_invariant_behavior_digests(
            plan=plan,
            invariant_ids=("INV_ONE",),
            invariants=invariants,
        )
        invariants["INV_ONE"]["behavior"][0]["outcome"] = "reject"
        with self.assertRaisesRegex(ValueError, "behavior rows drifted"):
            validate_invariant_behavior_digests(
                plan=plan,
                invariant_ids=("INV_ONE",),
                invariants=invariants,
            )


if __name__ == "__main__":
    unittest.main()

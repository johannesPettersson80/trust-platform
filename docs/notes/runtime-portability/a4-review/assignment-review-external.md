# External-binding shadowing review addendum

Source-inspection verdict: no remaining actionable finding in this correction.
No tests, builds, formatters or validators ran.

The [seven-file manifest](assignment-review-external.json) records exact current
per-file hashes, available prior review hashes, and aggregate SHA-256
`d67e4ceb32b6e7ede71817eeb075ddf4b27f37d3294dc1505b95d4410073fa8f` using sorted relative path/NUL/raw-bytes/NUL framing.
It supplements the earlier [assignment/context review](assignment-review.md);
that review's own-code exclusions still apply. This addendum does not turn either
source review into execution evidence.

The wire validator permits non-owning External records without allocating a
physical slot and assigns them zero construction demand. Excluding those records
from physical permission, visibility, type, global-name, staged-owner and retained
program lookups matches that contract. The retained-global graph already selects
physical Variable records.

The regression prepends a compatible External alias before the protected global,
clears the alias's constant/default fields, sets zero construction demand, and
updates all declaration-table indexes in roots, initializers and related records.
Other table identities remain unchanged. Its assertions require successful
admission followed by a constant-write fault, preserved value 7 and latched fault;
they have not been executed.

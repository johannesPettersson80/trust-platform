# Verification Inventory Contract

Specification identity: `SPEC_VERIFICATION_INVENTORY_001`.

This document defines repository-governance behavior for inventories that link
verification metadata to test source. It does not define Structured Text
language behavior and does not provide IEC 61131-3 product proof.

## IEC table test inventory

The canonical IEC table inventory is the tracked repository file
`docs/specs/coverage/iec-table-test-map.toml`. The inventory check must read
that exact file and parse it as TOML containing table entries with an
identifier, an optional specification path, and a list of Rust test names. An
unreadable or malformed map is a failed inventory check.

For this check, the Rust test inventory consists of test functions discovered
recursively below the repository `crates/` and `tests/` trees. Discovery
considers Rust source files, skips generated or dependency trees named
`target`, `.git`, and `node_modules`, and recognizes functions following
`#[test]` or `#[tokio::test]`, including intervening attributes. Test names are
matched exactly.

Every test name listed by every table entry must resolve to at least one source
path in that Rust test inventory. A name that resolves nowhere is rendered as
missing and fails the check. A name that resolves in more than one source file
is not silently collapsed: every resolving path is displayed.

The human-readable report preserves table-entry and listed-test order from the
map. For each resolved name, displayed source paths are repository-relative,
use `/` separators, and are sorted lexically. The complete report is compared
with its reviewed snapshot so additions, removals, path changes, and ordering
changes require explicit review.

## Evidence boundary

This inventory establishes only that the map is structurally readable, each
listed name exists in the scanned Rust source inventory, and the report is
deterministic and review-locked. It does not:

- execute any listed test;
- establish that a test passes, reaches production behavior, or has an
  adequate assertion;
- establish that every relevant IEC table or every relevant test is listed;
- resolve a test name to a unique source file;
- prove IEC conformance or any IEC language semantic claim.

IEC product proof requires separate specification, invariant, catalog, oracle,
execution, and evidence bindings. This governance inventory cannot substitute
for any of them.

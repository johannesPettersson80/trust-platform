//! TIME-08 / TIME-09: the hosted default preparation limits state their work bounds explicitly.
use trust_runtime_core::vm::PreparationLimits;

#[test]
fn hosted_defaults_state_one_million_units_per_invocation_and_64_mi_construction_units() {
    let limits = PreparationLimits::default();
    // Spec 34 TIME-08 and spec 10: one top-level VM invocation may use 1,000,000 units.
    assert_eq!(limits.max_work, 1_000_000);
    // Spec 34 TIME-09: each construction entry may use 64 Mi units, equal to the default
    // preparation work bound.
    assert_eq!(limits.max_construction_work, 64 * 1024 * 1024);
    assert_eq!(limits.max_preparation_work, 64 * 1024 * 1024);
}

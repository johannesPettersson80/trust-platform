/// HIR authoring adapter for the shared native state contract.
pub(crate) fn builtin_state_layout(
    kind: super::BuiltinFbKind,
) -> Vec<(&'static str, trust_hir::TypeId)> {
    trust_runtime_core::stdlib::hosted::state::builtin_state_layout(kind)
        .iter()
        .map(|(name, primitive)| {
            (
                *name,
                match primitive {
                    1 => trust_hir::TypeId::BOOL,
                    17 => trust_hir::TypeId::LTIME,
                    _ => unreachable!("closed native state contract"),
                },
            )
        })
        .collect()
}

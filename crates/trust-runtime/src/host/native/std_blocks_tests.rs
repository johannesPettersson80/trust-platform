use super::StdFb;
use crate::stdlib::fbs::builtin_kind;

// Specification 08: the compiler lowers every standard function block the analyzer declares,
// and the host's registry knows it. Catches a block the analyzer accepts that the compiler then
// cannot lower (F24 had to add one to both lists), and a name that no specification gives.
#[test]
fn every_standard_function_block_lowers_and_no_other_name_does() {
    for name in trust_hir::symbols::STANDARD_FUNCTION_BLOCKS {
        assert!(
            StdFb::named(name).is_some(),
            "the compiler does not know {name}"
        );
        assert!(
            builtin_kind(name).is_some(),
            "the host's registry does not know {name}"
        );
    }
    for name in [
        "TON_DINT",
        "TP_DATE",
        "CTU_SINT",
        "CTUD_UINT",
        "DIF",
        "R_TRIGGER",
    ] {
        assert!(StdFb::named(name).is_none(), "{name}");
        assert!(builtin_kind(name).is_none(), "{name}");
    }
}

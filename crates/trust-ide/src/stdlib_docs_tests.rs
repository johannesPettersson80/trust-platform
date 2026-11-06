use super::STANDARD_FB_NAMES;

// Specification 08: hover documents every standard function block that completion offers.
// Catches a block the analyzer declares without documentation, as DIFU, DIFD and the `_TIME`
// timers were.
#[test]
fn every_standard_function_block_has_its_documentation() {
    let mut documented: Vec<&str> = STANDARD_FB_NAMES.iter().map(|(name, _)| *name).collect();
    documented.sort_unstable();
    let mut listed = trust_hir::symbols::STANDARD_FUNCTION_BLOCKS.to_vec();
    listed.sort_unstable();
    assert_eq!(documented, listed);
}

// Specification 14, `stdlib` profiles: `iec` offers the IEC blocks of Tables 43 to 46, the
// `_TIME` timers included, and not Mitsubishi's DIFU and DIFD; `none` offers no standard block.
// Catches the aliases offered as IEC names, and a standard block that no profile filters because
// the editor does not know it as standard.
#[test]
fn the_iec_profile_offers_the_iec_blocks_and_none_offers_none() {
    let iec = super::StdlibFilter::from_profile("iec");
    let none = super::StdlibFilter::from_profile("none");
    for name in trust_hir::symbols::STANDARD_FUNCTION_BLOCKS {
        assert!(super::is_standard_fb_name(name), "{name}");
        let alias = trust_hir::symbols::STANDARD_FUNCTION_BLOCK_ALIASES.contains(&name);
        assert_eq!(iec.allows_function_block(name), !alias, "{name}");
        assert!(!none.allows_function_block(name), "{name}");
    }
}

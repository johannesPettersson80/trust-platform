//! Specification 39, §3.3 item 8: a group is invisible, each member recording `group = "<name>"`; a
//! locked object records `locked = true`. Both are kept and written back byte for byte.

use super::fixtures::FILE;
use super::*;

const PAGE: &str = r#"format = 1
title = "Groups"

[[object]]
id = "p-101"
type = "pump"
at = [40, 40]
size = [128, 108]
group = "feed"
locked = true

[[object]]
id = "value-1"
type = "value"
at = [200, 40]
size = [240, 96]
group = "feed"
"#;

#[test]
fn a_group_and_a_lock_are_kept_and_written_back_byte_for_byte() {
    let page = parse_page(FILE, PAGE).expect("the page loads");
    assert_eq!(page.objects[0].group.as_deref(), Some("feed"));
    assert!(page.objects[0].locked);
    assert!(!page.objects[1].locked);
    assert_eq!(write_page(&page), PAGE);
}

//! Specification 39, §18 phase 4 item 2: a page names the P&ID whose process objects it shares.

use super::fixtures::FILE;
use super::*;

#[test]
fn a_page_names_its_pid_and_writes_it_back() {
    let text = "format = 1\ntitle = \"Overview\"\npid = \"tank-farm\"\n";
    let page = parse_page(FILE, text).expect("valid");
    assert_eq!(page.pid.as_deref(), Some("tank-farm"));
    assert_eq!(write_page(&page), text, "written back as read");
}

#[test]
fn a_pid_is_named_as_a_file_is() {
    let errors = parse_page(
        FILE,
        "format = 1\ntitle = \"Overview\"\npid = \"../other\"\n",
    )
    .expect_err("refused");
    assert_eq!(errors[0].line, 3, "{errors:?}");
    assert!(
        errors[0].message.contains("pid names a P&ID"),
        "{}",
        errors[0].message
    );
}

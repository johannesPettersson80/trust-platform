//! Specification 39, §4 item 5 and §5.1: a panel or group says how it rearranges its objects on a
//! smaller screen, with `arrange` and `gap`.

use super::fixtures::FILE;
use super::*;

fn page(keys: &str, kind: &str) -> String {
    format!("format = 1\ntitle = \"Overview\"\n\n[[object]]\nid = \"pumps\"\ntype = \"{kind}\"\nat = [24, 24]\nsize = [640, 240]\n{keys}")
}

#[test]
fn a_panel_reads_and_writes_its_arrangement() {
    let text = page("arrange = \"row\"\ngap = 12\n", "panel");
    let parsed = parse_page(FILE, &text).expect("valid");
    assert_eq!(parsed.objects[0].arrange, Some(HmiArrange::Row));
    assert_eq!(parsed.objects[0].gap, Some(12));
    assert_eq!(write_page(&parsed), text, "written back as read");
    for (word, arrange) in [("column", HmiArrange::Column), ("grid", HmiArrange::Grid)] {
        let parsed =
            parse_page(FILE, &page(&format!("arrange = \"{word}\"\n"), "group")).expect("valid");
        assert_eq!(parsed.objects[0].arrange, Some(arrange), "{word}");
    }
}

#[test]
fn an_arrangement_is_one_of_three_and_only_for_panels_and_groups() {
    let errors = parse_page(FILE, &page("arrange = \"diagonal\"\n", "panel")).expect_err("refused");
    assert_eq!(errors[0].line, 9, "{errors:?}");
    assert!(
        errors[0]
            .message
            .contains("arrange must be row, column or grid"),
        "{}",
        errors[0].message
    );
    let errors = parse_page(FILE, &page("arrange = \"row\"\n", "value")).expect_err("refused");
    assert!(
        errors[0]
            .message
            .contains("arrange is for panels and groups"),
        "{}",
        errors[0].message
    );
    let errors = parse_page(FILE, &page("gap = 8\n", "pump")).expect_err("refused");
    assert!(
        errors[0].message.contains("gap is for panels and groups"),
        "{}",
        errors[0].message
    );
}

//! Specification 39, §8.9 item 3: a faceplate placed on a page is one object of type `faceplate`
//! naming the faceplate and the instance it is bound to; it is written back canonically, and each
//! key is checked at its line.

use super::fixtures::FILE;
use super::*;

const PAGE: &str = r#"format = 1
title = "Pumps"

[[object]]
id = "card-1"
tag = "P-101"
type = "faceplate"
faceplate = "pump"
instance = "Main.P101"
at = [40, 40]
size = [400, 300]

[[object]]
id = "card-2"
type = "faceplate"
faceplate = "valve"
instance = "Main.V102"
at = [460, 40]
size = [200, 150]
"#;

fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .expect("the needle is in the text")
        + 1
}

#[test]
fn a_placed_faceplate_names_its_faceplate_and_instance_and_is_written_back_byte_for_byte() {
    let page = parse_page(FILE, PAGE).expect("the page loads");
    assert_eq!(page.objects[0].faceplate.as_deref(), Some("pump"));
    assert_eq!(page.objects[0].instance.as_deref(), Some("Main.P101"));
    assert_eq!(page.objects[1].faceplate.as_deref(), Some("valve"));
    assert_eq!(page.objects[1].instance.as_deref(), Some("Main.V102"));
    assert_eq!(write_page(&page), PAGE);
}

#[test]
fn a_placed_panel_without_its_definition_is_an_error_at_its_id() {
    let removed = "faceplate = \"valve\"\n";
    let message = "a placed faceplate names its faceplate: faceplate = \"<name>\"";
    let text = PAGE.replace(removed, "");
    let errors = parse_page(FILE, &text).expect_err(message);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        errors[0].line,
        line_of(&text, "id = \"card-2\""),
        "{message}"
    );
    assert!(errors[0].message.contains(message), "{}", errors[0].message);
}

const EQUIPMENT: &str = r#"format = 1
title = "Overview"

[[object]]
id = "p-101"
tag = "P-101"
type = "pump"
block = "Plant.FB_Pump"
faceplate = "plant-run"
instance = "Main.P101"
at = [40, 40]
size = [128, 108]
"#;

#[test]
fn equipment_names_its_block_panel_and_instance_and_opens_that_panel() {
    // Specification 39 §21.1: the editor's equipment object is the format every reader accepts.
    let page = parse_page(FILE, EQUIPMENT).expect("the page loads");
    let pump = &page.objects[0];
    assert_eq!(pump.block.as_deref(), Some("Plant.FB_Pump"));
    assert_eq!(pump.faceplate.as_deref(), Some("plant-run"));
    assert_eq!(pump.instance.as_deref(), Some("Main.P101"));
    assert_eq!(
        pump.effective_click(),
        Some(HmiClick::Faceplate {
            name: "plant-run".to_string(),
            instance: Some("Main.P101".to_string()),
        }),
        "a click opens the equipment's panel for its instance"
    );
    assert_eq!(
        write_page(&page),
        EQUIPMENT,
        "it is written back byte for byte"
    );
}

#[test]
fn an_object_click_takes_precedence_over_the_equipment_panel() {
    let text = format!("{EQUIPMENT}\n[object.click]\npage = \"trends\"\n");
    let page = parse_page(FILE, &text).expect("the page loads");
    assert_eq!(
        page.objects[0].effective_click(),
        Some(HmiClick::Page("trends".to_string()))
    );
}

#[test]
fn a_block_that_is_not_a_type_name_is_an_error_at_its_line() {
    let text = EQUIPMENT.replace("block = \"Plant.FB_Pump\"", "block = \"Plant.FB Pump\"");
    let errors = parse_page(FILE, &text).expect_err("not a type name");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].line, line_of(&text, "block = "));
    assert!(errors[0].message.contains("not a function block type name"));
}

#[test]
fn a_problem_with_a_placed_faceplates_instance_is_at_its_instance_line() {
    assert_eq!(
        problem_line(PAGE, "card-2", Some("instance")),
        line_of(PAGE, "instance = \"Main.V102\"")
    );
}

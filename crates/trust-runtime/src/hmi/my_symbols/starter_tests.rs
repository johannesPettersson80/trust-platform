//! The imported symbol's starting definition (specification 39, §14.4 item 5).

use super::starter_definition;

fn size_line(drawing: &str) -> String {
    starter_definition("Pump", drawing)
        .unwrap()
        .lines()
        .find(|line| line.starts_with("size = "))
        .unwrap()
        .to_owned()
}

#[test]
fn the_size_keeps_the_drawings_proportions_above_the_tag_and_state_word() {
    // A symbol drawn 64 wide; 128 wide and 44 high for the tag and state word under it. Taking
    // the drawing's own width as the object's would make a 1000-unit drawing a 1000-pixel object.
    assert_eq!(
        size_line(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 64"/>"#),
        "size = [128, 76]",
        "64 × 64/128 = 32 high"
    );
    assert_eq!(
        size_line(r#"<svg xmlns="http://www.w3.org/2000/svg" width="48px" height="96px"/>"#),
        "size = [128, 172]",
        "without a viewBox the width and height give the proportions: 64 × 96/48 = 128"
    );
    assert_eq!(
        size_line(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0,0,1000,10"/>"#),
        "size = [128, 60]",
        "a flat drawing is still drawn 16 high"
    );
    assert_eq!(
        size_line(r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#),
        "size = [128, 108]",
        "a drawing without any size is taken as square"
    );
}

#[test]
fn each_element_id_is_offered_once_as_a_part_in_the_drawings_order() {
    let text = starter_definition(
        "Pump \"A\"",
        r#"<svg xmlns="http://www.w3.org/2000/svg" id="root" viewBox="0 0 64 64"><g id="rotor"><path id="blade" d="M0 0"/></g><rect id="rotor"/></svg>"#,
    )
    .unwrap();
    assert_eq!(
        text,
        "format = 1\n\
         title = \"Pump \\\"A\\\"\"\n\
         size = [128, 108]\n\
         \n\
         # The bindings the symbol takes, each boolean, number or text, for example:\n\
         [properties]\n\
         # running = \"boolean\"\n\
         # level = \"number\"\n\
         \n\
         # Ports where pipes connect, in the drawing's coordinates, for example:\n\
         # [[port]]\n\
         # name = \"inlet\"\n\
         # at = [0, 32]\n\
         # direction = \"left\"\n\
         \n\
         # The drawing's elements. Name the ones the bindings drive, each one way:\n\
         # spin, angle, level, show, blink, alarm or active.\n\
         \n\
         # [[part]]\n\
         # id = \"root\"\n\
         # spin = \"running\"\n\
         \n\
         # [[part]]\n\
         # id = \"rotor\"\n\
         # spin = \"running\"\n\
         \n\
         # [[part]]\n\
         # id = \"blade\"\n\
         # spin = \"running\"\n"
    );
}

#[test]
fn a_file_that_is_no_xml_drawing_is_refused() {
    // Written anyway, the definition would point at a drawing every later check refuses.
    let error = starter_definition("Pump", "<svg><g></svg>").unwrap_err();
    assert!(error.starts_with("the drawing does not load: "), "{error}");
}

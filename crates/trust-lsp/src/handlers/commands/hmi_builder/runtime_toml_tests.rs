//! `runtime.toml` edits (specification 39, §12.8): appended at the end, removed with their lines,
//! inline lists left to the user.

use toml_edit::{value, Table};

use super::{appended, parse, tables, without, Entries};

fn rule(path: &str) -> Table {
    let mut table = Table::new();
    table.insert("path", value(path));
    table.insert("role", value("engineer"));
    table
}

#[test]
fn a_new_table_comes_at_the_end_after_a_blank_line_and_the_file_is_kept() {
    // Placed after `[runtime.web.hmi]` instead, the rule would land in the middle of the file,
    // between the user's own sections and comments.
    let text = "[runtime.web.hmi]\nidle_minutes = 5\n\n[runtime.log]\nlevel = \"info\" # keep\n";
    assert_eq!(
        appended(text, Entries::WriteRules, vec![rule("Main.Speed")]).unwrap(),
        format!("{text}\n[[runtime.web.hmi.write]]\npath = \"Main.Speed\"\nrole = \"engineer\"\n")
    );
}

#[test]
fn a_file_without_a_final_line_break_gets_one_before_the_new_table() {
    // Without it the header would continue the last line: `a = 1[[runtime.web.hmi.write]]`.
    assert_eq!(
        appended("a = 1", Entries::WriteRules, vec![rule("Main.Speed")]).unwrap(),
        "a = 1\n\n[[runtime.web.hmi.write]]\npath = \"Main.Speed\"\nrole = \"engineer\"\n"
    );
}

#[test]
fn nothing_to_add_leaves_the_text_as_it_is() {
    assert_eq!(
        appended("a = 1\n", Entries::Users, Vec::new()).unwrap(),
        "a = 1\n"
    );
}

#[test]
fn entries_listed_inline_are_left_to_the_user() {
    // A `[[runtime.web.hmi.write]]` table after `write = [...]` would define the key twice and the
    // runtime would not load the file.
    let inline = "[runtime.web.hmi]\nwrite = [{ path = \"Main.Level\", role = \"engineer\" }]\n";
    assert_eq!(
        appended(inline, Entries::WriteRules, vec![rule("Main.Speed")]),
        Err("runtime.toml lists its write rules inline under [runtime.web.hmi]; add the rule there by hand.".to_owned())
    );
    let inline_users = "[runtime.web.hmi]\nuser = []\n";
    assert_eq!(
        tables(&parse(inline_users).unwrap(), Entries::Users).map(|found| found.len()),
        Err("runtime.toml lists its users inline under [runtime.web.hmi]; add the user there by hand.".to_owned())
    );
    let inline_section = "[runtime.web]\nhmi = { idle_minutes = 5 }\n";
    assert_eq!(
        appended(
            inline_section,
            Entries::WriteRules,
            vec![rule("Main.Speed")]
        ),
        Err("runtime.toml writes [runtime.web.hmi] inline; add the rule there by hand.".to_owned())
    );
}

#[test]
fn a_file_that_does_not_load_is_left_as_it_is() {
    // Appending to a broken file would hide the user's own error behind a second change.
    let error = appended("[runtime\n", Entries::WriteRules, vec![rule("Main.Speed")]).unwrap_err();
    assert!(error.starts_with("runtime.toml does not load: "), "{error}");
}

#[test]
fn a_removed_table_leaves_with_its_lines_and_the_rest_stays() {
    // Removing only the key-values would leave an empty `[[runtime.web.hmi.user]]`, a user without
    // a name that the runtime refuses.
    let text = "a = 1\n\n[[runtime.web.hmi.user]]\nname = \"anna\"\nrole = \"operator\"\npassword = \"x\"\n\n[[runtime.web.hmi.user]]\nname = \"ben\"\nrole = \"engineer\"\npassword = \"y\"\n\n[runtime.log]\nlevel = \"info\"\n";
    let named =
        |name: &'static str| move |table: &Table| super::text_of(table, "name") == Some(name);
    assert_eq!(
        without(text, Entries::Users, named("anna")).unwrap(),
        "a = 1\n\n[[runtime.web.hmi.user]]\nname = \"ben\"\nrole = \"engineer\"\npassword = \"y\"\n\n[runtime.log]\nlevel = \"info\"\n"
    );
    assert_eq!(
        without(text, Entries::Users, named("carl")).unwrap(),
        text,
        "no such user"
    );
}

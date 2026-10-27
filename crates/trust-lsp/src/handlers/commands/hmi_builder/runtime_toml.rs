//! `runtime.toml` as the HMI Builder edits it (specification 39, §12.8): the runtime display's
//! users and write rules are `[[runtime.web.hmi.user]]` and `[[runtime.web.hmi.write]]` tables. A
//! new table is appended at the end of the file and a removed one leaves with its own lines, so the
//! rest of the file and its comments stay as they are. A file that lists these entries inline
//! (`write = [...]` under `[runtime.web.hmi]`, or `hmi = { ... }` under `[runtime.web]`) cannot take
//! a new table there and is left to the user.

use toml_edit::{ArrayOfTables, DocumentMut, Item, Table};

/// The file the runtime display's users and write rules live in.
pub(super) const FILE: &str = "runtime.toml";

/// One kind of entry of `[runtime.web.hmi]`: `user` or `write`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Entries {
    /// `[[runtime.web.hmi.user]]`.
    Users,
    /// `[[runtime.web.hmi.write]]`.
    WriteRules,
}

impl Entries {
    /// The key under `[runtime.web.hmi]`.
    fn key(self) -> &'static str {
        match self {
            Self::Users => "user",
            Self::WriteRules => "write",
        }
    }

    /// What the entries are, in the user's words.
    fn words(self) -> &'static str {
        match self {
            Self::Users => "users",
            Self::WriteRules => "write rules",
        }
    }

    /// One entry, in the user's words.
    fn one(self) -> &'static str {
        match self {
            Self::Users => "user",
            Self::WriteRules => "rule",
        }
    }
}

/// The document of `text`.
///
/// # Errors
///
/// Why the text is not TOML.
pub(super) fn parse(text: &str) -> Result<DocumentMut, String> {
    text.parse::<DocumentMut>()
        .map_err(|error| format!("{FILE} does not load: {}", error.message().trim()))
}

/// The entries' tables, in the file's order; none when the file has none.
///
/// # Errors
///
/// A file that lists them inline, where a new table would not load.
pub(super) fn tables(document: &DocumentMut, entries: Entries) -> Result<Vec<&Table>, String> {
    let Some(hmi) = hmi_item(document) else {
        return Ok(Vec::new());
    };
    let Some(table) = hmi.as_table() else {
        return Err(inline_hmi(entries));
    };
    match table.get(entries.key()) {
        None => Ok(Vec::new()),
        Some(Item::ArrayOfTables(array)) => Ok(array.iter().collect()),
        Some(_) => Err(inline(entries)),
    }
}

/// `text` with `new` appended as entries of the kind `entries`, each after a blank line, at the
/// end of the file.
///
/// # Errors
///
/// A file that does not load or lists the entries inline.
pub(super) fn appended(text: &str, entries: Entries, new: Vec<Table>) -> Result<String, String> {
    tables(&parse(text)?, entries)?;
    if new.is_empty() {
        return Ok(text.to_owned());
    }
    let mut array = ArrayOfTables::new();
    for mut table in new {
        table.decor_mut().set_prefix("\n");
        array.push(table);
    }
    let addition = nested(entries, Item::ArrayOfTables(array)).to_string();
    let separator = if text.is_empty() || text.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let next = format!("{text}{separator}{addition}");
    // What is written loads, with the new entries in it.
    parse(&next).map_err(|error| format!("The new entries would not load: {error}"))?;
    Ok(next)
}

/// `text` without the entry of the kind `entries` that `matches` picks first, with the lines it
/// was written on; unchanged when none matches.
///
/// # Errors
///
/// A file that does not load or lists the entries inline.
pub(super) fn without(
    text: &str,
    entries: Entries,
    matches: impl Fn(&Table) -> bool,
) -> Result<String, String> {
    let mut document = parse(text)?;
    let Some(index) = tables(&document, entries)?
        .iter()
        .position(|table| matches(table))
    else {
        return Ok(text.to_owned());
    };
    if let Some(Item::ArrayOfTables(array)) = hmi_item_mut(&mut document)
        .and_then(Item::as_table_mut)
        .and_then(|hmi| hmi.get_mut(entries.key()))
    {
        array.remove(index);
    }
    Ok(document.to_string())
}

/// A table's string value at `key`; none when it has none or another kind of value.
pub(super) fn text_of<'a>(table: &'a Table, key: &str) -> Option<&'a str> {
    table.get(key).and_then(Item::as_str)
}

/// `[runtime.web.hmi]`, as a table or an inline table.
fn hmi_item(document: &DocumentMut) -> Option<&Item> {
    document.get("runtime")?.get("web")?.get("hmi")
}

fn hmi_item_mut(document: &mut DocumentMut) -> Option<&mut Item> {
    document.get_mut("runtime")?.get_mut("web")?.get_mut("hmi")
}

/// A document holding only `item` at `runtime.web.hmi.<key>`, the parents implicit so that only
/// the entries' headers are written.
fn nested(entries: Entries, item: Item) -> DocumentMut {
    let mut hmi = Table::new();
    hmi.set_implicit(true);
    hmi.insert(entries.key(), item);
    let mut web = Table::new();
    web.set_implicit(true);
    web.insert("hmi", Item::Table(hmi));
    let mut runtime = Table::new();
    runtime.set_implicit(true);
    runtime.insert("web", Item::Table(web));
    let mut document = DocumentMut::new();
    document.insert("runtime", Item::Table(runtime));
    document
}

fn inline(entries: Entries) -> String {
    format!(
        "{FILE} lists its {} inline under [runtime.web.hmi]; add the {} there by hand.",
        entries.words(),
        entries.one()
    )
}

fn inline_hmi(entries: Entries) -> String {
    format!(
        "{FILE} writes [runtime.web.hmi] inline; add the {} there by hand.",
        entries.one()
    )
}

#[cfg(test)]
#[path = "runtime_toml_tests.rs"]
mod tests;

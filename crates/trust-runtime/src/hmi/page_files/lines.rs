//! The line a problem with a page object belongs to (specification 39, §7.6, §16.2): shared by the
//! language server's checks and the bundle checks.

/// The one-based line a problem belongs to: the bound property's line in the object's bind
/// table, its `attach` line for its attachment, the object's `type` line for a problem with the
/// object itself, or its `id` line. A
/// pipe, which has no id, is named `pipe:<n>` by its place among the pipes (§6).
pub fn problem_line(text: &str, object: &str, property: Option<&str>) -> usize {
    if let Some(index) = object
        .strip_prefix("pipe:")
        .and_then(|n| n.parse::<usize>().ok())
    {
        return pipe_line(text, index, property);
    }
    // An instrument's attachment and an equipment object's block, panel and instance are object
    // keys, not bindings (§6 item 5, §8.9 item 3, §21.1).
    if let Some(key @ ("attach" | "measures" | "acts_on" | "block" | "faceplate" | "instance")) =
        property
    {
        return object_key_line(text, object, key);
    }
    let lines: Vec<&str> = text.lines().collect();
    let Some(id) = lines
        .iter()
        .position(|line| key_and_string(line) == Some(("id", object)))
    else {
        return 1;
    };
    let end = lines[id + 1..]
        .iter()
        .position(|line| line.trim_start().starts_with("[["))
        .map_or(lines.len(), |offset| id + 1 + offset);
    let block = &lines[id..end];
    let found = match property {
        None => block.iter().position(|line| key(line) == Some("type")),
        Some(property) => block
            .iter()
            .position(|line| line.trim() == "[object.bind]")
            .and_then(|table| {
                block[table + 1..]
                    .iter()
                    .take_while(|line| !line.trim_start().starts_with('['))
                    .position(|line| key(line) == Some(property))
                    .map(|offset| table + 1 + offset)
            }),
    };
    id + found.unwrap_or(0) + 1
}

/// The line of a pipe's problem: its `from` or `to` line, the bound property's line in its bind
/// table, or its `[[pipe]]` line.
fn pipe_line(text: &str, index: usize, property: Option<&str>) -> usize {
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim() == "[[pipe]]")
        .nth(index)
        .map(|(at, _)| at)
    else {
        return 1;
    };
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.trim_start().starts_with("[["))
        .map_or(lines.len(), |offset| start + 1 + offset);
    let block = &lines[start..end];
    let found = match property.unwrap_or("from") {
        end_key @ ("from" | "to") => block.iter().position(|line| key(line) == Some(end_key)),
        bound => block
            .iter()
            .position(|line| line.trim() == "[pipe.bind]")
            .and_then(|table| {
                block[table + 1..]
                    .iter()
                    .take_while(|line| !line.trim_start().starts_with('['))
                    .position(|line| key(line) == Some(bound))
                    .map(|offset| table + 1 + offset)
            }),
    };
    start + found.unwrap_or(0) + 1
}

/// The one-based line of an object's own `key = value`, or its `id` line when it has none.
pub fn object_key_line(text: &str, object: &str, key_name: &str) -> usize {
    let lines: Vec<&str> = text.lines().collect();
    let Some(id) = lines
        .iter()
        .position(|line| key_and_string(line) == Some(("id", object)))
    else {
        return 1;
    };
    let found = lines[id..]
        .iter()
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .position(|line| key(line) == Some(key_name))
        .map_or(0, |offset| offset + 1);
    id + found + 1
}

/// The one-based line of an object's click target in its `[object.click]` table, or its `id`
/// line when it has none.
pub fn click_line(text: &str, object: &str) -> usize {
    let lines: Vec<&str> = text.lines().collect();
    let Some(id) = lines
        .iter()
        .position(|line| key_and_string(line) == Some(("id", object)))
    else {
        return 1;
    };
    let end = lines[id + 1..]
        .iter()
        .position(|line| line.trim_start().starts_with("[["))
        .map_or(lines.len(), |offset| id + 1 + offset);
    let found = lines[id..end]
        .iter()
        .position(|line| line.trim() == "[object.click]")
        .and_then(|table| {
            lines[id + table + 1..end]
                .iter()
                .position(|line| key(line).is_some())
                .map(|offset| table + 1 + offset)
        });
    id + found.unwrap_or(0) + 1
}

/// The key of a `key = value` line, without quotes.
fn key(line: &str) -> Option<&str> {
    let (key, _) = line.split_once('=')?;
    let key = key.trim();
    Some(
        key.strip_prefix('"')
            .and_then(|quoted| quoted.strip_suffix('"'))
            .unwrap_or(key),
    )
}

/// The key and the string value of a `key = "value"` line.
fn key_and_string(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    let value = value.trim().strip_prefix('"')?.strip_suffix('"')?;
    Some((key.trim(), value))
}

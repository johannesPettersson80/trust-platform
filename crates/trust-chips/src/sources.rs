//! The `[sources]` table: which values were read on a chip, and which manufacturer values the
//! repository's records do not yet confirm (specification 34, Appendix H.4 and rule 10 of H.5).

use crate::check::{filled, listed_once};
use crate::model::Sources;
use crate::Problem;

/// Rule 10: documents are named, `measured_on` goes with `measured`, and every path names a key
/// of the file outside `[sources]`, listed once.
pub(crate) fn check(sources: &Sources, table: &toml::Table) -> Result<(), Problem> {
    if sources.documents.is_empty() {
        return Err(Problem::NoDocuments);
    }
    listed_once("sources.documents", &sources.documents)?;
    if sources.measured.is_empty() == sources.measured_on.is_some() {
        return Err(Problem::MeasuredOn);
    }
    if let Some(origin) = &sources.measured_on {
        filled("sources.measured_on", origin)?;
    }
    let paths: Vec<&str> = sources
        .measured
        .iter()
        .chain(&sources.unchecked)
        .map(String::as_str)
        .collect();
    for (index, &path) in paths.iter().enumerate() {
        if !names_a_key(table, path) {
            return Err(Problem::UnknownSourcePath {
                path: path.to_owned(),
            });
        }
        if paths[..index].iter().any(|&earlier| nested(path, earlier)) {
            return Err(Problem::RepeatedSourcePath {
                path: path.to_owned(),
            });
        }
    }
    Ok(())
}

/// Whether `path` names a key of the file outside `[sources]`.
fn names_a_key(table: &toml::Table, path: &str) -> bool {
    let keys: Vec<&str> = path.split('.').collect();
    let Some((&first, rest)) = keys.split_first() else {
        return false;
    };
    first != "sources" && table.get(first).is_some_and(|value| reaches(value, rest))
}

/// Whether `keys` lead from `value` to a key. A path through an array of tables leads there
/// through every entry.
fn reaches(value: &toml::Value, keys: &[&str]) -> bool {
    match (value, keys.split_first()) {
        (_, None) => true,
        (toml::Value::Table(table), Some((&key, rest))) => {
            table.get(key).is_some_and(|inner| reaches(inner, rest))
        }
        (toml::Value::Array(entries), Some(_)) => {
            !entries.is_empty() && entries.iter().all(|entry| reaches(entry, keys))
        }
        _ => false,
    }
}

/// Whether two paths are one, or one lies inside the other.
fn nested(first: &str, second: &str) -> bool {
    let inside = |inner: &str, outer: &str| {
        inner
            .strip_prefix(outer)
            .is_some_and(|rest| rest.starts_with('.'))
    };
    first == second || inside(first, second) || inside(second, first)
}

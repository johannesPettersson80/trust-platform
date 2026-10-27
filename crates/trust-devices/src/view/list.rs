//! The list (specification 25 §5.14): the same items as the canvas in one table, computers,
//! runtimes, connections and devices, each in one row. The columns are fixed here; each item fills
//! the cells it has, and a cell it has no value for stays empty.

use std::collections::BTreeMap;

use super::types::{Column, ListCells, ListView};
use crate::texts::graph as texts;

/// The item's kind in words.
pub(crate) const KIND: &str = "kind";
/// Its name.
pub(crate) const NAME: &str = "name";
/// Its state.
pub(crate) const STATE: &str = "state";
/// Its address.
pub(crate) const ADDRESS: &str = "address";
/// Its platform, runtime kind, protocol or what it presents.
pub(crate) const TYPE: &str = "type";
/// The item that holds it: a runtime's computer, a connection's runtime.
pub(crate) const BELONGS_TO: &str = "belongs_to";
/// Its detail.
pub(crate) const DETAIL: &str = "detail";
/// A connection's live values.
pub(crate) const VALUES: &str = "values";
/// When it was last seen.
pub(crate) const LAST_SEEN: &str = "last_seen";
/// Its response time.
pub(crate) const RESPONSE_TIME: &str = "response_time";
/// Its identity: a computer's host name, a runtime's truST version and platform.
pub(crate) const IDENTITY: &str = "identity";

/// The columns, in their order.
pub(crate) fn list_view() -> ListView {
    let columns = [
        (KIND, texts::COLUMN_KIND),
        (NAME, texts::COLUMN_NAME),
        (STATE, texts::COLUMN_STATE),
        (ADDRESS, texts::COLUMN_ADDRESS),
        (TYPE, texts::COLUMN_TYPE),
        (BELONGS_TO, texts::COLUMN_BELONGS_TO),
        (DETAIL, texts::COLUMN_DETAIL),
        (VALUES, texts::COLUMN_VALUES),
        (LAST_SEEN, texts::COLUMN_LAST_SEEN),
        (RESPONSE_TIME, texts::COLUMN_RESPONSE_TIME),
        (IDENTITY, texts::COLUMN_IDENTITY),
    ];
    ListView {
        columns: columns
            .into_iter()
            .map(|(id, label)| Column {
                id: id.to_owned(),
                label: label.to_owned(),
            })
            .collect(),
    }
}

/// A row: the item's kind and the cells that have a value.
pub(crate) fn cells<'a>(
    kind: &str,
    values: impl IntoIterator<Item = (&'static str, Option<&'a str>)>,
) -> ListCells {
    ListCells {
        kind: kind.to_owned(),
        cells: values
            .into_iter()
            .filter_map(|(column, value)| {
                value
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| (column.to_owned(), value.to_owned()))
            })
            .collect::<BTreeMap<_, _>>(),
    }
}

/// What a computer's row shows.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ComputerCells<'a> {
    /// Whether it is a chip.
    pub(crate) chip: bool,
    /// Its heading.
    pub(crate) name: &'a str,
    /// `Reachable` or `Unreachable`.
    pub(crate) state: &'a str,
    /// Its address.
    pub(crate) address: Option<&'a str>,
    /// Its platform.
    pub(crate) platform: Option<&'a str>,
    /// Its network's name.
    pub(crate) network: Option<&'a str>,
    /// When it was last seen.
    pub(crate) last_seen: Option<&'a str>,
    /// Its host name.
    pub(crate) host_name: Option<&'a str>,
}

/// A computer's row.
pub(crate) fn computer_cells(computer: &ComputerCells<'_>) -> ListCells {
    let (kind, word) = if computer.chip {
        ("chip", texts::KIND_CHIP)
    } else {
        ("computer", texts::KIND_COMPUTER)
    };
    cells(
        kind,
        [
            (KIND, Some(word)),
            (NAME, Some(computer.name)),
            (STATE, Some(computer.state)),
            (ADDRESS, computer.address),
            (TYPE, computer.platform),
            (BELONGS_TO, computer.network),
            (LAST_SEEN, computer.last_seen),
            (IDENTITY, computer.host_name),
        ],
    )
}

/// What a runtime's row shows.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeCells<'a> {
    /// Its name.
    pub(crate) name: &'a str,
    /// Its state word.
    pub(crate) state: &'a str,
    /// Its control address.
    pub(crate) address: Option<&'a str>,
    /// Its kind: the simulator, a runtime on a computer or on a chip.
    pub(crate) kind: &'a str,
    /// The heading of its computer.
    pub(crate) computer: &'a str,
    /// Its detail.
    pub(crate) detail: &'a str,
    /// Its truST version.
    pub(crate) version: Option<&'a str>,
    /// Its platform.
    pub(crate) platform: Option<&'a str>,
}

/// A runtime's row.
pub(crate) fn runtime_cells(runtime: &RuntimeCells<'_>) -> ListCells {
    let identity = texts::joined(&[
        runtime.version.unwrap_or_default(),
        runtime.platform.unwrap_or_default(),
    ]);
    cells(
        "runtime",
        [
            (KIND, Some(texts::KIND_RUNTIME)),
            (NAME, Some(runtime.name)),
            (STATE, Some(runtime.state)),
            (ADDRESS, runtime.address),
            (TYPE, Some(runtime.kind)),
            (BELONGS_TO, Some(runtime.computer)),
            (DETAIL, Some(runtime.detail)),
            (IDENTITY, Some(identity.as_str())),
        ],
    )
}

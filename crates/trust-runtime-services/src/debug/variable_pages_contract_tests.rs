//! The variable pages of `debug.variables`, through the shared dispatcher and the handle table of a
//! chip (specification 11 §6.13.3, "Variable pages" and "Handle bound"; specification 13,
//! "Variables / Evaluate"). A debugger over a storage stands in for a device's loop: the dispatcher
//! reads it exactly as it reads the loop, and its table holds the 256 handles of a chip.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use serde_json::{json, Value as Json};
use smol_str::SmolStr;
use trust_runtime_core::bytecode::LineTable;
use trust_runtime_core::collections::OrderedMap;
use trust_runtime_core::debug::StatementSpan;
use trust_runtime_core::memory::{FrameId, VariableStorage};
use trust_runtime_core::value::{ArrayValue, StructValue, Value};

use super::projection::{DebugVariableHandles, CHIP_HANDLE_BOUND};
use super::reads::{NamedFrame, ReadState};
use super::requests::{answer, BreakpointView, Debugger, StopRecord};
use super::run::StepKind;
use super::sources::LineSources;
use super::statement::Span;
use crate::live_values::io_requests::IoSnapshotRead;
use crate::protocol::{ControlRequest, ControlResponse};

/// A debugger over a storage with the handle table of a chip. Its one frame, number 1, has no
/// locals.
struct StorageDebugger {
    sources: LineSources,
    storage: VariableStorage,
    handles: RefCell<DebugVariableHandles>,
}

impl StorageDebugger {
    fn chip(storage: VariableStorage) -> Self {
        Self {
            sources: LineSources::new(LineTable::default(), Vec::new()),
            storage,
            handles: RefCell::new(DebugVariableHandles::with_bound(CHIP_HANDLE_BOUND)),
        }
    }

    /// How many variable handles are live.
    fn live_handles(&self) -> usize {
        self.handles.borrow().len()
    }
}

impl Debugger for StorageDebugger {
    fn sources(&self) -> Option<&LineSources> {
        Some(&self.sources)
    }

    fn read_state(&self) -> Result<ReadState<'_>, String> {
        Ok(ReadState {
            storage: &self.storage,
            frames: vec![NamedFrame {
                id: FrameId(1),
                owner: "Main",
                instance: None,
                variables: Vec::new(),
            }],
        })
    }

    fn io_snapshot(&self) -> IoSnapshotRead {
        IoSnapshotRead::Missing
    }

    fn is_held(&self) -> bool {
        true
    }

    fn accepts_step(&self) -> bool {
        false
    }

    fn step(&self, _kind: StepKind) {}

    fn last_stop(&self) -> Option<StopRecord> {
        None
    }

    fn drain_stops(&self) -> Vec<StopRecord> {
        Vec::new()
    }

    fn frame_span(&self, _frame: FrameId) -> Option<StatementSpan> {
        None
    }

    fn breakpoints(&self) -> Vec<BreakpointView> {
        Vec::new()
    }

    fn replace_breakpoints(
        &self,
        _file_id: u32,
        _breakpoints: Vec<(Span<(u32, u32)>, crate::debug::BreakpointOptions)>,
    ) -> u64 {
        0
    }

    fn clear_breakpoints(&self) {}

    fn with_handles(&self, operation: &mut dyn FnMut(&mut DebugVariableHandles)) {
        operation(&mut self.handles.borrow_mut());
    }
}

/// A storage whose globals are `globals`, in that order.
fn storage_with(globals: Vec<(&str, Value)>) -> VariableStorage {
    let mut storage = VariableStorage::new();
    for (name, value) in globals {
        storage.set_global(name, value);
    }
    storage
}

/// A structure value of type `type_name` with the members `members`, in that order.
fn structure(type_name: &str, members: &[(&str, Value)]) -> Value {
    let mut fields = OrderedMap::default();
    for (name, value) in members {
        fields.insert(SmolStr::new(name), value.clone());
    }
    Value::Struct(Arc::new(StructValue::from_untyped_parts(
        SmolStr::new(type_name),
        fields,
    )))
}

/// An array of `len` DINT elements, the element at index `i` holding `first + i`.
fn dint_array(first: i32, len: i32) -> Value {
    let elements = (first..first + len).map(Value::DInt).collect();
    let dimensions = vec![(0, i64::from(len) - 1)];
    Value::Array(Box::new(
        ArrayValue::from_untyped_parts(elements, dimensions).expect("an array"),
    ))
}

/// Answers the operation `kind` with `params` through the shared dispatcher.
fn ask(debugger: &StorageDebugger, kind: &str, params: Json) -> ControlResponse {
    let mut message = json!({"id": 1, "type": kind});
    message["params"] = params;
    let request: ControlRequest = serde_json::from_value(message).expect("a control request");
    answer(&request, debugger)
}

/// The scopes of frame 1, which hand out the handles Locals 1 and Globals 2.
fn open_scopes(debugger: &StorageDebugger) {
    let scopes = ask(debugger, "debug.scopes", json!({"frame_id": 1}));
    assert!(scopes.ok, "{:?}", scopes.error);
}

/// The variables that `debug.variables` lists for `params`.
fn variables_of(debugger: &StorageDebugger, params: Json) -> Json {
    let response = ask(debugger, "debug.variables", params);
    assert!(response.ok, "{:?}", response.error);
    response.result.expect("a result")["variables"].clone()
}

/// The `(name, value)` of every listed variable.
fn names_and_values(variables: &Json) -> Vec<(&str, &str)> {
    variables
        .as_array()
        .expect("a list")
        .iter()
        .map(|variable| {
            (
                variable["name"].as_str().expect("a name"),
                variable["value"].as_str().expect("a value"),
            )
        })
        .collect()
}

/// How many variables are listed, the first name and the last name.
fn shape(variables: &Json) -> (usize, &str, &str) {
    let names = names_and_values(variables);
    let first = names.first().map_or("", |entry| entry.0);
    let last = names.last().map_or("", |entry| entry.0);
    (names.len(), first, last)
}

/// The handle of every listed variable.
fn references(variables: &Json) -> Vec<u64> {
    variables
        .as_array()
        .expect("a list")
        .iter()
        .map(|variable| variable["variablesReference"].as_u64().expect("a handle"))
        .collect()
}

/// The entry of element `index` of an array of `Point` structures, holding handle `reference`.
fn point_entry(index: u32, reference: u32) -> Json {
    json!({
        "name": format!("[{index}]"),
        "value": "Point {...}",
        "type": "Point",
        "variablesReference": reference,
        "evaluateName": format!("[{index}]"),
        "namedVariables": 2
    })
}

/// An array of 300 `Point` structures, `x` holding the index and `y` twice the index.
fn points() -> Value {
    let elements = (0..300)
        .map(|index| {
            structure(
                "Point",
                &[("x", Value::DInt(index)), ("y", Value::DInt(2 * index))],
            )
        })
        .collect();
    Value::Array(Box::new(
        ArrayValue::from_untyped_parts(elements, vec![(0, 299)]).expect("an array"),
    ))
}

#[test]
fn an_array_of_300_structures_lists_page_by_page_on_a_chip_within_the_handle_bound() {
    // Specification 11 §6.13.3, "Variable pages" and "Handle bound": a chip holds at most 256 live
    // handles, and a page takes a handle for each of its structures only. Worked: the scopes take
    // Locals 1 and Globals 2, the Globals listing the array's handle 3, and the three pages of 100
    // elements 4 to 103, 104 to 203 and 204 to 303. After two pages 203 handles are live; the third
    // page needs 100 and only 53 are free, so the table releases the oldest page (4 to 103) and 203
    // are live again, never more than 256. The released handle 4 answers `unknown variables
    // reference 4`; the handles of the later pages are live and list `x` and `y` (element 100 holds
    // 100 and 200). A start at 300 lists none.
    // Wrong behaviour caught: the whole array listed in one request, which needs 300 handles and is
    // refused; a table that keeps every page, which refuses the third; a release of the newest page
    // instead of the oldest (handle 104 would be gone and 4 live); a release that leaves the
    // released handles counted (the table would stay full); and a page that numbers its elements
    // from 0 instead of their index in the array.
    let debugger = StorageDebugger::chip(storage_with(vec![("points", points())]));
    open_scopes(&debugger);
    let globals = variables_of(&debugger, json!({"variables_reference": 2}));
    assert_eq!(
        globals,
        json!([{
            "name": "points",
            "value": "[300]",
            "type": "ARRAY",
            "variablesReference": 3,
            "evaluateName": "points",
            "indexedVariables": 300
        }])
    );
    let page = |start: u32| {
        variables_of(
            &debugger,
            json!({"variables_reference": 3, "filter": "indexed", "start": start, "count": 100}),
        )
    };

    let first = page(0);
    assert_eq!(first.as_array().map(Vec::len), Some(100));
    assert_eq!(first[0], point_entry(0, 4));
    assert_eq!(first[99], point_entry(99, 103));
    assert_eq!(references(&first), (4..=103).collect::<Vec<u64>>());
    assert_eq!(debugger.live_handles(), 103);

    let second = page(100);
    assert_eq!(second[0], point_entry(100, 104));
    assert_eq!(second[99], point_entry(199, 203));
    assert_eq!(debugger.live_handles(), 203);

    let third = page(200);
    assert_eq!(third.as_array().map(Vec::len), Some(100));
    assert_eq!(third[0], point_entry(200, 204));
    assert_eq!(third[99], point_entry(299, 303));
    assert_eq!(debugger.live_handles(), 203, "the oldest page was released");
    assert!(debugger.live_handles() <= CHIP_HANDLE_BOUND);

    let released = ask(
        &debugger,
        "debug.variables",
        json!({"variables_reference": 4}),
    );
    assert!(!released.ok);
    assert_eq!(
        released.error.as_deref(),
        Some("unknown variables reference 4")
    );
    let members =
        |reference: u32| variables_of(&debugger, json!({"variables_reference": reference}));
    assert_eq!(
        members(104),
        json!([
            {"name": "x", "value": "100", "type": "DINT", "variablesReference": 0, "evaluateName": "x"},
            {"name": "y", "value": "200", "type": "DINT", "variablesReference": 0, "evaluateName": "y"}
        ])
    );
    assert_eq!(
        names_and_values(&members(303)),
        [("x", "299"), ("y", "598")]
    );
    assert_eq!(page(300), json!([]));
}

#[test]
fn a_page_that_cannot_fit_even_after_releasing_every_page_is_refused_whole() {
    // Specification 11 §6.13.3, "Handle bound": a page that the table could not hold even with
    // every earlier page released is refused whole with `variable_handles_full`, releases nothing
    // and leaves the table as it was. Worked: after the first page 103 handles are live (1 to 3 and
    // 4 to 103). A page of all 300 elements needs 300, and releasing the 100 of the first page
    // leaves room for only 253, so it is refused; the first page stays live (handle 4 lists
    // `x` 0 and `y` 0), and the next page of 100 takes 104 to 203, the numbers the refused listing
    // started from.
    // Wrong behaviour caught: a refusal that has already released the first page, a refused
    // listing that keeps the handles it took (the next page would not start at 104), and a page
    // larger than the bound that is cut to what fits instead of refused.
    let debugger = StorageDebugger::chip(storage_with(vec![("points", points())]));
    open_scopes(&debugger);
    let _ = variables_of(&debugger, json!({"variables_reference": 2}));
    let first = variables_of(
        &debugger,
        json!({"variables_reference": 3, "filter": "indexed", "start": 0, "count": 100}),
    );
    assert_eq!(references(&first), (4..=103).collect::<Vec<u64>>());

    let whole = ask(
        &debugger,
        "debug.variables",
        json!({"variables_reference": 3, "filter": "indexed", "start": 0, "count": 300}),
    );

    assert!(!whole.ok);
    assert_eq!(whole.error_code.as_deref(), Some("variable_handles_full"));
    assert_eq!(
        whole.error.as_deref(),
        Some("the variable handle table is full: request the scopes again")
    );
    assert_eq!(debugger.live_handles(), 103);
    let kept = variables_of(&debugger, json!({"variables_reference": 4}));
    assert_eq!(names_and_values(&kept), [("x", "0"), ("y", "0")]);
    let next = variables_of(
        &debugger,
        json!({"variables_reference": 3, "filter": "indexed", "start": 100, "count": 100}),
    );
    assert_eq!(references(&next), (104..=203).collect::<Vec<u64>>());
}

#[test]
fn a_page_lists_from_start_at_most_count_elements() {
    // Specification 11 §6.13.3, "Variable pages": a page lists the children from `start`, in order,
    // at most `count`, and fewer at the end of the list; a start at or past the last child lists
    // none; `filter` keeps one kind of child, and the children of an array are indexed and every
    // other child named; a name keeps its index in the whole list. Worked from an array of ten
    // elements holding 100 to 109 (handle 3) and a structure with the members a to e holding 1 to 5
    // (handle 4): `start` 3 and `count` 4 list `[3]` to `[6]`; `start` 8 and `count` 5 list `[8]`
    // and `[9]` only; `start` 10, and `start` 25, list none; `count` 3 alone lists `[0]` to `[2]`;
    // the filter `named` lists none of an array and `indexed` none of a structure; `start` 1 and
    // `count` 2 of the structure list b and c; `named` with `start` 4 lists e; a `count` of 0 lists
    // all five members.
    // Wrong behaviour caught: a start that is off by one in either direction, a count that is
    // ignored, an element named by its place in the page, a start past the end that fails instead of
    // listing none, a filter that is ignored, and a count of 0 read as "list none".
    let motor = structure(
        "Motor",
        &[
            ("a", Value::DInt(1)),
            ("b", Value::DInt(2)),
            ("c", Value::DInt(3)),
            ("d", Value::DInt(4)),
            ("e", Value::DInt(5)),
        ],
    );
    let debugger = StorageDebugger::chip(storage_with(vec![
        ("values", dint_array(100, 10)),
        ("motor", motor),
    ]));
    open_scopes(&debugger);
    let globals = variables_of(&debugger, json!({"variables_reference": 2}));
    assert_eq!(references(&globals), [3, 4]);
    let listed = |params: Json| variables_of(&debugger, params);

    assert_eq!(
        names_and_values(&listed(
            json!({"variables_reference": 3, "filter": "indexed", "start": 3, "count": 4})
        )),
        [
            ("[3]", "103"),
            ("[4]", "104"),
            ("[5]", "105"),
            ("[6]", "106")
        ]
    );
    assert_eq!(
        names_and_values(&listed(
            json!({"variables_reference": 3, "start": 8, "count": 5})
        )),
        [("[8]", "108"), ("[9]", "109")]
    );
    assert_eq!(
        listed(json!({"variables_reference": 3, "start": 10})),
        json!([])
    );
    assert_eq!(
        listed(json!({"variables_reference": 3, "start": 25, "count": 2})),
        json!([])
    );
    assert_eq!(
        names_and_values(&listed(json!({"variables_reference": 3, "count": 3}))),
        [("[0]", "100"), ("[1]", "101"), ("[2]", "102")]
    );
    assert_eq!(
        listed(json!({"variables_reference": 3, "filter": "named"})),
        json!([])
    );
    assert_eq!(
        names_and_values(&listed(
            json!({"variables_reference": 3, "filter": "indexed", "start": 9})
        )),
        [("[9]", "109")]
    );
    assert_eq!(
        names_and_values(&listed(
            json!({"variables_reference": 4, "start": 1, "count": 2})
        )),
        [("b", "2"), ("c", "3")]
    );
    assert_eq!(
        listed(json!({"variables_reference": 4, "filter": "indexed"})),
        json!([])
    );
    assert_eq!(
        names_and_values(&listed(
            json!({"variables_reference": 4, "filter": "named", "start": 4})
        )),
        [("e", "5")]
    );
    assert_eq!(
        names_and_values(&listed(json!({"variables_reference": 4, "count": 0}))),
        [("a", "1"), ("b", "2"), ("c", "3"), ("d", "4"), ("e", "5")]
    );
}

#[test]
fn a_container_reports_its_indexed_and_named_counts() {
    // Specification 11 §6.13.3, "Variable pages": an entry whose value is an array reports
    // `indexedVariables`, the number of its elements, and one whose value is a structure reports
    // `namedVariables`, the number of its members; both are the size of the whole container, not of
    // the page; no other entry reports a count. Worked: the globals `speed` (DINT 7, a scalar),
    // `table` (an array of 250 elements, handle 3) and `motor` (a structure of three members,
    // handle 4). The entry of `table` says 250 although a page lists 100; the entries of the
    // elements and of the members, which are scalars, report neither count.
    // Wrong behaviour caught: counts missing, the two swapped, the page's length reported in place
    // of the container's, and a count (even 0) on a scalar entry.
    let motor = structure(
        "Motor",
        &[
            ("enabled", Value::Bool(true)),
            ("speed", Value::DInt(7)),
            ("mode", Value::Int(2)),
        ],
    );
    let debugger = StorageDebugger::chip(storage_with(vec![
        ("speed", Value::DInt(7)),
        ("table", dint_array(0, 250)),
        ("motor", motor),
    ]));
    open_scopes(&debugger);

    let globals = variables_of(&debugger, json!({"variables_reference": 2}));

    assert_eq!(
        globals,
        json!([
            {"name": "speed", "value": "7", "type": "DINT", "variablesReference": 0, "evaluateName": "speed"},
            {"name": "table", "value": "[250]", "type": "ARRAY", "variablesReference": 3, "evaluateName": "table", "indexedVariables": 250},
            {"name": "motor", "value": "Motor {...}", "type": "Motor", "variablesReference": 4, "evaluateName": "motor", "namedVariables": 3}
        ])
    );
    assert_eq!(
        variables_of(
            &debugger,
            json!({"variables_reference": 3, "filter": "indexed", "start": 249, "count": 1})
        ),
        json!([{"name": "[249]", "value": "249", "type": "DINT", "variablesReference": 0, "evaluateName": "[249]"}])
    );
    assert_eq!(
        variables_of(&debugger, json!({"variables_reference": 4})),
        json!([
            {"name": "enabled", "value": "TRUE", "type": "BOOL", "variablesReference": 0, "evaluateName": "enabled"},
            {"name": "speed", "value": "7", "type": "DINT", "variablesReference": 0, "evaluateName": "speed"},
            {"name": "mode", "value": "2", "type": "INT", "variablesReference": 0, "evaluateName": "mode"}
        ])
    );
}

#[test]
fn a_request_without_paging_answers_the_first_page() {
    // Specification 11 §6.13.3, "Variable pages": a `count` that is absent or 0 lists a page of 100
    // elements of an array, so a request with neither `start` nor `count` answers the first page,
    // and a `start` alone lists a page from there; the entry of the array still reports its full
    // size. The named children are all listed from `start` without a `count`. Worked: the arrays of
    // 100 (handle 3), 101 (4) and 250 (5) elements and a structure of 150 members (6): the array of
    // exactly 100 lists all 100, `[0]` to `[99]`; the array of 101 lists `[0]` to `[99]`; the array
    // of 250 lists `[0]` to `[99]` whether it names no count or a count of 0, `[100]` to `[199]` from
    // `start` 100 and the last 50, `[200]` to `[249]`, from `start` 200, and its entry says 250; the
    // structure lists all 150 members, `m0` to `m149`, and from `start` 100 its last 50.
    // Wrong behaviour caught: an array of 101 elements listed whole (the page cut one element
    // late), a first page of 99 or 101, an old client's request refused or answered with every
    // element, a `start` alone that restarts at the first page, and a structure whose members are cut
    // at a page.
    let owned = (0..150)
        .map(|index| (format!("m{index}"), Value::DInt(index)))
        .collect::<Vec<_>>();
    let members = owned
        .iter()
        .map(|(name, value)| (name.as_str(), value.clone()))
        .collect::<Vec<_>>();
    let debugger = StorageDebugger::chip(storage_with(vec![
        ("hundred", dint_array(0, 100)),
        ("hundred_one", dint_array(0, 101)),
        ("many", dint_array(0, 250)),
        ("wide", structure("Wide", &members)),
    ]));
    open_scopes(&debugger);
    let globals = variables_of(&debugger, json!({"variables_reference": 2}));
    assert_eq!(references(&globals), [3, 4, 5, 6]);
    assert_eq!(globals[2]["indexedVariables"], json!(250));
    assert_eq!(globals[3]["namedVariables"], json!(150));
    let listed = |params: Json| variables_of(&debugger, params);

    assert_eq!(
        shape(&listed(json!({"variables_reference": 3}))),
        (100, "[0]", "[99]")
    );
    assert_eq!(
        shape(&listed(json!({"variables_reference": 4}))),
        (100, "[0]", "[99]")
    );
    assert_eq!(
        shape(&listed(json!({"variables_reference": 5}))),
        (100, "[0]", "[99]")
    );
    assert_eq!(
        shape(&listed(json!({"variables_reference": 5, "count": 0}))),
        (100, "[0]", "[99]")
    );
    assert_eq!(
        shape(&listed(json!({"variables_reference": 5, "start": 100}))),
        (100, "[100]", "[199]")
    );
    assert_eq!(
        shape(&listed(json!({"variables_reference": 5, "start": 200}))),
        (50, "[200]", "[249]")
    );
    assert_eq!(
        shape(&listed(json!({"variables_reference": 6}))),
        (150, "m0", "m149")
    );
    assert_eq!(
        shape(&listed(json!({"variables_reference": 6, "start": 100}))),
        (50, "m100", "m149")
    );
}

#[test]
fn a_filter_other_than_indexed_or_named_is_refused_and_changes_nothing() {
    // Specification 11 §6.13.3, "Variable pages": any other value of `filter`, a negative `start`
    // and a `count` that is not a number are refused with an `invalid params` error, and the
    // request changes nothing. Worked: after the scopes (handles 1 and 2) each bad request is
    // refused with the error `invalid params: ...`, and the globals still list with the same
    // handle 3 for the array as the first listing gave.
    // Wrong behaviour caught: an unknown filter read as "no filter", a negative start wrapped around
    // to a huge one, and a refused request that still takes handles.
    let debugger = StorageDebugger::chip(storage_with(vec![("values", dint_array(0, 3))]));
    open_scopes(&debugger);
    for params in [
        json!({"variables_reference": 2, "filter": "sideways"}),
        json!({"variables_reference": 2, "filter": "Indexed"}),
        json!({"variables_reference": 2, "start": -1}),
        json!({"variables_reference": 2, "count": "ten"}),
    ] {
        let refused = ask(&debugger, "debug.variables", params.clone());
        assert!(!refused.ok, "{params}");
        assert!(
            refused
                .error
                .as_deref()
                .is_some_and(|error| error.starts_with("invalid params: ")),
            "{params}: {:?}",
            refused.error
        );
    }

    let globals = variables_of(&debugger, json!({"variables_reference": 2}));

    assert_eq!(references(&globals), [3]);
    assert_eq!(debugger.live_handles(), 3);
}

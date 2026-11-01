//! A trend's visible window as CSV (specification 39, §5.9 item 1), written by the runtime from its
//! trend history. The worked example is the display's own: the page's clock at 10,000,000 ms, a
//! window of the last 60 s, `Main.T` sampled 90, 30 and 10 s before with 1, 2 and 3, and the lamp
//! 20 s before with 1.

use super::{csv_field, export_csv, export_name, utc_time};

const NOW: u64 = 10_000_000;

fn pens() -> Vec<(String, Vec<(u64, f64)>)> {
    vec![
        ("on".to_owned(), vec![(NOW - 20_000, 1.0)]),
        (
            "pv".to_owned(),
            vec![
                (NOW - 90_000, 1.0),
                (NOW - 30_000, 2.0),
                (NOW - 10_000, 3.0),
            ],
        ),
    ]
}

/// A row for each time a pen has a sample in the window, each pen holding its last value then and
/// empty before its first; the sample 90 s before is outside the window and gives no row.
/// Worked by hand: 9,970,000 ms is 2 h 46 min 10 s after 1970. Catches a pen that drops its value
/// between its samples, a row outside the window, and a number written with a fraction it has
/// not (`2.0`).
#[test]
fn the_window_has_a_row_for_each_sample_time_with_every_pen_held() {
    assert_eq!(
        export_csv(&pens(), NOW - 60_000, NOW),
        "time,on,pv\n\
         1970-01-01T02:46:10.000Z,,2\n\
         1970-01-01T02:46:20.000Z,1,2\n\
         1970-01-01T02:46:30.000Z,1,3\n"
    );
}

/// A pen holds a value from before the window: its last sample before the first row. Catches a
/// window whose first rows are empty for a pen that has a value.
#[test]
fn a_pen_holds_its_last_value_from_before_the_window() {
    let pens = vec![
        ("level".to_owned(), vec![(NOW - 120_000, 7.5)]),
        (
            "flow".to_owned(),
            vec![(NOW - 30_000, 4.0), (NOW - 5_000, 4.25)],
        ),
    ];
    assert_eq!(
        export_csv(&pens, NOW - 60_000, NOW),
        "time,level,flow\n\
         1970-01-01T02:46:10.000Z,7.5,4\n\
         1970-01-01T02:46:35.000Z,7.5,4.25\n"
    );
}

/// A window without samples is the header alone; a pen's name that holds a comma or a quote is
/// quoted, its quotes doubled (RFC 4180). Catches a header that splits a name into two columns.
#[test]
fn an_empty_window_is_its_header_and_a_name_is_quoted_when_it_must_be() {
    assert_eq!(export_csv(&pens(), NOW, NOW), "time,on,pv\n");
    assert_eq!(csv_field("pv"), "pv");
    assert_eq!(csv_field("flow, total"), "\"flow, total\"");
    assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
}

/// The file name is the trend's id and the moment of the export in UTC, and a time is ISO 8601
/// with milliseconds as JavaScript writes it. Worked by hand: 10,000 s after 1970 is 02:46:40;
/// 10^12 ms is 2001-09-09 01:46:40 UTC. Catches a local time zone and a name without seconds.
#[test]
fn the_file_name_and_the_times_are_utc() {
    assert_eq!(export_name("loop", NOW), "loop-19700101-024640.csv");
    assert_eq!(utc_time(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(utc_time(1_000_000_000_123), "2001-09-09T01:46:40.123Z");
}

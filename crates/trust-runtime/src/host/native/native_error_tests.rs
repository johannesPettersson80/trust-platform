use super::{NativeError, SourcePlace};

/// A construct not compiled yet at `line` and `column` of `src/main.st`.
fn at(line: u32, column: u32) -> NativeError {
    NativeError::Unsupported {
        construct: format!("the construct of line {line}"),
        at: Some(SourcePlace {
            file: "src/main.st".into(),
            line,
            column,
        }),
    }
}

/// The line and column of each construct of `error`, `None` for one without a place.
fn places(error: &NativeError) -> Vec<Option<(u32, u32)>> {
    let each = match error {
        NativeError::AllUnsupported(each) => each.iter().collect(),
        other => vec![other],
    };
    each.into_iter()
        .map(|refusal| match refusal {
            NativeError::Unsupported {
                at: Some(place), ..
            } => Some((place.line, place.column)),
            _ => None,
        })
        .collect()
}

// Specification 11 §6.8.1c: one build's constructs that it does not compile yet come in the order
// of their places, by line and then by column, a construct of the declarations without a place
// after them, and a group that was already gathered is taken apart into its constructs. Worked by
// hand: met as (20, 10), none, (8, 6), and the group of (18, 6) and (18, 2), they are (8, 6),
// (18, 2), (18, 6), (20, 10), none. Catches the order in which the build met them, a construct
// without a place first, and a group kept as one entry.
#[test]
fn several_refusals_are_ordered_by_their_places_and_one_without_a_place_comes_last() {
    let group = NativeError::AllUnsupported(vec![at(18, 6), at(18, 2)]);
    let refused = vec![
        at(20, 10),
        NativeError::unsupported("an FB_init argument".into()),
        at(8, 6),
        group,
    ];
    let error = NativeError::refusals(refused).expect("the constructs");
    assert_eq!(
        places(&error),
        vec![
            Some((8, 6)),
            Some((18, 2)),
            Some((18, 6)),
            Some((20, 10)),
            None
        ]
    );
}

// Specification 11 §6.8.1c: a build that met one construct reports that construct as it is, the
// error of a build before the build went on after one, and a build that met none fails on no
// construct. Catches one construct wrapped in a group, whose text and issue would change, and an
// empty group that would fail a build with nothing to report.
#[test]
fn one_refusal_stays_itself_and_none_is_no_error() {
    let one = NativeError::refusals(vec![at(6, 6)]).expect("the construct");
    assert!(matches!(one, NativeError::Unsupported { .. }), "{one:?}");
    assert_eq!(
        one.to_string(),
        "the construct of line 6 is not compiled yet (src/main.st:6; specification 44 NX-09)"
    );
    assert!(NativeError::refusals(Vec::new()).is_none());
}

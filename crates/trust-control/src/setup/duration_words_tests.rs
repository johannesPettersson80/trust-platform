//! Tests of the words of a duration (specification 35, B9.9, "Durations"): the largest fitting unit, and the reading back of the words.

use super::duration_words;
use crate::setup::loop_bindings::duration_seconds_of_text;

#[test]
fn a_duration_is_written_in_its_largest_fitting_unit() {
    // B9.9: seconds below 2 min, minutes below 2 h, then hours and minutes; a remainder goes in the next smaller unit. The expected words
    // are worked out by hand: 7199 s is 119 min and 59 s, 7265 s is 2 h, 1 min and 5 s.
    for (seconds, words) in [
        (0.0005, "0.0005 s"),
        (0.1, "0.1 s"),
        (10.0, "10 s"),
        (90.0, "90 s"),
        (119.0, "119 s"),
        (120.0, "2 min"),
        (125.0, "2 min 5 s"),
        (900.0, "15 min"),
        (4_500.0, "75 min"),
        (5_400.0, "90 min"),
        (7_199.0, "119 min 59 s"),
        (7_200.0, "2 h"),
        (7_265.0, "2 h 1 min 5 s"),
        (9_000.0, "2 h 30 min"),
        (86_400.0, "24 h"),
    ] {
        assert_eq!(duration_words(seconds), words, "{seconds} s");
    }
}

#[test]
fn duration_words_read_back_as_the_same_duration() {
    // B9.9: the page shows these words, the engineer may leave them as they are, and the engine reads them again as the same duration. The
    // page formatted a duration itself before, so its formatter and this reader had to agree; now one writes and the same reads. A
    // remainder with decimals (130.3 s is 2 min and 10.3 s) must add back to exactly the same number, and a tiny number must be written in
    // plain decimals, never with an exponent, which the reader refuses.
    for seconds in [
        1e-10, 0.0005, 0.1, 1.0, 47.4, 89.999, 119.9, 120.0, 130.3, 600.0, 3_599.5, 7_200.0,
        7_265.25, 12_345.678, 86_400.0,
    ] {
        let words = duration_words(seconds);
        assert_eq!(
            duration_seconds_of_text(&words),
            Some(seconds),
            "{seconds} s was written as {words:?}"
        );
        assert!(
            !words.contains('e'),
            "no exponent in {words:?} (a unit is min, h or s)"
        );
    }
}

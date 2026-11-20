//! What a change of `PT` does to `TP`, `TON` and `TOF`, each value worked out by hand from
//! specification 08 §5 ("Timer Scan-Step Contract" and each timer's rules) and its worked
//! examples: a change applies from the call that samples it, a call takes `ET` no further than its
//! own `PT`, and once the timing has ended `TP` and `TOF` hold `ET` at the `PT` that ended it.
//! IEC 61131-3 leaves the effect of a change to the implementer (Table 46 NOTE).

use trust_plc_abi::Plain;

use crate::*;

const MS: i64 = 1_000_000;

/// Specification 08 §5, TON's worked example on a 10 ms task: `Q` rises at 30 ms with `PT`
/// T#30ms; `PT` T#100ms from the call at 50 ms returns `Q` FALSE and `ET` T#40ms, as `ET` goes on
/// from the 30 ms it stood at, and `Q` rises again at 110 ms; a smaller `PT` returns `Q` TRUE with
/// `ET` at it, also while timing. Catches a TON that keeps `Q` TRUE once it rose (`Q` TRUE at
/// 50 ms), one that times from the edge again (`Q` TRUE at 100 ms), and an `ET` past `PT`.
#[test]
fn ton_takes_a_changed_pt_from_the_call_that_samples_it() {
    let timer = Ton::ZEROED;
    timer.PT.set(30 * MS);
    let step = |now: i64, input: bool| {
        timer.IN.set(u8::from(input));
        ton(&timer, now * MS);
        (timer.Q.get(), timer.ET.get() / MS)
    };
    assert_eq!(step(0, true), (0, 0), "the edge call starts at 0");
    assert_eq!(step(10, true), (0, 10));
    assert_eq!(step(20, true), (0, 20));
    assert_eq!(step(30, true), (1, 30));
    assert_eq!(step(40, true), (1, 30), "ET stands at PT");

    timer.PT.set(100 * MS);
    assert_eq!(
        step(50, true),
        (0, 40),
        "a larger PT turns Q FALSE, ET goes on from 30 ms"
    );
    for now in [60, 70, 80, 90] {
        assert_eq!(step(now, true), (0, now - 10));
    }
    assert_eq!(step(100, true), (0, 90), "not yet 100 ms of ET");
    assert_eq!(step(110, true), (1, 100));

    timer.PT.set(60 * MS);
    assert_eq!(step(120, true), (1, 60), "a smaller PT: Q TRUE, ET at it");

    assert_eq!(step(130, false), (0, 0));
    assert_eq!(step(140, true), (0, 0));
    assert_eq!(step(150, true), (0, 10));
    timer.PT.set(5 * MS);
    assert_eq!(
        step(160, true),
        (1, 5),
        "a PT that ET reached ends the timing"
    );
}

/// Specification 08 §5, TP's worked example on a 10 ms task: `PT` T#50ms from the call at 20 ms
/// lengthens the pulse started at 0 ms with `PT` T#30ms, which ends at 50 ms with `ET` T#50ms; then
/// `ET` stays at 50 ms whatever `PT` later calls give while `IN` stays TRUE; a `PT` that `ET` has
/// reached ends a running pulse with `ET` at it. Catches a pulse that keeps its first `PT` (ending
/// at 30 ms), an `ET` that follows `PT` after the pulse, and a pulse that runs past a smaller `PT`.
#[test]
fn tp_takes_a_changed_pt_until_its_pulse_ends() {
    let timer = Tp::ZEROED;
    timer.PT.set(30 * MS);
    let step = |now: i64, input: bool| {
        timer.IN.set(u8::from(input));
        tp(&timer, now * MS);
        (timer.Q.get(), timer.ET.get() / MS)
    };
    assert_eq!(step(0, true), (1, 0));
    assert_eq!(step(10, true), (1, 10));
    timer.PT.set(50 * MS);
    assert_eq!(step(20, true), (1, 20), "a larger PT lengthens the pulse");
    assert_eq!(step(30, true), (1, 30));
    assert_eq!(step(40, true), (1, 40));
    assert_eq!(step(50, true), (0, 50), "the pulse ends at the new PT");

    timer.PT.set(10 * MS);
    assert_eq!(
        step(60, true),
        (0, 50),
        "ET holds the PT that ended the pulse"
    );
    assert_eq!(step(70, false), (0, 0));

    assert_eq!(step(80, true), (1, 0));
    timer.PT.set(50 * MS);
    assert_eq!(step(90, true), (1, 10));
    timer.PT.set(5 * MS);
    assert_eq!(
        step(100, true),
        (0, 5),
        "a PT that ET reached ends the pulse"
    );
}

/// Specification 08 §5, TOF's worked example on a 10 ms task: `PT` T#40ms from the call at 20 ms
/// lengthens the off-delay that started at 0 ms with `PT` T#20ms, and `Q` falls at 40 ms with `ET`
/// T#40ms; a `PT` that `ET` has reached ends a running off-delay with `ET` at it, and after expiry
/// `ET` stays at that `PT` whatever `PT` later calls give. Catches an off-delay that keeps its
/// first `PT` (`Q` FALSE at 20 ms) and an `ET` that follows `PT` after expiry.
#[test]
fn tof_takes_a_changed_pt_until_it_expires() {
    let timer = Tof::ZEROED;
    timer.PT.set(20 * MS);
    let step = |now: i64, input: bool| {
        timer.IN.set(u8::from(input));
        tof(&timer, now * MS);
        (timer.Q.get(), timer.ET.get() / MS)
    };
    assert_eq!(step(-10, true), (1, 0));
    assert_eq!(step(0, false), (1, 0), "the falling-edge call starts at 0");
    assert_eq!(step(10, false), (1, 10));
    timer.PT.set(40 * MS);
    assert_eq!(step(20, false), (1, 20), "a larger PT lengthens the delay");
    assert_eq!(step(30, false), (1, 30));
    assert_eq!(step(40, false), (0, 40), "Q falls at the new PT");

    assert_eq!(step(50, true), (1, 0));
    timer.PT.set(30 * MS);
    assert_eq!(step(60, false), (1, 0));
    assert_eq!(step(70, false), (1, 10));
    timer.PT.set(15 * MS);
    assert_eq!(
        step(80, false),
        (0, 15),
        "a PT that ET reached ends the delay"
    );
    timer.PT.set(100 * MS);
    assert_eq!(
        step(90, false),
        (0, 15),
        "ET holds the PT that ended the delay"
    );
}

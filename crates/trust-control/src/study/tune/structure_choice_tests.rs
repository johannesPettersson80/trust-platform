//! Tests of the structure a Tune designs for (specification 35, B4.2 and B4.4). The expected structures are the ones the specification's
//! order gives: the structure the request names, else the one the study records, and, for the manual method alone, the family the
//! Controller selector shows.

use std::collections::BTreeMap;

use super::structure_of_request;
use crate::messages::TuneMethod;
use crate::tuning::settings_pins::ControllerSettingsPins;
use crate::tuning::structure::Structure;

/// Pins recorded for the family `family`.
fn pins(family: Structure) -> ControllerSettingsPins {
    ControllerSettingsPins {
        loop_revision: "rev-1".to_owned(),
        family,
        values: BTreeMap::new(),
    }
}

#[test]
fn a_structure_the_request_names_is_never_replaced() {
    // B4.4: an explicitly chosen structure is never replaced silently. A request that names PI keeps PI, whatever the study records,
    // whatever its candidate is, and whatever method asks. Preferring the record would send the engineer's new choice back to the old one.
    for method in [None, Some(TuneMethod::Simc), Some(TuneMethod::Manual)] {
        assert_eq!(
            structure_of_request(
                Some(Structure::PI),
                method,
                Some(Structure::P),
                Some(Structure::PID),
                Some(&pins(Structure::PD)),
            ),
            Some(Structure::PI),
            "{method:?}"
        );
    }
}

#[test]
fn a_request_that_names_none_designs_for_the_structure_the_study_records() {
    // A8.1, B4.2: the Controller selector records the engineer's choice as the study's structure, so a Tune that names none designs for
    // it. Ignoring the record would design a PI under the method's default after the engineer chose P.
    for method in [None, Some(TuneMethod::Simc), Some(TuneMethod::Manual)] {
        assert_eq!(
            structure_of_request(None, method, Some(Structure::P), Some(Structure::PID), None),
            Some(Structure::P),
            "{method:?}"
        );
    }
}

#[test]
fn the_manual_method_with_no_choice_takes_the_family_the_selector_shows() {
    // B4.2: the selector shows the family of the study's candidate, else the family of its recorded pins. Manual settings belong to a
    // family, so with no choice they are designed for that one: the candidate first, then the pins.
    assert_eq!(
        structure_of_request(
            None,
            Some(TuneMethod::Manual),
            None,
            Some(Structure::PID),
            Some(&pins(Structure::PI)),
        ),
        Some(Structure::PID),
        "the candidate's family comes before the pins' family"
    );
    assert_eq!(
        structure_of_request(
            None,
            Some(TuneMethod::Manual),
            None,
            None,
            Some(&pins(Structure::PI)),
        ),
        Some(Structure::PI),
        "without a candidate the pins' family applies"
    );
    assert_eq!(
        structure_of_request(None, Some(TuneMethod::Manual), None, None, None),
        None,
        "with nothing to show, no family is invented: the engine refuses the settings for want of one"
    );
}

#[test]
fn another_method_with_no_choice_leaves_the_structure_to_its_own_design() {
    // B4.4: Automatic recommends the simplest structure that meets the requirements, and a named formula designs PI where the law offers
    // it. If the earlier candidate's family fixed the structure, Automatic could never move to another structure after a new model.
    for method in [
        None,
        Some(TuneMethod::Simc),
        Some(TuneMethod::Amigo),
        Some(TuneMethod::Relay),
        Some(TuneMethod::StateSpace),
    ] {
        assert_eq!(
            structure_of_request(
                None,
                method,
                None,
                Some(Structure::PID),
                Some(&pins(Structure::PI)),
            ),
            None,
            "{method:?}"
        );
    }
}

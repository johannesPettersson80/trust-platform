//! The number fields of the Ladder properties panel (specification 16 §2, specification 25
//! §11.4): a timer's preset in milliseconds, a counter's preset and an element's position.
//!
//! The panel sends the text typed into a field; [`set_number`] reads it as the number the program
//! stores and stores it, or refuses it and stores nothing, so a field never stores a number that
//! was not typed. Dragging an element is drawing and stays in the editor.

use super::geometry::snap;
use super::{network_mut, no_rung};
use crate::js;
use crate::json::js_round;
use crate::Json;

/// A number field of the properties panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberField {
    /// `presetMs` of a timer: rounded, a half up, and at least 0.
    TimerPreset,
    /// `preset` of a counter: rounded, a half up.
    CounterPreset,
    /// `position.x` of any element: on the 20-unit grid.
    PositionX,
    /// `position.y` of any element: on the 20-unit grid.
    PositionY,
}

impl NumberField {
    /// The field the panel names `name`: `presetMs`, `preset`, `x` or `y`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "presetMs" => Some(Self::TimerPreset),
            "preset" => Some(Self::CounterPreset),
            "x" => Some(Self::PositionX),
            "y" => Some(Self::PositionY),
            _ => None,
        }
    }

    /// The field's label in the panel.
    fn label(self) -> &'static str {
        match self {
            Self::TimerPreset => "Preset (ms)",
            Self::CounterPreset => "Preset",
            Self::PositionX => "Position X",
            Self::PositionY => "Position Y",
        }
    }

    /// The element kind the field belongs to; a position belongs to every element.
    fn kind(self) -> Option<&'static str> {
        match self {
            Self::TimerPreset => Some("timer"),
            Self::CounterPreset => Some("counter"),
            Self::PositionX | Self::PositionY => None,
        }
    }

    /// The number the field stores for `number`; `+ 0.0` writes `-0` as `0`.
    fn stored(self, number: f64) -> f64 {
        match self {
            Self::TimerPreset => js_round(number).max(0.0) + 0.0,
            Self::CounterPreset => js_round(number) + 0.0,
            Self::PositionX | Self::PositionY => snap(number) + 0.0,
        }
    }
}

/// Stores in `field` of the element `element` of the rung `rung` the number that `text`, typed
/// into the field, reads as, and answers the stored number.
///
/// The text reads as the panel's `Number(text)` read it ([`js::number_from_text`]): a decimal
/// number with an optional sign, fraction and exponent, or a `0x`, `0o` or `0b` integer, with
/// blanks around it. A timer preset is rounded, a half up, and a negative one becomes 0; a counter
/// preset is rounded, a half up; a position is put on the 20-unit grid. The element keeps its
/// other members and their order.
///
/// # Errors
/// There is no such rung or element, the field belongs to another kind of element, or the text is
/// blank, not a number or not finite. Nothing is stored then.
pub fn set_number(
    program: &mut Json,
    rung: usize,
    element: usize,
    field: NumberField,
    text: &str,
) -> Result<f64, String> {
    let network = network_mut(program, rung).ok_or_else(|| no_rung(rung))?;
    let node = network
        .get_mut("nodes")
        .and_then(Json::as_array_mut)
        .and_then(|nodes| nodes.get_mut(element))
        .ok_or_else(|| format!("Ladder rung {rung} has no element {element}."))?;
    let node_kind = node.str_at("type").unwrap_or_default().to_owned();
    if let Some(kind) = field.kind()
        && kind != node_kind
    {
        return Err(format!(
            "{} belongs to a {kind}, not to a {node_kind}.",
            field.label()
        ));
    }
    let number = Some(text)
        .filter(|text| !js::trim(text).is_empty())
        .and_then(js::number_from_text)
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("{} must be a number.", field.label()))?;
    let value = field.stored(number);
    match field {
        NumberField::TimerPreset => node.set("presetMs", Json::Number(value)),
        NumberField::CounterPreset => node.set("preset", Json::Number(value)),
        NumberField::PositionX | NumberField::PositionY => {
            let axis = if field == NumberField::PositionX {
                "x"
            } else {
                "y"
            };
            let mut position = node.get("position").cloned().unwrap_or_else(Json::object);
            position.set(axis, Json::Number(value));
            node.set("position", position);
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROGRAM: &str = r#"{
      "schemaVersion": 2,
      "networks": [
        {
          "id": "rung_1",
          "order": 0,
          "nodes": [
            { "id": "c1", "type": "contact", "contactType": "NO", "variable": "Start", "position": { "x": 100, "y": 100 } },
            { "id": "t1", "type": "timer", "timerType": "TON", "instance": "T_1", "input": "", "presetMs": 1000, "qOutput": "T_1_Q", "etOutput": "T_1_ET", "position": { "x": 200, "y": 100 } },
            { "id": "k1", "type": "counter", "counterType": "CTU", "instance": "C_1", "input": "", "preset": 10, "qOutput": "C_1_Q", "cvOutput": "C_1_CV", "position": { "x": 400, "y": 100 } }
          ],
          "edges": [],
          "layout": { "y": 100 }
        }
      ],
      "variables": [],
      "metadata": { "name": "fields", "description": "" }
    }"#;

    fn program() -> Json {
        Json::parse(PROGRAM).expect("test program")
    }

    /// The number `text` stores in `field` of the element at `element`, and the element after it.
    fn typed(element: usize, field: NumberField, text: &str) -> (Result<f64, String>, Json) {
        let mut program = program();
        let answer = set_number(&mut program, 0, element, field, text);
        let node = program
            .get("networks")
            .and_then(Json::as_array)
            .expect("networks")[0]
            .get("nodes")
            .and_then(Json::as_array)
            .expect("nodes")[element]
            .clone();
        (answer, node)
    }

    #[test]
    fn a_timer_preset_is_rounded_half_up_and_kept_from_zero() {
        // Worked by hand from Math.max(0, Math.round(Number(text))): 12.5 rounds up to 13, -12.5
        // rounds to -12 and becomes 0, 2e3 is 2000, 0x10 is 16, and blanks around a number do not
        // matter. Catches a truncating or banker's rounding, a negative preset, or exponent and
        // hexadecimal text read as something else.
        for (text, stored) in [
            ("1500", 1500.0),
            ("12.5", 13.0),
            ("12.49", 12.0),
            ("-12.5", 0.0),
            ("-3", 0.0),
            (" 2e3 ", 2000.0),
            ("0x10", 16.0),
        ] {
            let (answer, node) = typed(1, NumberField::TimerPreset, text);
            assert_eq!(answer, Ok(stored), "{text}");
            assert_eq!(
                node.get("presetMs").and_then(Json::as_f64),
                Some(stored),
                "{text}"
            );
        }
    }

    #[test]
    fn a_counter_preset_is_rounded_half_up_and_may_be_negative() {
        // Math.round(Number(text)): -3.5 rounds towards positive infinity to -3, 7.5 to 8; a counter
        // preset is not kept from zero. Catches rounding away from zero (-4) or a clamp copied
        // from the timer preset.
        for (text, stored) in [("-3.5", -3.0), ("7.5", 8.0), ("7.49", 7.0), ("-0.4", 0.0)] {
            let (answer, node) = typed(2, NumberField::CounterPreset, text);
            assert_eq!(answer, Ok(stored), "{text}");
            assert_eq!(
                node.get("preset").and_then(Json::as_f64),
                Some(stored),
                "{text}"
            );
        }
        // -0.4 rounds to zero (JavaScript's -0), which the file writes as 0, as JSON.stringify did.
        let (_, node) = typed(2, NumberField::CounterPreset, "-0.4");
        assert_eq!(
            node.get("preset").map(Json::to_compact),
            Some("0".to_owned())
        );
    }

    #[test]
    fn a_position_is_put_on_the_twenty_unit_grid() {
        // Math.round(n / 20) * 20: 130 is 6.5 grid steps and goes up to 140, 129 is 6.45 and goes
        // down to 120, -10 is -0.5 and goes up to 0. The other coordinate keeps its value and
        // place. Catches a position stored off the grid the canvas draws on.
        let (answer, node) = typed(0, NumberField::PositionX, "130");
        assert_eq!(answer, Ok(140.0));
        assert_eq!(
            node.get("position").map(Json::to_compact),
            Some("{\"x\":140,\"y\":100}".to_owned())
        );
        let (answer, node) = typed(0, NumberField::PositionY, "129");
        assert_eq!(answer, Ok(120.0));
        assert_eq!(
            node.get("position").map(Json::to_compact),
            Some("{\"x\":100,\"y\":120}".to_owned())
        );
        let (answer, _) = typed(0, NumberField::PositionX, "-10");
        assert_eq!(answer, Ok(0.0));
    }

    #[test]
    fn text_that_is_no_number_is_refused_and_nothing_is_stored() {
        // Blank text was stored as 0 (Number("") is 0) and Infinity could reach a preset; now each
        // is refused with the field's label and the program keeps its value. Catches a field that
        // silently stores a number nobody typed.
        for text in ["", "   ", "abc", "12abc", "Infinity", "-Infinity", "1e400"] {
            let (answer, node) = typed(1, NumberField::TimerPreset, text);
            assert_eq!(
                answer,
                Err("Preset (ms) must be a number.".to_owned()),
                "{text:?}"
            );
            assert_eq!(node.get("presetMs").and_then(Json::as_f64), Some(1000.0));
        }
        let (answer, _) = typed(0, NumberField::PositionY, "");
        assert_eq!(answer, Err("Position Y must be a number.".to_owned()));
    }

    #[test]
    fn a_field_of_another_element_or_a_missing_element_is_refused() {
        // A preset belongs to its own element kind; a wrong rung or element index changes nothing.
        // Catches a preset written onto a contact or into the wrong rung.
        let mut changed = program();
        assert_eq!(
            set_number(&mut changed, 0, 0, NumberField::TimerPreset, "5"),
            Err("Preset (ms) belongs to a timer, not to a contact.".to_owned())
        );
        assert_eq!(
            set_number(&mut changed, 0, 1, NumberField::CounterPreset, "5"),
            Err("Preset belongs to a counter, not to a timer.".to_owned())
        );
        assert_eq!(
            set_number(&mut changed, 0, 9, NumberField::PositionX, "5"),
            Err("Ladder rung 0 has no element 9.".to_owned())
        );
        assert_eq!(
            set_number(&mut changed, 4, 0, NumberField::PositionX, "5"),
            Err("Ladder rung 4 does not exist.".to_owned())
        );
        assert_eq!(changed, program());
    }

    #[test]
    fn the_fields_are_named_as_the_panel_names_them() {
        // The panel sends the member name it edits. Catches a field the panel can never reach.
        assert_eq!(
            NumberField::from_name("presetMs"),
            Some(NumberField::TimerPreset)
        );
        assert_eq!(
            NumberField::from_name("preset"),
            Some(NumberField::CounterPreset)
        );
        assert_eq!(NumberField::from_name("x"), Some(NumberField::PositionX));
        assert_eq!(NumberField::from_name("y"), Some(NumberField::PositionY));
        assert_eq!(NumberField::from_name("pt"), None);
    }
}

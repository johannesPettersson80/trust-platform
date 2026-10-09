//! Specification 39, §5.2 and §5.6: the shipped component catalogue.

use super::*;
use crate::harness::TestHarness;
use crate::hmi::member_paths::{resolve_member, MemberPath};

/// Phase 1 of §5.2: displays, controls and layout components.
const PHASE_ONE: &[(&str, ComponentCategory)] = &[
    ("text", ComponentCategory::Display),
    ("image", ComponentCategory::Display),
    ("value", ComponentCategory::Display),
    ("gauge", ComponentCategory::Display),
    ("bar", ComponentCategory::Display),
    ("split-bar", ComponentCategory::Display),
    ("trend", ComponentCategory::Display),
    ("status", ComponentCategory::Display),
    ("table", ComponentCategory::Display),
    ("event-log", ComponentCategory::Display),
    ("alarm-list", ComponentCategory::Display),
    ("button", ComponentCategory::Control),
    ("pulse-button", ComponentCategory::Control),
    ("switch", ComponentCategory::Control),
    ("selector", ComponentCategory::Control),
    ("mode-selector", ComponentCategory::Control),
    ("slider", ComponentCategory::Control),
    ("numeric-input", ComponentCategory::Control),
    ("setpoint", ComponentCategory::Control),
    ("panel", ComponentCategory::Layout),
    ("group", ComponentCategory::Layout),
    ("page-button", ComponentCategory::Layout),
    ("line", ComponentCategory::Layout),
];

#[test]
fn every_phase_one_component_is_defined() {
    let catalogue = Catalogue::shipped();
    assert_eq!(catalogue.format, 1);
    for (kind, category) in PHASE_ONE {
        let component = catalogue
            .component(kind)
            .unwrap_or_else(|| panic!("{kind} is not in the catalogue"));
        assert_eq!(component.category, *category, "{kind}");
        assert_eq!(component.phase, 1, "{kind}");
    }
}

/// Phase 2 of §5.2, milestone 2A (§14.3, item 2): the pump, the on/off valve and the control
/// valve, each with the bindings that drive its states (§5.3, item 4).
#[test]
fn the_pump_and_the_valves_are_equipment_with_the_bindings_of_their_states() {
    let catalogue = Catalogue::shipped();
    let position = catalogue
        .component("control-valve")
        .and_then(|valve| valve.properties.get("position"))
        .expect("the control valve takes its position");
    assert_eq!(position.accepts, [ValueKind::Number]);
    assert_eq!(position.directions, [HmiBindingDirection::Read]);
    for (kind, states) in [
        (
            "pump",
            &["fault", "manual", "out-of-service", "running"][..],
        ),
        (
            "valve",
            &["closed", "fault", "manual", "open", "out-of-service"][..],
        ),
        ("control-valve", &["fault", "manual", "out-of-service"][..]),
    ] {
        let component = catalogue
            .component(kind)
            .unwrap_or_else(|| panic!("the catalogue has the {kind}"));
        assert_eq!(component.category, ComponentCategory::Equipment, "{kind}");
        assert_eq!(component.phase, 2, "{kind}");
        // The numbers: a control valve's position and demand, a pump's speed.
        let states_of = || {
            component.properties.iter().filter(|(name, _)| {
                !["position", "demand", "speed", VISIBLE].contains(&name.as_str())
            })
        };
        let names: Vec<&str> = states_of().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, states, "{kind}");
        for (name, property) in states_of() {
            assert_eq!(property.accepts, [ValueKind::Boolean], "{kind}.{name}");
            assert_eq!(
                property.directions,
                [HmiBindingDirection::Read],
                "{kind}.{name}"
            );
        }
    }
}

#[test]
fn definitions_are_well_formed() {
    let catalogue = Catalogue::shipped();
    let mut seen = std::collections::HashSet::new();
    for component in &catalogue.components {
        assert!(
            seen.insert(component.kind.as_str()),
            "{} twice",
            component.kind
        );
        assert!(
            component.size.iter().all(|side| *side > 0),
            "{}",
            component.kind
        );
        for (name, property) in component.properties.iter().chain(
            component
                .additional
                .as_ref()
                .map(|rule| (&component.kind, rule)),
        ) {
            assert!(!property.accepts.is_empty(), "{}.{name}", component.kind);
            assert!(!property.directions.is_empty(), "{}.{name}", component.kind);
        }
    }
}

#[test]
fn displays_only_read_and_controls_write() {
    let catalogue = Catalogue::shipped();
    let bar = catalogue.component("bar").expect("bar");
    let value = bar.property("value").expect("bar.value");
    assert_eq!(value.accepts, vec![ValueKind::Number]);
    assert_eq!(value.directions, vec![HmiBindingDirection::Read]);
    assert!(bar.property("colour").is_none());

    let switch = catalogue.component("switch").expect("switch");
    let value = switch.property("value").expect("switch.value");
    assert_eq!(value.accepts, vec![ValueKind::Boolean]);
    assert!(value.directions.contains(&HmiBindingDirection::ReadWrite));
}

#[test]
fn a_trend_accepts_named_pens() {
    let trend = Catalogue::shipped().component("trend").expect("trend");
    let pen = trend.property("pv").expect("any pen name");
    assert!(pen.accepts.contains(&ValueKind::Number));
    assert_eq!(pen.directions, vec![HmiBindingDirection::Read]);
}

#[test]
fn value_kinds_match_runtime_values() {
    let harness = TestHarness::from_source(
        r#"
TYPE Mode : (Off, Auto); END_TYPE
TYPE Reading :
STRUCT
    Value : REAL;
END_STRUCT
END_TYPE
PROGRAM Main
VAR
    Level : REAL;
    Status : WORD;
    Run : BOOL;
    Note : STRING;
    Uptime : TIME;
    State : Mode;
    Sample : Reading;
    Samples : ARRAY[1..2] OF REAL;
END_VAR
END_PROGRAM
"#,
    )
    .expect("the program compiles");
    let value = |name: &str| {
        let path = MemberPath::parse(&format!("Main.{name}")).expect("path");
        resolve_member(&path, harness.runtime().storage())
            .unwrap_or_else(|| panic!("Main.{name}"))
            .clone()
    };
    let kinds = |name: &str| {
        let value = value(name);
        [
            ValueKind::Number,
            ValueKind::Boolean,
            ValueKind::Text,
            ValueKind::Time,
            ValueKind::Enumeration,
            ValueKind::Structure,
            ValueKind::Array,
            ValueKind::Value,
        ]
        .into_iter()
        .filter(|kind| kind.matches(&value))
        .collect::<Vec<_>>()
    };
    use ValueKind::*;
    assert_eq!(kinds("Level"), vec![Number, Value]);
    assert_eq!(kinds("Status"), vec![Number, Value]);
    assert_eq!(kinds("Run"), vec![Boolean, Value]);
    assert_eq!(kinds("Note"), vec![Text, Value]);
    assert_eq!(kinds("Uptime"), vec![Time, Value]);
    assert_eq!(kinds("State"), vec![Enumeration, Value]);
    assert_eq!(kinds("Sample"), vec![Structure]);
    assert_eq!(kinds("Samples"), vec![Array]);
}

/// Specification 39, §12.5–12.6 and §12.8: every control can name the feedback that confirms its
/// command, the interlock that permits it, and the interlock's reason text.
#[test]
fn every_control_names_its_feedback_interlock_and_reason() {
    let catalogue = Catalogue::shipped();
    let controls: Vec<&ComponentDef> = catalogue
        .components
        .iter()
        .filter(|component| component.category == ComponentCategory::Control)
        .collect();
    assert!(controls.len() >= 8, "{} controls", controls.len());
    for component in controls {
        for (property, accepts) in [
            ("feedback", ValueKind::Value),
            ("interlock", ValueKind::Boolean),
            ("interlock-reason", ValueKind::Text),
        ] {
            let rule = component
                .properties
                .get(property)
                .unwrap_or_else(|| panic!("a {} has no {property}", component.kind));
            assert_eq!(rule.accepts, [accepts], "{}.{property}", component.kind);
            assert_eq!(
                rule.directions,
                [HmiBindingDirection::Read],
                "{}.{property}",
                component.kind
            );
        }
    }
}

/// Specification 39, §3.5.2 and §14.1 item 4: a new control or page button is a touch target of
/// at least 44 × 44 px.
#[test]
fn every_control_and_page_button_starts_at_least_44_by_44() {
    let small: Vec<(&str, [u32; 2])> = Catalogue::shipped()
        .components
        .iter()
        .filter(|component| {
            component.category == ComponentCategory::Control || component.kind == "page-button"
        })
        .filter(|component| component.size[0] < 44 || component.size[1] < 44)
        .map(|component| (component.kind.as_str(), component.size))
        .collect();
    assert_eq!(small, vec![], "touch targets smaller than 44 × 44");
}

/// Specification 39, §5.2: a new setpoint entry has room for its label, value and limits (at
/// least 80 px high, below which it draws one line), and a numeric input for about eight letters
/// of label beside its value and edit mark.
#[test]
fn entries_start_with_room_for_what_they_show() {
    let size = |kind: &str| {
        Catalogue::shipped()
            .component(kind)
            .unwrap_or_else(|| panic!("{kind}"))
            .size
    };
    assert_eq!(size("setpoint"), [240, 96]);
    assert_eq!(size("numeric-input"), [208, 48]);
}

/// Specification 39, §5.7 items 4 and 5: a new alarm list or event log has room for its tab row
/// and six rows of 52 px, and the alarm list for its four tabs and pager side by side.
#[test]
fn alarm_lists_and_event_logs_start_with_room_for_six_rows() {
    for kind in ["alarm-list", "event-log"] {
        let size = Catalogue::shipped()
            .component(kind)
            .unwrap_or_else(|| panic!("{kind} is in the catalogue"))
            .size;
        assert_eq!(size, [720, 376], "{kind}");
    }
}

/// Phase 2 of §5.2, liquid: the open and the gravity-drained tank, the tank cascade, the dosing
/// pump, the supply line and the mixing point, with the numbers that drive them (§5.3, item 5);
/// the transfer pump in variable speed and the control valve with its positioner as variants.
#[test]
fn the_liquid_equipment_is_defined_with_its_bindings_ports_and_variants() {
    let catalogue = Catalogue::shipped();
    let number = |kind: &str, name: &str| {
        let property = catalogue
            .component(kind)
            .and_then(|component| component.properties.get(name))
            .unwrap_or_else(|| panic!("the {kind} takes {name}"));
        assert_eq!(property.accepts, [ValueKind::Number], "{kind}.{name}");
        assert_eq!(
            property.directions,
            [HmiBindingDirection::Read],
            "{kind}.{name}"
        );
    };
    for (kind, numbers, ports) in [
        ("tank", &["level"][..], &["inlet", "outlet"][..]),
        ("gravity-tank", &["level"][..], &["inlet", "outlet"][..]),
        (
            "tank-cascade",
            &["lower-level", "upper-level"][..],
            &["inlet", "outlet"][..],
        ),
        ("dosing-pump", &["rate"][..], &["inlet", "outlet"][..]),
        ("supply", &["pressure"][..], &["outlet"][..]),
        ("mixer", &["ratio"][..], &["inlet", "inlet-2", "outlet"][..]),
    ] {
        let component = catalogue
            .component(kind)
            .unwrap_or_else(|| panic!("the catalogue has the {kind}"));
        assert_eq!(component.category, ComponentCategory::Equipment, "{kind}");
        assert_eq!(component.phase, 2, "{kind}");
        assert_eq!(component.ports, ports, "{kind}");
        for name in numbers {
            number(kind, name);
        }
    }
    let dosing = catalogue.component("dosing-pump").expect("defined");
    for state in ["fault", "manual", "out-of-service", "running"] {
        let property = dosing
            .properties
            .get(state)
            .unwrap_or_else(|| panic!("dosing-pump.{state}"));
        assert_eq!(
            property.accepts,
            [ValueKind::Boolean],
            "dosing-pump.{state}"
        );
    }
    number("pump", "speed");
    number("control-valve", "demand");
    for (kind, variant) in [
        ("pump", "variable-speed"),
        ("control-valve", "positioner"),
        ("supply", "regulated"),
    ] {
        assert_eq!(
            catalogue.component(kind).expect("defined").variants,
            [variant],
            "{kind}"
        );
    }
}

/// Phase 2 of §5.2, thermal: the cold room and its door, the refrigeration unit, the electric
/// heater with its water vessel, the cooler, the jacketed vessel, the coupled vessels and the
/// heat draw-off, each with the bindings that drive it and the ports of its pipes.
#[test]
fn the_thermal_equipment_is_defined_with_its_bindings_and_ports() {
    let catalogue = Catalogue::shipped();
    let machine = &["fault", "manual", "out-of-service", "running"][..];
    for (kind, numbers, booleans, ports) in [
        (
            "cold-room",
            &["temperature"][..],
            &["door-open"][..],
            &[][..],
        ),
        ("refrigeration-unit", &[][..], machine, &[][..]),
        (
            "heater",
            &["level", "power", "temperature"][..],
            &[][..],
            &["inlet", "outlet"][..],
        ),
        ("cooler", &[][..], machine, &["inlet", "outlet"][..]),
        (
            "jacketed-vessel",
            &["level", "temperature"][..],
            &[][..],
            &["inlet", "outlet", "jacket-inlet", "jacket-outlet"][..],
        ),
        (
            "coupled-vessels",
            &["temperature-1", "temperature-2"][..],
            &[][..],
            &["inlet-1", "outlet-1", "inlet-2", "outlet-2"][..],
        ),
        (
            "heat-draw-off",
            &["power"][..],
            &[][..],
            &["inlet", "outlet"][..],
        ),
    ] {
        let component = catalogue
            .component(kind)
            .unwrap_or_else(|| panic!("the catalogue has the {kind}"));
        assert_eq!(component.category, ComponentCategory::Equipment, "{kind}");
        assert_eq!(component.phase, 2, "{kind}");
        assert_eq!(component.ports, ports, "{kind}");
        let of = |accepts: ValueKind| -> Vec<&str> {
            component
                .properties
                .iter()
                .filter(|(name, property)| {
                    name.as_str() != VISIBLE && property.accepts == [accepts]
                })
                .map(|(name, _)| name.as_str())
                .collect()
        };
        assert_eq!(of(ValueKind::Number), numbers, "{kind}");
        assert_eq!(of(ValueKind::Boolean), booleans, "{kind}");
    }
}

/// Phase 2 of §5.2, motion: the motor, the servo motor with its drive, the shaft, the drum with its
/// hanging load, the flexible coupling, the encoder, the rotary arm and the axis position
/// indicator, each with the numbers that turn it; they have no pipes.
#[test]
fn the_motion_equipment_is_defined_with_the_numbers_that_turn_it() {
    let catalogue = Catalogue::shipped();
    for (kind, numbers, booleans) in [
        (
            "motor",
            &[][..],
            &["fault", "manual", "out-of-service", "running"][..],
        ),
        (
            "servo-motor",
            &["angle", "speed"][..],
            &["fault", "manual", "out-of-service", "running"][..],
        ),
        ("shaft", &["angle", "speed"][..], &[][..]),
        ("drum", &["angle", "position"][..], &[][..]),
        ("coupling", &["load-angle", "motor-angle"][..], &[][..]),
        ("encoder", &["angle"][..], &[][..]),
        ("rotary-arm", &["angle"][..], &[][..]),
        ("axis-indicator", &["actual", "target"][..], &[][..]),
    ] {
        let component = catalogue
            .component(kind)
            .unwrap_or_else(|| panic!("the catalogue has the {kind}"));
        assert_eq!(component.category, ComponentCategory::Equipment, "{kind}");
        assert_eq!(component.phase, 2, "{kind}");
        assert!(component.ports.is_empty(), "{kind} has no pipes");
        let of = |accepts: ValueKind| -> Vec<&str> {
            component
                .properties
                .iter()
                .filter(|(name, property)| {
                    name.as_str() != VISIBLE && property.accepts == [accepts]
                })
                .map(|(name, _)| name.as_str())
                .collect()
        };
        assert_eq!(of(ValueKind::Number), numbers, "{kind}");
        assert_eq!(of(ValueKind::Boolean), booleans, "{kind}");
    }
}

/// Phase 2 of §5.2, communication: the network link's status, its connection, its packets' age,
/// holding the last value and falling back to local control.
#[test]
fn the_network_link_shows_connection_age_hold_and_fallback() {
    let link = Catalogue::shipped()
        .component("network-link")
        .expect("the catalogue has the network link");
    assert_eq!(link.category, ComponentCategory::Equipment);
    assert_eq!(link.phase, 2);
    for (name, kind) in [
        ("age", ValueKind::Number),
        ("connected", ValueKind::Boolean),
        ("fallback", ValueKind::Boolean),
        ("hold", ValueKind::Boolean),
    ] {
        assert_eq!(
            link.properties
                .get(name)
                .map(|property| property.accepts.clone()),
            Some(vec![kind]),
            "network-link.{name}"
        );
    }
}

/// Specification 39, §5.6 item 1: each piece of equipment names the heading of §5.2 it is listed
/// under, which groups it in the editor's Components panel; nothing else has a group.
#[test]
fn equipment_is_grouped_by_the_headings_of_phase_two() {
    let catalogue = Catalogue::shipped();
    let headings = [
        "liquid",
        "thermal",
        "motion",
        "instruments",
        "communication",
        "process",
        "electrical",
    ];
    for component in &catalogue.components {
        match component.category {
            ComponentCategory::Equipment => assert!(
                component
                    .group
                    .as_deref()
                    .is_some_and(|group| headings.contains(&group)),
                "{} is grouped: {:?}",
                component.kind,
                component.group
            ),
            _ => assert_eq!(component.group, None, "{}", component.kind),
        }
    }
    let group = |kind: &str| {
        catalogue
            .component(kind)
            .and_then(|component| component.group.clone())
    };
    assert_eq!(group("pump").as_deref(), Some("liquid"));
    assert_eq!(group("temperature-sensor").as_deref(), Some("instruments"));
    assert_eq!(group("network-link").as_deref(), Some("communication"));
}

/// Phase 2 of §5.2, instrumentation, and §6 item 5: the ISA instruments, each with its value and
/// sensor fault, and the redundant sensor pair; they attach to what they measure, and only they.
#[test]
fn the_instruments_measure_a_value_and_attach_to_what_they_measure() {
    let catalogue = Catalogue::shipped();
    for kind in [
        "temperature-sensor",
        "level-sensor",
        "flow-sensor",
        "pressure-sensor",
        "position-sensor",
        "speed-sensor",
    ] {
        let component = catalogue
            .component(kind)
            .unwrap_or_else(|| panic!("the catalogue has the {kind}"));
        assert_eq!(component.category, ComponentCategory::Equipment, "{kind}");
        assert_eq!(component.phase, 2, "{kind}");
        assert!(component.attaches, "{kind} attaches");
        assert_eq!(
            component
                .properties
                .get("value")
                .map(|property| property.accepts.clone()),
            Some(vec![ValueKind::Number]),
            "{kind}"
        );
        assert_eq!(
            component
                .properties
                .get("fault")
                .map(|property| property.accepts.clone()),
            Some(vec![ValueKind::Boolean]),
            "{kind}"
        );
    }
    let pair = catalogue
        .component("sensor-pair")
        .expect("the catalogue has the sensor pair");
    assert!(pair.attaches);
    for (name, kind) in [
        ("a", ValueKind::Number),
        ("b", ValueKind::Number),
        ("disagree", ValueKind::Boolean),
    ] {
        assert_eq!(
            pair.properties
                .get(name)
                .map(|property| property.accepts.clone()),
            Some(vec![kind]),
            "sensor-pair.{name}"
        );
    }
    assert!(
        !catalogue.component("pump").expect("defined").attaches,
        "equipment does not attach"
    );
}

/// §8.1 and §5.2 instrumentation: the loop controller, the symbol of the PID loop and controller
/// pairs (an ISA-5.1 shared display), with its measurement, its setpoint and manual; it is no field
/// instrument, so it does not attach: its loop shows as signal lines (§6 item 5).
#[test]
fn the_loop_controller_shows_its_measurement_setpoint_and_manual() {
    let controller = Catalogue::shipped()
        .component("controller")
        .expect("the catalogue has the loop controller");
    assert_eq!(controller.category, ComponentCategory::Equipment);
    assert_eq!(controller.phase, 2);
    assert_eq!(controller.group.as_deref(), Some("instruments"));
    assert!(!controller.attaches);
    for (name, kind) in [
        ("pv", ValueKind::Number),
        ("sp", ValueKind::Number),
        ("manual", ValueKind::Boolean),
    ] {
        assert_eq!(
            controller
                .properties
                .get(name)
                .map(|property| property.accepts.clone()),
            Some(vec![kind]),
            "controller.{name}"
        );
    }
}

/// §8.8 and specification 41, §7 and §8: the symbols of the equipment PLC programmers need, each
/// under its heading with the properties its pair binds.
#[test]
fn the_plant_equipment_has_its_symbols() {
    let catalogue = Catalogue::shipped();
    let running = &["fault", "manual", "out-of-service", "running"][..];
    let positioned = &["closed", "fault", "manual", "open", "out-of-service"][..];
    for (kind, group, booleans, numbers) in [
        ("fan", "thermal", running, &["speed"][..]),
        ("blower", "liquid", running, &[][..]),
        ("agitator", "liquid", running, &[][..]),
        ("conveyor", "motion", running, &[][..]),
        ("damper", "thermal", positioned, &["position"][..]),
        ("cylinder", "motion", positioned, &[][..]),
        ("motor-valve", "liquid", positioned, &[][..]),
        ("weigh-scale", "instruments", &["fault"][..], &["value"][..]),
        ("analyzer", "instruments", &["fault"][..], &["value"][..]),
        (
            "level-switch",
            "instruments",
            &["fault", "state"][..],
            &[][..],
        ),
        (
            "pressure-switch",
            "instruments",
            &["fault", "state"][..],
            &[][..],
        ),
        (
            "digital-switch",
            "instruments",
            &["fault", "state"][..],
            &[][..],
        ),
    ] {
        let component = catalogue
            .component(kind)
            .unwrap_or_else(|| panic!("the catalogue has the {kind}"));
        assert_eq!(component.category, ComponentCategory::Equipment, "{kind}");
        assert_eq!(component.group.as_deref(), Some(group), "{kind}");
        let of = |accepts: ValueKind| -> Vec<&str> {
            let mut names: Vec<&str> = component
                .properties
                .iter()
                .filter(|(name, property)| {
                    name.as_str() != VISIBLE && property.accepts == [accepts]
                })
                .map(|(name, _)| name.as_str())
                .collect();
            names.sort();
            names
        };
        assert_eq!(of(ValueKind::Boolean), booleans, "{kind}");
        assert_eq!(of(ValueKind::Number), numbers, "{kind}");
    }
    for kind in [
        "analyzer",
        "level-switch",
        "pressure-switch",
        "digital-switch",
    ] {
        assert!(
            catalogue.component(kind).expect("defined").attaches,
            "{kind} attaches to what it measures"
        );
    }
}

/// Phase 3 of §5.2: the process and electrical equipment, each under its heading with its
/// bindings and ports; electrical symbols have none, a single-line diagram joins them with lines.
#[test]
fn the_phase_three_equipment_has_its_symbols() {
    /// A component: its type, heading, boolean and number bindings, and ports.
    type Expected = (
        &'static str,
        &'static str,
        &'static [&'static str],
        &'static [&'static str],
        &'static [&'static str],
    );
    let catalogue = Catalogue::shipped();
    let running = &["fault", "manual", "out-of-service", "running"][..];
    let cases: [Expected; 11] = [
        ("vessel", "process", &[], &["level"], &["inlet", "outlet"]),
        (
            "heat-exchanger",
            "process",
            &["fault"],
            &["temperature"],
            &["inlet", "outlet", "medium-inlet", "medium-outlet"],
        ),
        (
            "filter",
            "process",
            &["clogged", "fault"],
            &["differential-pressure"],
            &["inlet", "outlet"],
        ),
        (
            "cyclone",
            "process",
            &["fault"],
            &["differential-pressure"],
            &["inlet", "gas-outlet", "solids-outlet"],
        ),
        (
            "crusher",
            "process",
            running,
            &["load"],
            &["inlet", "outlet"],
        ),
        (
            "screen",
            "process",
            running,
            &[],
            &["inlet", "oversize", "undersize"],
        ),
        ("vfd", "electrical", running, &["frequency"], &[]),
        (
            "drive",
            "electrical",
            &["fault", "manual", "out-of-service", "ready", "running"],
            &["speed"],
            &[],
        ),
        ("contactor", "electrical", &["closed", "fault"], &[], &[]),
        ("breaker", "electrical", &["closed", "tripped"], &[], &[]),
        ("device", "process", running, &[], &[]),
    ];
    for (kind, group, booleans, numbers, ports) in cases {
        let component = catalogue
            .component(kind)
            .unwrap_or_else(|| panic!("the catalogue has the {kind}"));
        assert_eq!(component.category, ComponentCategory::Equipment, "{kind}");
        assert_eq!(component.phase, 3, "{kind}");
        assert_eq!(component.group.as_deref(), Some(group), "{kind}");
        let of = |accepts: ValueKind| -> Vec<&str> {
            let mut names: Vec<&str> = component
                .properties
                .iter()
                .filter(|(name, property)| {
                    name.as_str() != VISIBLE && property.accepts == [accepts]
                })
                .map(|(name, _)| name.as_str())
                .collect();
            names.sort();
            names
        };
        assert_eq!(of(ValueKind::Boolean), booleans, "{kind}");
        assert_eq!(of(ValueKind::Number), numbers, "{kind}");
        assert_eq!(component.ports, ports, "{kind}");
        assert!(
            component
                .properties
                .values()
                .all(|property| property.directions == [HmiBindingDirection::Read]),
            "{kind} only shows"
        );
    }
}

/// Specification 39, §6: each symbol declares its ports, so a pipe's ends can be checked; a
/// junction joins pipes, one on each side.
#[test]
fn equipment_declares_the_ports_its_pipes_connect_to() {
    let catalogue = Catalogue::shipped();
    for kind in ["pump", "valve", "control-valve"] {
        let component = catalogue.component(kind).expect("defined");
        assert_eq!(component.ports, ["inlet", "outlet"], "{kind}");
    }
    let junction = catalogue
        .component("junction")
        .expect("a junction is defined");
    assert_eq!(junction.ports, ["left", "right", "top", "bottom"]);
    assert!(
        catalogue
            .component("value")
            .expect("defined")
            .ports
            .is_empty(),
        "a display has no ports"
    );
}

/// Specification 39, §18 phase 5 item 2: every object may be shown only while a BOOL is TRUE.
#[test]
fn every_component_takes_a_visibility_binding_of_a_boolean() {
    for component in &Catalogue::shipped().components {
        let visible = component
            .properties
            .get("visible")
            .unwrap_or_else(|| panic!("{} has no visibility binding", component.kind));
        assert_eq!(
            visible.accepts,
            vec![ValueKind::Boolean],
            "{}",
            component.kind
        );
    }
}

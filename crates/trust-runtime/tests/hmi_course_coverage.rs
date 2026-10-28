//! Specification 39, §5.5: every plant the control course uses has its component in the
//! catalogue, and its PID loops have the Plant library's PID panel (a cascade is two linked loops,
//! each with that panel); the course's other controller structures are shown with the general
//! components (values, setpoints, trends).

use std::path::Path;

use trust_runtime::hmi::catalogue::Catalogue;

/// The course's parts, each with the components and faceplates it uses (§5.5's table).
const COURSE: &[(&str, &[&str], &[&str])] = &[
    (
        "Cold room",
        &["cold-room", "refrigeration-unit", "temperature-sensor"],
        &[],
    ),
    (
        "Water heater",
        &["heater", "heat-draw-off", "temperature-sensor", "trend"],
        &["plant-pid"],
    ),
    (
        "Valve-fed heater",
        &["control-valve", "supply", "temperature-sensor"],
        &[],
    ),
    (
        "Shaft and drum",
        &["servo-motor", "shaft", "drum", "encoder", "axis-indicator"],
        &["plant-pid"],
    ),
    (
        "Tanks",
        &["tank", "gravity-tank", "pump", "level-sensor"],
        &["plant-pid"],
    ),
    ("Classical design", &["coupling", "trend"], &["plant-pid"]),
    (
        "Industrial structures",
        &[
            "jacketed-vessel",
            "mixer",
            "dosing-pump",
            "heater",
            "cooler",
        ],
        &["plant-pid"],
    ),
    ("Multivariable", &["coupled-vessels"], &["plant-pid"]),
    ("State space", &["tank-cascade"], &[]),
    (
        "Nonlinear, adaptive and robust",
        &["rotary-arm"],
        &["plant-pid"],
    ),
    (
        "Networked and fault-aware, loop diagnosis",
        &["network-link", "sensor-pair"],
        &[],
    ),
];

#[test]
fn every_plant_and_controller_type_of_the_course_has_a_component_or_panel() {
    let catalogue = Catalogue::shipped();
    let faceplates = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../libraries/plant/hmi/panels");
    let mut missing = Vec::new();
    for (part, components, shipped) in COURSE {
        missing.extend(
            components
                .iter()
                .filter(|kind| catalogue.component(kind).is_none())
                .map(|kind| format!("{part}: the component {kind}")),
        );
        missing.extend(
            shipped
                .iter()
                .filter(|name| !faceplates.join(format!("{name}.toml")).is_file())
                .map(|name| format!("{part}: the panel {name}")),
        );
    }
    assert_eq!(missing, Vec::<String>::new());
}

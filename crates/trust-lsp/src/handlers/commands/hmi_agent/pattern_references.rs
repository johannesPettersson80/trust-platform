//! The synthetic reference page of each composition pattern (specification 43, AT-02).
//!
//! Port of the structure of `agent/patternReferences.ts` (`patternReference`): the reference's
//! objects, pipes, its declared branch and its missing components. Every binding uses the explicit
//! `Example` namespace, so a reference is never project connectivity. The sample values and the
//! sample alarm of the reference are preview data for the shared drawing and stay in TypeScript
//! with the renderer (checklist H4.7).

use serde_json::{json, Value};
use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::page_files::HmiPage;

use super::edits::{object_from_json, page_from_json};
use super::error::{AgentError, AgentResult};
use super::geometry::whole_pixel;
use super::patterns::{pattern, PatternId};
use super::ports::port_point;

/// The design size of every reference, in pixels.
const REFERENCE_SIZE: [u32; 2] = [1280, 800];

/// A synthetic reference: the normal builder model with explicit example bindings.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PatternReference {
    /// Always `synthetic`: the reference is a design input, never project data.
    pub(super) origin: &'static str,
    /// The design size in pixels.
    pub(super) size: [u32; 2],
    /// The reference page, in the builder's own model.
    pub(super) model: HmiPage,
    /// Object types the catalogue does not define, in first-use order.
    pub(super) missing_components: Vec<String>,
}

/// The label, unit and scale of a value reading.
struct Scale {
    /// The label shown with the value.
    label: &'static str,
    /// The engineering unit.
    unit: &'static str,
    /// The top of the display range.
    maximum: i64,
}

/// Builds the reference of a pattern (`patternReference`).
///
/// # Errors
///
/// Returns `VALIDATION_FAILED` when the reference's own vessel does not place the outlet port
/// the reference connects to, and `INVALID_ARGUMENT` when an object cannot be read by the model.
pub(super) fn pattern_reference(
    pattern_id: PatternId,
    catalogue: &Catalogue,
) -> AgentResult<PatternReference> {
    let title = pattern(pattern_id).title;
    let mut builder = Builder::default();
    builder.add(
        "heading",
        "text",
        [32, 24],
        [1216, 40],
        json!({"variant": "heading", "text": title}),
    );
    builder.add(
        "example-note",
        "text",
        [32, 72],
        [1216, 28],
        json!({
            "variant": "caption",
            "text": "Synthetic design reference · Replace Example bindings with inspected project paths",
        }),
    );
    let pipes = match pattern_id {
        PatternId::ProcessOverview => process_overview(&mut builder)?,
        PatternId::EquipmentDetail => {
            equipment_detail(&mut builder);
            Vec::new()
        }
        PatternId::MultiUnit => {
            multi_unit(&mut builder);
            Vec::new()
        }
        PatternId::Investigation => {
            investigation(&mut builder);
            Vec::new()
        }
        PatternId::Settings => {
            settings(&mut builder);
            Vec::new()
        }
    };
    let model = page_from_json(&json!({
        "title": title,
        "object": builder.objects,
        "pipe": pipes,
    }))?;
    let missing_components = missing_types(&model, catalogue);
    Ok(PatternReference {
        origin: "synthetic",
        size: REFERENCE_SIZE,
        model,
        missing_components,
    })
}

/// The object types of the page that the catalogue does not define, in first-use order.
fn missing_types(model: &HmiPage, catalogue: &Catalogue) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for object in &model.objects {
        if !seen.contains(&object.kind) {
            seen.push(object.kind.clone());
        }
    }
    seen.into_iter()
        .filter(|kind| catalogue.component(kind).is_none())
        .collect()
}

/// A binding that reads an example path.
fn read(path: &str) -> Value {
    json!({"path": format!("Example.{path}"), "direction": "read"})
}

/// A binding that reads and writes an example path.
fn write(path: &str) -> Value {
    json!({"path": format!("Example.{path}"), "direction": "read-write"})
}

/// The objects of a reference, in the order the reference draws them.
#[derive(Default)]
struct Builder {
    /// The objects as their file writes them.
    objects: Vec<Value>,
}

impl Builder {
    /// Adds an object: its position and size, then its own properties.
    fn add(&mut self, id: &str, kind: &str, at: [i64; 2], size: [i64; 2], properties: Value) {
        let mut object = json!({
            "id": id,
            "type": kind,
            "at": at,
            "size": size,
            "rotation": 0,
            "bind": {},
        });
        if let (Some(fields), Some(extra)) = (object.as_object_mut(), properties.as_object()) {
            for (key, value) in extra {
                fields.insert(key.clone(), value.clone());
            }
        }
        self.objects.push(object);
    }

    /// Adds a reading of a process value, scaled and labelled, bound to an example path.
    fn reading(&mut self, id: &str, at: [i64; 2], width: i64, scale: Scale, path: &str) {
        self.add(
            id,
            "value",
            at,
            [width, 96],
            json!({
                "label": scale.label,
                "unit": scale.unit,
                "min": 0,
                "max": scale.maximum,
                "decimals": 1,
                "bind": {"value": read(path)},
            }),
        );
    }

    /// Adds the trend of the tank level and setpoint.
    fn trend(&mut self, at: [i64; 2], size: [i64; 2]) {
        self.add(
            "history",
            "trend",
            at,
            size,
            json!({
                "label": "Tank level and setpoint",
                "unit": "m",
                "min": 0,
                "max": 3,
                "span": 300,
                "bind": {"Level": read("Level"), "Setpoint": read("Setpoint")},
            }),
        );
    }
}

/// The process overview: a vessel, two pumps on a declared branch, three readings, a trend and an
/// alarm list. The branch runs through a junction on the vessel outlet's line, as the canvas
/// makes one when a pipe is dropped onto another.
fn process_overview(builder: &mut Builder) -> AgentResult<Vec<Value>> {
    builder.add(
        "process-area",
        "group",
        [32, 120],
        [792, 400],
        json!({"label": "Transfer line · declared example branch"}),
    );
    let vessel = json!({
        "id": "vessel",
        "type": "tank",
        "at": [88, 192],
        "size": [200, 260],
        "rotation": 0,
        "tag": "TK-101",
        "unit": "m",
        "min": 0,
        "max": 3,
        "bind": {"level": read("Level")},
    });
    let outlet = port_point(&object_from_json(&vessel)?, "outlet")
        .ok_or_else(|| AgentError::validation("the reference vessel has no outlet port."))?
        .at;
    builder.objects.push(vessel);
    for (index, y) in [184_i64, 360].into_iter().enumerate() {
        let number = index + 1;
        builder.add(
            &format!("pump-{index}"),
            "pump",
            [504, y],
            [160, 120],
            json!({
                "tag": format!("P-10{number}"),
                "bind": {
                    "running": read(&format!("Pump{index}.Running")),
                    "fault": read(&format!("Pump{index}.Fault")),
                },
            }),
        );
    }
    builder.add(
        "branch",
        "junction",
        [408, whole_pixel(outlet[1] - 8.0)],
        [16, 16],
        json!({}),
    );
    builder.reading(
        "level",
        [856, 120],
        392,
        Scale {
            label: "Tank level",
            unit: "m",
            maximum: 3,
        },
        "Level",
    );
    builder.reading(
        "flow",
        [856, 240],
        392,
        Scale {
            label: "Transfer flow",
            unit: "m³/h",
            maximum: 80,
        },
        "Flow",
    );
    builder.reading(
        "pressure",
        [856, 360],
        392,
        Scale {
            label: "Discharge pressure",
            unit: "bar",
            maximum: 6,
        },
        "Pressure",
    );
    builder.trend([32, 552], [600, 216]);
    builder.add("alarms", "alarm-list", [656, 552], [592, 216], json!({}));
    Ok(vec![
        json!({"from": "vessel.outlet", "to": "branch.left"}),
        json!({"from": "branch.top", "to": "pump-0.inlet"}),
        json!({"from": "branch.bottom", "to": "pump-1.inlet"}),
    ])
}

/// The equipment detail: one tank with its readings, a setpoint, a mode selector and a trend.
fn equipment_detail(builder: &mut Builder) {
    builder.add(
        "equipment-area",
        "group",
        [32, 120],
        [792, 400],
        json!({"label": "TK-101 · storage vessel"}),
    );
    builder.add(
        "vessel",
        "tank",
        [96, 184],
        [240, 312],
        json!({"tag": "TK-101", "unit": "m", "min": 0, "max": 3, "bind": {"level": read("Level")}}),
    );
    builder.reading(
        "level",
        [416, 184],
        368,
        Scale {
            label: "Process value",
            unit: "m",
            maximum: 3,
        },
        "Level",
    );
    builder.reading(
        "output",
        [416, 312],
        368,
        Scale {
            label: "Controller output",
            unit: "%",
            maximum: 100,
        },
        "Output",
    );
    builder.add(
        "setpoint",
        "setpoint",
        [856, 120],
        [392, 112],
        json!({
            "label": "Level setpoint",
            "unit": "m",
            "min": 0,
            "max": 3,
            "decimals": 1,
            "bind": {"value": write("Setpoint")},
        }),
    );
    builder.add(
        "mode",
        "mode-selector",
        [856, 264],
        [392, 64],
        json!({"options": ["Manual", "Auto"], "bind": {"mode": write("Mode")}}),
    );
    builder.add(
        "control-note",
        "text",
        [856, 368],
        [392, 96],
        json!({"text": "Named operator login required"}),
    );
    builder.trend([32, 552], [1216, 216]);
}

/// The repeated multi-unit overview: three equal cards, each with a pump and its flow.
fn multi_unit(builder: &mut Builder) {
    for index in 0..3_i64 {
        let x = 32 + index * 416;
        let prefix = format!("Unit{index}");
        let number = index + 1;
        builder.add(
            &format!("card-{index}"),
            "group",
            [x, 120],
            [384, 400],
            json!({"label": format!("P-10{number} · transfer pump")}),
        );
        builder.add(
            &format!("pump-{index}"),
            "pump",
            [x + 96, 184],
            [192, 144],
            json!({
                "tag": format!("P-10{number}"),
                "bind": {
                    "running": read(&format!("{prefix}.Running")),
                    "fault": read(&format!("{prefix}.Fault")),
                },
            }),
        );
        builder.reading(
            &format!("flow-{index}"),
            [x + 24, 384],
            336,
            Scale {
                label: "Outlet flow",
                unit: "m³/h",
                maximum: 80,
            },
            &format!("{prefix}.Flow"),
        );
    }
    builder.trend([32, 552], [1216, 216]);
}

/// The trends and alarms investigation: one trend above one alarm list.
fn investigation(builder: &mut Builder) {
    builder.trend([32, 120], [1216, 288]);
    builder.add("alarms", "alarm-list", [32, 440], [1216, 328], json!({}));
}

/// The settings and commissioning reference: two grouped areas with editable values.
fn settings(builder: &mut Builder) {
    builder.add(
        "settings-area",
        "group",
        [32, 120],
        [584, 648],
        json!({"label": "Process settings"}),
    );
    builder.add(
        "readback-area",
        "group",
        [648, 120],
        [600, 648],
        json!({"label": "Readback and response"}),
    );
    builder.add(
        "setpoint",
        "setpoint",
        [64, 184],
        [520, 112],
        json!({
            "label": "Level setpoint",
            "unit": "m",
            "min": 0.2,
            "max": 2.6,
            "decimals": 1,
            "bind": {"value": write("Setpoint")},
        }),
    );
    builder.add(
        "limit",
        "setpoint",
        [64, 336],
        [520, 112],
        json!({
            "label": "High level alarm",
            "unit": "m",
            "min": 2,
            "max": 3,
            "decimals": 1,
            "bind": {"value": write("HighLimit")},
        }),
    );
    builder.add(
        "mode",
        "mode-selector",
        [64, 488],
        [520, 64],
        json!({"options": ["Manual", "Auto"], "bind": {"mode": write("Mode")}}),
    );
    builder.add(
        "settings-note",
        "text",
        [64, 592],
        [520, 96],
        json!({"text": "Verify units and limits before editing."}),
    );
    builder.reading(
        "level",
        [680, 184],
        536,
        Scale {
            label: "Measured level",
            unit: "m",
            maximum: 3,
        },
        "Level",
    );
    builder.trend([680, 336], [536, 288]);
}

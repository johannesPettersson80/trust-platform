//! Fixtures that the discovery tests share with the tests of the modules split from it (specification 35, B1.2 to B1.4, C4): the tank loop
//! file and the loop it decodes to, and the three named methods of a PID, proportional or program's own law with their titles. Declared
//! under `cfg(test)` by `setup.rs`.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::loop_definition::LoopDefinition;

pub(super) const RUNTIME: &str = "runtime:local";

pub(super) const CONTROLLER: &str = "Main.TankController";

/// A loop file of one tank: the setpoint, the measurement and the output, each with its address, and no parameter record.
pub(super) fn loop_json(id: &str, project: &str, runtime: &str) -> Value {
    json!({
        "schemaVersion": 2, "id": id, "name": "Tank level", "revision": "sha256:loop-1",
        "projectId": project, "runtimeId": runtime,
        "controller": { "instance": CONTROLLER, "type": "Control.PID", "revision": "sha256:controller-1",
            "action": "direct", "taskPeriodMs": 10 },
        "signals": {
            "setpoint": { "symbol": "Main.SP", "address": "%ML0", "iecType": "LREAL", "direction": "read",
                "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "processVariable": { "symbol": "Main.PV", "address": "%IL0", "iecType": "LREAL", "direction": "read",
                "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "manipulatedVariable": { "symbol": "Main.MV", "address": "%QL0", "iecType": "LREAL", "direction": "write",
                "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 10 }
    })
}

pub(super) fn tank_json(id: &str) -> Value {
    loop_json(id, "project:sha256:abc", RUNTIME)
}

pub(super) fn loop_of(value: &Value) -> LoopDefinition {
    LoopDefinition::decode(&value.to_string()).expect("the fixture is a complete loop definition")
}

/// The three named methods that a PID, proportional or program's own law takes, in the order the page lists them (C4).
pub(super) fn simc_amigo_manual() -> Vec<crate::messages::TuneMethod> {
    use crate::messages::TuneMethod;
    vec![TuneMethod::Simc, TuneMethod::Amigo, TuneMethod::Manual]
}

/// The titles of the three named methods, keyed by the methods' words, as the engine gives them (C4).
pub(super) fn simc_amigo_manual_titles() -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "SIMC".to_owned(),
            "Simple internal model control (SIMC)".to_owned(),
        ),
        (
            "AMIGO".to_owned(),
            "Approximate M-constrained integral gain optimization (AMIGO)".to_owned(),
        ),
        ("manual".to_owned(), "Manual".to_owned()),
    ])
}

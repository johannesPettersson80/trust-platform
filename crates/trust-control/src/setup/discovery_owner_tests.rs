//! Discovery's check of a loop's setpoint and output against their owners (specification 35 B1.2; protocol annex E2.3). The check
//! of a parameter set's own symbols is in `discovery_tests.rs`.

use serde_json::json;

use super::{catalog, discover_bindings, reasons, signal, symbol, text_file};
use crate::setup::catalog::{CompiledSymbol, GlobalOwner};
use crate::setup::discovery_test_support::{tank_json, RUNTIME};

/// B1.2: what a loop's transfers and experiments write belongs to its task, as its gains do. The tank in CODESYS layout: the gain and
/// the mode are globals that nothing writes, the set runs in `MainTask`, and each case says who writes the setpoint and the output.
/// A setpoint that `SlowTask` writes and an output that no single task owns are refused, each by the sentence that names it; a setpoint
/// that `MainTask` writes and an output that nothing writes are discovered. Catches the setpoint or the output left out of the check,
/// so that setup offers a loop whose transfer is then refused.
#[test]
fn a_loop_whose_setpoint_or_output_is_a_global_its_task_does_not_own_is_refused_by_name() {
    let mut value = tank_json("loop:tank");
    value["signals"]["setpoint"]["symbol"] = json!("GVL_IO.lrSetpoint");
    value["signals"]["manipulatedVariable"]["symbol"] = json!("GVL_IO.lrValve");
    value["parameterBinding"] = json!({
        "loop_id": "loop:tank", "task_name": "MainTask",
        "parameters": [
            { "symbol": "GVL_Par.lrKp", "iec_type": "LREAL", "unit": "%/%", "convention": "proportional gain", "minimum": 0, "maximum": 100 }
        ],
        "mode": { "symbol": "GVL_IO.bAuto", "manual_value": false }
    });
    let files = [text_file("/project/control-loop.json", &value)];
    let discover = |setpoint: GlobalOwner, output: GlobalOwner| {
        let mut compiled = catalog();
        compiled.signals = vec![
            signal("GVL_IO.lrSetpoint", "%ML0", "LREAL"),
            signal("Main.PV", "%IL0", "LREAL"),
            signal("GVL_IO.lrValve", "%QL0", "LREAL"),
        ];
        compiled.control.symbols = [
            ("GVL_Par.lrKp", "LREAL", GlobalOwner::Unwritten),
            ("GVL_IO.bAuto", "BOOL", GlobalOwner::Unwritten),
            ("GVL_IO.lrSetpoint", "LREAL", setpoint),
            ("GVL_IO.lrValve", "LREAL", output),
        ]
        .map(|(name, iec_type, owner)| CompiledSymbol {
            owner: Some(owner),
            ..symbol(name, iec_type)
        })
        .to_vec();
        discover_bindings("/project", &compiled, RUNTIME, &files)
    };
    let task = |name: &str| GlobalOwner::Task {
        task: name.to_string(),
    };

    let written_by_slow = discover(task("SlowTask"), GlobalOwner::Unwritten);
    assert_eq!(
        reasons(&written_by_slow),
        ["GVL_IO.lrSetpoint is a global that task SlowTask writes, so the loop's task MainTask cannot write it. Run the loop in task \
          SlowTask, or let only the loop's task write GVL_IO.lrSetpoint."]
    );
    let shared_output = discover(task("MainTask"), GlobalOwner::Shared);
    assert_eq!(
        reasons(&shared_output),
        ["GVL_IO.lrValve is a global that no single task owns: the programs of several tasks write it, directly or through the \
          POUs they call, or a method that an interface's call may run writes it. Let only the loop's task write GVL_IO.lrValve."]
    );
    let discovered = discover(task("MainTask"), GlobalOwner::Unwritten);
    assert_eq!(reasons(&discovered), Vec::<&str>::new());
    assert_eq!(discovered.targets.len(), 1);
}

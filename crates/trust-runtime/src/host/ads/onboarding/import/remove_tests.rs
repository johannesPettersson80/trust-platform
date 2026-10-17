use trust_ads_core::{
    AdsDataTypeDescriptor, IecDataType, SymbolDescriptor, SymbolFlag, SymbolSnapshot,
};

use super::*;

/// The literal `ads.toml` of the VS Code test `ads-tag-config-mutation.test.ts` (lines 10-50).
const ADS_TOML_BEFORE: &str = r#"[[connections]]
name = "line_301"
target_net_id = "100.67.6.217.1.1"
host = "192.168.77.11"
ams_port = 301
local_net_id = "192.168.77.10.1.1"

[[connections.points]]
var = "var_1"
symbol = "Task 4.Inputs.Var 1"
type = "USINT"

[[connections.points]]
var = "var_2"
symbol = "Task 4.Inputs.Var 2"
type = "USINT"

[[connections]]
name = "duplicate_301"
target_net_id = "100.67.6.217.1.1"
host = "192.168.77.11"
ams_port = 301

[[connections.points]]
var = "duplicate_var_1"
symbol = "Task 4.Inputs.Var 1"
type = "USINT"

[[connections]]
name = "line_851"
target_net_id = "100.67.6.217.1.1"
host = "192.168.77.11"
ams_port = 851

[[connections.points]]
var = "other_port_var_1"
symbol = "Task 4.Inputs.Var 1"
type = "USINT"
"#;

#[test]
fn ads_tag_remove_rewrites_ads_toml_and_regenerates_st() {
    // VS Code test `unchecking removes the tag from matching ADS connections only`
    // (ads-tag-config-mutation.test.ts, lines 9-63) with its literal TOML (lines 10-50): only the
    // port-301 connections lose `Task 4.Inputs.Var 1`; line_851 and its point stay.
    let device = AdsTagDevice {
        host: Some("192.168.77.11".to_string()),
        target_net_id: Some("100.67.6.217.1.1".to_string()),
        ams_port: 301,
    };
    let paths = vec!["Task 4.Inputs.Var 1".to_string()];

    let removal = remove_symbol_imports(ADS_TOML_BEFORE, &device, &paths);

    assert_eq!(removal.removed_count, 2);
    assert!(removal.text.contains("symbol = \"Task 4.Inputs.Var 2\""));
    assert!(removal.text.contains("name = \"line_851\""));
    assert!(removal.text.contains("var = \"other_port_var_1\""));
    assert!(!removal.text.contains("var = \"var_1\"\n"));
    assert!(!removal.text.contains("name = \"duplicate_301\""));

    // VS Code test `tag removal regenerates ADS ST from the updated config and all snapshots`
    // (lines 65-91): the generated ST comes from the rewritten config and every snapshot.
    let snapshots = vec![
        SymbolSnapshot::new("line_301", vec![usint_symbol("Task 4.Inputs.Var 2")]),
        SymbolSnapshot::new("line_851", vec![usint_symbol("Task 4.Inputs.Var 1")]),
    ];
    let generated = regenerate_ads_interface(removal.text.as_str(), &snapshots)
        .expect("regenerate ADS ST from the updated config");

    assert!(generated.contains("var_2 : USINT;"), "{generated}");
    assert!(
        generated.contains("other_port_var_1 : USINT;"),
        "{generated}"
    );
    assert!(
        !generated
            .lines()
            .any(|line| line.trim_start() == "var_1 : USINT;"),
        "{generated}"
    );
}

#[test]
fn removing_the_last_point_removes_the_connection() {
    // A connection whose only point goes away must not survive as an empty `[[connections]]`
    // block; a wrong implementation that keeps the header would leave a connection `parse_ads_toml`
    // rejects.
    let text = concat!(
        "[[connections]]\n",
        "name = \"line_301\"\n",
        "target_net_id = \"100.67.6.217.1.1\"\n",
        "host = \"192.168.77.11\"\n",
        "ams_port = 301\n",
        "\n",
        "[[connections.points]]\n",
        "var = \"var_1\"\n",
        "symbol = \"Task 4.Inputs.Var 1\"\n",
        "type = \"USINT\"\n",
    );
    let device = AdsTagDevice {
        host: Some("192.168.77.11".to_string()),
        target_net_id: Some("100.67.6.217.1.1".to_string()),
        ams_port: 301,
    };

    let removal = remove_symbol_imports(text, &device, &["Task 4.Inputs.Var 1".to_string()]);

    assert_eq!(removal.removed_count, 1);
    assert!(!removal.text.contains("name = \"line_301\""));
    assert!(!removal.text.contains("[[connections]]"));
}

fn usint_symbol(name: &str) -> SymbolDescriptor {
    SymbolDescriptor::new(
        name,
        AdsDataTypeDescriptor::scalar("USINT", IecDataType::Usint),
        0x4020,
        0,
        1,
    )
    .with_flag(SymbolFlag::Read)
}

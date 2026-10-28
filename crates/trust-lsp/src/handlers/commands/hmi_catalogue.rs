//! The HMI Builder's component catalogue for the editor (specification 39, §5.2, §5.6, §17.3):
//! the same definition the binding checks use, with the project's own symbols (§14.4).

use serde::Deserialize;
use serde_json::{json, Value};
use tower_lsp::lsp_types::Url;
use trust_runtime::hmi::catalogue::{Catalogue, CATALOGUE_JSON};
use trust_runtime::hmi::my_symbols::symbols_in;

use super::hmi_files::{refused, single_argument};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogueArgs {
    root_uri: Url,
    /// Unsaved HMI text, read in memory only, keyed by project-relative path.
    #[serde(default)]
    open: std::collections::BTreeMap<String, String>,
}

/// `trust-lsp.hmiCatalogue`: no arguments, to `{ ok, catalogue }`; or `{ root_uri }`, to the
/// catalogue with the project's symbols and `symbols`, the symbols themselves.
pub(super) fn catalogue_value(args: Vec<Value>) -> Value {
    if args.is_empty() {
        return match serde_json::from_str::<Value>(CATALOGUE_JSON) {
            Ok(catalogue) => json!({ "ok": true, "catalogue": catalogue }),
            Err(error) => refused(format!("the component catalogue does not load: {error}")),
        };
    }
    let args: CatalogueArgs = match single_argument(args, super::HMI_CATALOGUE_COMMAND) {
        Ok(args) => args,
        Err(message) => return refused(message),
    };
    let Ok(project) = args.root_uri.to_file_path() else {
        return refused(format!(
            "{} needs a file root_uri",
            super::HMI_CATALOGUE_COMMAND
        ));
    };
    let snapshot = match super::hmi_snapshot::paths(&project, &args.open) {
        Ok(snapshot) => snapshot,
        Err(error) => return refused(error),
    };
    let bundle = match super::hmi_authoring::bundle::read_edited(&project, &snapshot) {
        Ok(bundle) => bundle,
        Err(error) => return refused(error.to_string()),
    };
    let (symbols, symbol_errors) = symbols_in(&bundle.files);
    let catalogue = Catalogue::shipped().with_symbols(&symbols);
    let formats = trust_runtime::hmi::presentation::value_formats(&bundle, &catalogue);
    let (equipment, component_errors) =
        super::hmi_authoring::components::equipment_definitions(&bundle, &catalogue);
    json!({ "ok": true, "catalogue": catalogue, "symbols": symbols, "formats": formats,
        "equipment": equipment, "componentErrors": component_errors, "symbolErrors": symbol_errors })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_project_catalogue_gives_shared_formats_and_unsaved_text_wins() {
        let root = std::env::temp_dir().join(format!(
            "hmi-formats-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("hmi/pages")).unwrap();
        std::fs::write(
            root.join("hmi/screen.toml"),
            "format = 1\nsize = [1280, 720]\n",
        )
        .unwrap();
        let page = "format = 1\ntitle = \"Units\"\n[[object]]\nid = \"speed\"\ntype = \"value\"\nat = [0, 0]\nsize = [160, 80]\nunit = \"rpm\"\ndecimals = 1\n[object.bind]\nvalue = \"Main.Speed\"\n";
        std::fs::write(root.join("hmi/pages/source.toml"), page).unwrap();
        let uri = Url::from_directory_path(&root).unwrap();
        let saved = catalogue_value(vec![json!({ "root_uri": uri })]);
        assert_eq!(
            saved["formats"]["main.speed"],
            json!({ "unit": "rpm", "decimals": 1 })
        );
        let open = catalogue_value(vec![
            json!({ "root_uri": uri, "open": { "hmi/pages/source.toml": page.replace("rpm", "Hz").replace("decimals = 1", "decimals = 2") } }),
        ]);
        assert_eq!(
            open["formats"]["main.speed"],
            json!({ "unit": "Hz", "decimals": 2 })
        );
        assert_eq!(
            std::fs::read_to_string(root.join("hmi/pages/source.toml")).unwrap(),
            page
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn library_and_project_symbols_use_unsaved_drawings_without_writing_files() {
        let root = std::env::temp_dir().join(format!(
            "hmi-customer-symbol-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let library = root.as_path().join("libraries/customer");
        std::fs::create_dir_all(library.join("src")).unwrap();
        std::fs::create_dir_all(library.join("hmi/symbols")).unwrap();
        std::fs::create_dir_all(root.as_path().join("src")).unwrap();
        std::fs::create_dir_all(root.as_path().join("hmi/symbols")).unwrap();
        std::fs::write(
            root.as_path().join("trust-lsp.toml"),
            "[dependencies]\nCustomer = { path = 'libraries/customer' }\n",
        )
        .unwrap();
        let definition = |title: &str| format!("format = 1\ntitle = '{title}'\nsize = [80, 80]\n");
        let saved = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 80"><circle cx="40" cy="40" r="30"/></svg>"#;
        let edited = saved.replace("r=\"30\"", "r=\"20\"");
        // A company library ships its symbol, as it ships its panels and pairs (§14.4 item 1).
        std::fs::write(
            library.join("hmi/symbols/rotor.toml"),
            definition("Customer rotor"),
        )
        .unwrap();
        std::fs::write(library.join("hmi/symbols/rotor.svg"), saved).unwrap();
        // A project's own symbol of the same name takes the library's place.
        std::fs::write(
            library.join("hmi/symbols/stator.toml"),
            definition("Shipped stator"),
        )
        .unwrap();
        std::fs::write(library.join("hmi/symbols/stator.svg"), saved).unwrap();
        std::fs::write(
            root.as_path().join("hmi/symbols/stator.toml"),
            definition("Own stator"),
        )
        .unwrap();
        std::fs::write(root.as_path().join("hmi/symbols/stator.svg"), saved).unwrap();
        let uri = Url::from_directory_path(root.as_path()).unwrap();
        let answer = catalogue_value(vec![json!({"root_uri":uri,
            "open":{"libraries/customer/hmi/symbols/rotor.svg":edited}})]);
        assert_eq!(answer["ok"], true, "{answer}");
        assert_eq!(answer["symbolErrors"], json!([]), "{answer}");
        let symbols = answer["symbols"].as_array().unwrap();
        let named = |name: &str| symbols.iter().find(|symbol| symbol["name"] == name);
        let rotor = named("rotor").expect("the library's symbol");
        assert_eq!(
            rotor["svg"]["children"][0]["attrs"]["r"], "20",
            "its unsaved drawing"
        );
        assert_eq!(
            named("stator").expect("the stator")["title"],
            "Own stator",
            "{answer}"
        );
        assert_eq!(
            std::fs::read_to_string(library.join("hmi/symbols/rotor.svg")).unwrap(),
            saved
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_catalogue_command_returns_the_runtime_catalogue() {
        let answer = catalogue_value(Vec::new());
        assert_eq!(answer["ok"], json!(true), "{answer}");
        let expected: Value = serde_json::from_str(CATALOGUE_JSON).expect("the catalogue is JSON");
        assert_eq!(answer["catalogue"], expected);
        let value = answer["catalogue"]["components"]
            .as_array()
            .and_then(|components| {
                components
                    .iter()
                    .find(|component| component["type"] == "value")
            })
            .expect("the value component");
        assert!(
            value["size"].as_array().is_some_and(|size| size.len() == 2),
            "{value}"
        );
    }

    #[test]
    fn the_catalogue_command_takes_no_arguments() {
        let answer = catalogue_value(vec![json!({ "kind": "page" })]);
        assert_eq!(answer["ok"], json!(false), "{answer}");
    }
}

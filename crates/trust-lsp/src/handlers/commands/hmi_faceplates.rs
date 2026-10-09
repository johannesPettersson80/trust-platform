//! The faceplates a project can open (specification 39, §8.7 item 3): its own under
//! `hmi/panels/`, then its libraries' not of a name it already has, for the editor's click
//! targets and its field picker.

use super::hmi_authoring::documents;
use serde::Deserialize;
use serde_json::{json, Value};
use tower_lsp::lsp_types::Url;

use super::hmi_files::{refused, single_argument};
use super::HMI_FACEPLATES_COMMAND;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FaceplatesArgs {
    root_uri: Url,
    #[serde(default)]
    open: std::collections::BTreeMap<String, String>,
}

/// `trust-lsp.hmiFaceplates`: `{ root_uri }` to `{ ok, faceplates: [{ name, title, size, fields,
/// library, uri }] }`, sorted by name; a faceplate that does not load is listed with its error.
pub(super) fn faceplates_value(args: Vec<Value>) -> Value {
    let args: FaceplatesArgs = match single_argument(args, HMI_FACEPLATES_COMMAND) {
        Ok(args) => args,
        Err(message) => return refused(message),
    };
    let Ok(root) = args.root_uri.to_file_path() else {
        return refused(format!(
            "{HMI_FACEPLATES_COMMAND} takes the address of a project folder, not {}",
            args.root_uri
        ));
    };
    let snapshot = match super::hmi_snapshot::paths(&root, &args.open) {
        Ok(snapshot) => snapshot,
        Err(error) => return refused(error),
    };
    let libraries = match super::hmi_authoring::sources::dependency_roots(&root, &snapshot) {
        Ok(roots) => roots,
        Err(error) => return refused(error),
    };
    let mut found: std::collections::BTreeMap<String, Value> = std::collections::BTreeMap::new();
    for (folder, library) in
        std::iter::once((root.clone(), None)).chain(libraries.into_iter().map(|path| {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned());
            (path, name)
        }))
    {
        for owner in [folder.join("builder"), folder.clone()] {
            for definition_folder in ["panels"] {
                let directory = owner.join("hmi").join(definition_folder);
                let mut paths = std::collections::BTreeSet::new();
                match std::fs::read_dir(&directory) {
                    Ok(entries) => {
                        for entry in entries {
                            let entry = match entry {
                                Ok(entry) => entry,
                                Err(error) => return refused(error.to_string()),
                            };
                            if entry.file_type().is_ok_and(|kind| kind.is_file()) {
                                paths.insert(entry.path());
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return refused(error.to_string()),
                }
                paths.extend(
                    snapshot
                        .keys()
                        .filter(|path| path.parent() == Some(directory.as_path()))
                        .cloned(),
                );
                for path in paths {
                    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                        continue;
                    };
                    let Some(name) = file_name.strip_suffix(".toml").map(str::to_string) else {
                        continue;
                    };
                    if found.contains_key(&name) {
                        continue;
                    }
                    let file = format!("hmi/{definition_folder}/{file_name}");
                    // Its address, for the editor to open it (specification 39, §3.3 item 8).
                    let uri = Url::from_file_path(&path).ok().map(|uri| uri.to_string());
                    let entry_value = match snapshot
                        .get(&path)
                        .cloned()
                        .map(Ok)
                        .unwrap_or_else(|| std::fs::read_to_string(&path))
                        .map_err(|error| error.to_string())
                        .and_then(|text| {
                            documents::parse("faceplate", &file, &text).map_err(|errors| {
                                errors
                                    .first()
                                    .map(|error| format!("{}: {}", error.line, error.message))
                                    .unwrap_or_default()
                            })
                        }) {
                        Ok(faceplate) => {
                            json!({ "name": name, "title": faceplate["title"], "size": faceplate["size"], "requires_instance": documents::requires_instance(&faceplate), "fields": faceplate["fields"], "library": library, "uri": uri })
                        }
                        Err(error) => {
                            json!({ "name": name, "error": error, "library": library, "uri": uri })
                        }
                    };
                    found.insert(name, entry_value);
                }
            }
        }
    }
    let bundle = match super::hmi_placed::preview_bundle(&root, args.open.clone()) {
        Ok(bundle) => bundle,
        Err(error) => return refused(format!("the HMI definitions cannot be read: {error}")),
    };
    for (file, text) in &args.open {
        let Some(name) = file
            .strip_prefix("hmi/panels/")
            .or_else(|| file.strip_prefix("hmi/panels/"))
            .and_then(|name| name.strip_suffix(".toml"))
        else {
            continue;
        };
        if name.contains('/') || name.contains('\\') {
            continue;
        }
        let value = match documents::parse("faceplate", file, text) {
            Ok(faceplate) => {
                json!({"name":name,"title":faceplate["title"],"size":faceplate["size"],"requires_instance":documents::requires_instance(&faceplate),"fields":faceplate["fields"],"library":null,"uri":Url::from_file_path(root.join(file)).ok(),"dirty":true})
            }
            Err(errors) => {
                json!({"name":name,"error":errors.first().map(|error|error.message.clone()),"dirty":true})
            }
        };
        found.insert(name.to_string(), value);
    }
    for (name, entry) in &mut found {
        entry["pairs"] = json!(super::hmi_authoring::bundle::pairs_of(&bundle, name));
    }
    json!({ "ok": true, "faceplates": found.into_values().collect::<Vec<_>>() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_projects_faceplates_come_first_then_its_libraries_not_of_a_name_it_has() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "trustlsp-hmi-faceplates-{}-{stamp}",
            std::process::id()
        ));
        let write = |path: &str, text: &str| {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().expect("a folder")).expect("folders");
            std::fs::write(file, text).expect("write");
        };
        let faceplate = |title: &str| {
            format!("format = 1\ntitle = \"{title}\"\nsize = [400, 300]\n\n[fields.Running]\naccepts = [\"boolean\"]\n")
        };
        write(
            "trust-lsp.toml",
            "[dependencies]\nequipment = { path = \"lib/equipment\" }\n",
        );
        write(
            "lib/equipment/trust-lsp.toml",
            "[package]\nversion = \"0.1.0\"\n",
        );
        write("lib/equipment/src/equipment.st", "(* sources *)\n");
        write(
            "lib/equipment/hmi/panels/pump.toml",
            &faceplate("Shipped pump"),
        );
        write(
            "lib/equipment/hmi/panels/valve.toml",
            &faceplate("Shipped valve"),
        );
        write("hmi/panels/pump.toml", &faceplate("Own pump"));
        let answer = faceplates_value(vec![
            json!({ "root_uri": Url::from_directory_path(&root).expect("uri") }),
        ]);
        assert_eq!(answer["ok"], json!(true), "{answer}");
        let listed: Vec<(Value, Value, Value)> = answer["faceplates"]
            .as_array()
            .expect("faceplates")
            .iter()
            .map(|entry| {
                (
                    entry["name"].clone(),
                    entry["title"].clone(),
                    entry["library"].clone(),
                )
            })
            .collect();
        assert_eq!(
            listed,
            vec![
                (json!("pump"), json!("Own pump"), Value::Null),
                (json!("valve"), json!("Shipped valve"), json!("equipment"))
            ]
        );
        assert_eq!(
            answer["faceplates"][0]["fields"]["Running"]["accepts"],
            json!(["boolean"])
        );
        // Each names its file, so the editor can open it (specification 39, §3.3 item 8).
        let uri = |index: usize| {
            answer["faceplates"][index]["uri"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        };
        assert!(
            uri(0).ends_with("/hmi/panels/pump.toml") && !uri(0).contains("/lib/"),
            "{}",
            uri(0)
        );
        assert!(
            uri(1).ends_with("/lib/equipment/hmi/panels/valve.toml"),
            "{}",
            uri(1)
        );
        let overlaid = faceplates_value(vec![json!({
            "root_uri": Url::from_directory_path(&root).expect("uri"),
            "open": {"hmi/panels/pump.toml": faceplate("Unsaved pump")}
        })]);
        assert_eq!(overlaid["faceplates"][0]["title"], json!("Unsaved pump"));
        assert_eq!(overlaid["faceplates"][0]["dirty"], json!(true));
        assert!(overlaid["faceplates"][0]["pairs"].is_array());
        std::fs::remove_dir_all(root).ok();
    }
}

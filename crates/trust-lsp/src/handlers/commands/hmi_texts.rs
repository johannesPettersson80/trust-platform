//! The project's texts for the HMI Builder editor (specification 39, §14.5 items 7 and 8): the
//! runtime reads them, so the editor draws with the same lookup as the runtime display.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{json, Value};
use tower_lsp::lsp_types::Url;
use trust_runtime::hmi::texts::{language_name, read_project_texts, BUILT_IN_LANGUAGES};

use super::hmi_files::{refused, single_argument};
use super::HMI_TEXTS_COMMAND;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextsArgs {
    root_uri: Url,
    /// The text of files the editor holds unsaved, by project-relative path.
    #[serde(default)]
    open: BTreeMap<String, String>,
}

/// `trust-lsp.hmiTexts`: `{ root_uri, open? }` to the project's texts, and `shipped`: the
/// languages truST ships its words in, each with its own name, for the language choice (§14.5
/// item 2).
pub(super) fn texts_value(args: Vec<Value>) -> Value {
    let args: TextsArgs = match single_argument(args, HMI_TEXTS_COMMAND) {
        Ok(args) => args,
        Err(message) => return refused(message),
    };
    let Ok(root) = args.root_uri.to_file_path() else {
        return refused(format!(
            "{HMI_TEXTS_COMMAND} takes the address of a project folder, not {}",
            args.root_uri
        ));
    };
    let texts = read_project_texts(&root, &args.open);
    match serde_json::to_value(texts) {
        Ok(Value::Object(mut answer)) => {
            answer.insert("ok".to_string(), json!(true));
            let shipped: Vec<Value> = BUILT_IN_LANGUAGES
                .iter()
                .map(|tag| json!({ "tag": tag, "name": language_name(tag) }))
                .collect();
            answer.insert("shipped".to_string(), Value::Array(shipped));
            Value::Object(answer)
        }
        _ => refused("the project's texts could not be given".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn project() -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "trust-lsp-hmi-texts-{}-{stamp}",
            std::process::id()
        ));
        let write = |file: &str, text: &str| {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().expect("a folder")).expect("a folder");
            std::fs::write(path, text).expect("written");
        };
        write(
            "hmi/screen.toml",
            "format = 1\nsize = [1280, 720]\ntheme = \"dark\"\nsymbols = \"flat\"\nlanguages = [\"en\", \"sv\"]\n\n[navigation]\nstyle = \"side\"\nposition = \"left\"\n",
        );
        write(
            "hmi/pages/overview.toml",
            "format = 1\ntitle = \"Overview\"\n\n[[object]]\nid = \"heading\"\ntype = \"text\"\nat = [40, 40]\nsize = [240, 40]\ntext = \"Pumps\"\n",
        );
        write(
            "hmi/texts/sv.toml",
            "format = 1\nlanguage = \"sv\"\n\n[texts]\n\"Pumps\" = \"Pumpar\"\n",
        );
        root
    }

    #[test]
    fn the_texts_command_gives_the_project_texts_as_the_runtime_reads_them() {
        let root = project();
        let address = Url::from_file_path(&root).expect("a file address");
        let answer = texts_value(vec![json!({ "root_uri": address })]);
        assert_eq!(answer["ok"], json!(true), "{answer}");
        assert_eq!(
            answer["languages"],
            json!([{ "tag": "en", "name": "English" }, { "tag": "sv", "name": "Svenska" }])
        );
        assert_eq!(
            answer["texts"],
            json!([
                { "text": "Overview", "places": ["overview"] },
                { "text": "Pumps", "places": ["overview"] }
            ])
        );
        assert_eq!(answer["project"]["sv"]["Pumps"], json!("Pumpar"));
        assert_eq!(answer["builtin"]["sv"]["No alarms"], json!("Inga larm"));
        let shipped = answer["shipped"].as_array().expect("the shipped languages");
        assert_eq!(shipped.len(), 11, "{answer}");
        assert_eq!(shipped[0], json!({ "tag": "en", "name": "English" }));
        assert!(
            shipped.contains(&json!({ "tag": "ja", "name": "日本語" })),
            "{answer}"
        );
        assert_eq!(answer["errors"], json!([]));
        // The editor's unsaved text wins over the saved file.
        let open = json!({ "hmi/texts/sv.toml": "format = 1\nlanguage = \"sv\"\n\n[texts]\n\"Pumps\" = \"Matarpumpar\"\n" });
        let answer = texts_value(vec![json!({ "root_uri": address, "open": open })]);
        assert_eq!(
            answer["project"]["sv"]["Pumps"],
            json!("Matarpumpar"),
            "{answer}"
        );
    }

    #[test]
    fn the_texts_command_takes_a_project_folder() {
        let answer = texts_value(vec![json!({ "root_uri": "https://example.com/project" })]);
        assert_eq!(answer["ok"], json!(false), "{answer}");
        assert_eq!(
            answer["errors"][0]["message"],
            json!("trust-lsp.hmiTexts takes the address of a project folder, not https://example.com/project")
        );
    }
}

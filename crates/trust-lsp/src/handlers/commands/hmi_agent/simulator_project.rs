//! A project for the tests of the simulator journey (specification 43, AT-08): a temporary folder
//! with a compiling program and the settings page, a draft composed as a copy of that page through
//! the authoring command, and the fields of a journey's argument that name them.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};
use tower_lsp::lsp_types::Url;

use crate::state::ServerState;

use super::native;
use super::service::{execute, COMPOSE};

/// The workspace the tests connect.
pub(super) const WORKSPACE: &str = "workspace-journey";
/// The settings page's file.
pub(super) const SETTINGS_FILE: &str = "hmi/pages/settings.toml";
/// The settings page: one setpoint, which the journeys' steps write.
pub(super) const SETTINGS: &str = "format = 1\ntitle = \"Settings\"\n\n[[object]]\nid = \"sp\"\ntype = \"setpoint\"\nat = [32, 88]\nsize = [200, 64]\n";

/// A project folder under the system temporary folder, removed when the test ends.
pub(super) struct JourneyProject(PathBuf);

impl JourneyProject {
    /// A project with a compiling program, a 1280 × 800 screen and the settings page.
    pub(super) fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "trust-hmi-journey-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&path);
        let project = Self(path);
        project.write(
            "src/main.st",
            "PROGRAM Main\nVAR Level : REAL; END_VAR\nEND_PROGRAM\n",
        );
        project.write(
            "hmi/screen.toml",
            "format = 1\nsize = [1280, 800]\ntheme = \"dark\"\nsymbols = \"flat\"\n",
        );
        project.write(SETTINGS_FILE, SETTINGS);
        project
    }

    /// Writes a file of the project, creating its folders.
    pub(super) fn write(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().expect("a folder")).expect("create the folder");
        fs::write(path, text).expect("write the file");
    }

    /// The project folder's address, which is also the workspace of its simulator sessions.
    pub(super) fn root_uri(&self) -> String {
        Url::from_directory_path(&self.0)
            .expect("a folder address")
            .to_string()
    }

    /// The fields of a journey's argument that the authoring commands share: the workspace, the
    /// project folder and its draft storage.
    pub(super) fn fields(&self) -> Value {
        json!({
            "workspaceId": WORKSPACE,
            "root_uri": self.root_uri(),
            "storage": self.0.join("private").display().to_string(),
        })
    }

    /// Composes a draft that copies the settings page under `key`: its ID, at revision `1`.
    pub(super) fn settings_draft(&self, state: &ServerState, key: &str) -> String {
        let mut argument = self.fields();
        argument["request"] = json!({
            "contractVersion": 1,
            "workspaceId": WORKSPACE,
            "input": { "key": key, "file": SETTINGS_FILE, "sourceFile": SETTINGS_FILE },
        });
        let answer = execute(state, COMPOSE, vec![argument]).expect("an authoring command");
        answer["data"]["draftId"]
            .as_str()
            .unwrap_or_else(|| panic!("a draft: {answer}"))
            .to_owned()
    }
}

impl Drop for JourneyProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The settings page's model, as the simulator serves it once the page is deployed.
pub(super) fn settings_model(state: &ServerState) -> Value {
    native::parse_model(state, "page", SETTINGS, SETTINGS_FILE).expect("the settings page")
}

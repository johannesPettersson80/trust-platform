//! The project list, the fleet root of the chosen project (decision O15) and the stamps that
//! decide when the project's files are read again (specification 25 §5.11).

use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::ProjectCache;
use crate::request::GatherMode;
use crate::test_project::{fleet_toml, runtime_toml, TempFolder};

const SOCKET: &str = "unix:///tmp/trust-devices-project.sock";

fn set_modified(path: &Path, time: SystemTime) {
    File::options()
        .write(true)
        .open(path)
        .and_then(|file| file.set_modified(time))
        .expect("set the modification time");
}

#[test]
fn project_list_reads_root_and_direct_children() {
    let first = TempFolder::new("projects-first");
    first.write("runtime.toml", &runtime_toml("first", SOCKET, None));
    first.write("a/runtime.toml", &runtime_toml("a", SOCKET, None));
    first.write("b/notes.txt", "a folder without a project");
    first.write("c/d/runtime.toml", &runtime_toml("d", SOCKET, None));
    first.write("e/runtime.toml/notes.txt", "a folder named runtime.toml");
    let second = TempFolder::new("projects-second");
    second.write("x/runtime.toml", &runtime_toml("x", SOCKET, None));
    // A workspace folder inside another one lists its project once.
    let folders = [first.path.clone(), second.path.clone(), first.join("a")];

    let inputs = ProjectCache::default().read(&folders, None, GatherMode::Auto);

    assert_eq!(
        inputs.projects,
        [first.path.clone(), first.join("a"), second.join("x")]
    );
    assert_eq!(inputs.chosen.as_deref(), Some(first.path.as_path()));
    assert!(matches!(inputs.topology, Some(Ok(_))));
    assert!(inputs.catalog.is_ok());

    let empty = TempFolder::new("projects-empty");
    let inputs =
        ProjectCache::default().read(std::slice::from_ref(&empty.path), None, GatherMode::Auto);
    assert_eq!(inputs.projects, Vec::<std::path::PathBuf>::new());
    assert!(inputs.chosen.is_none());
    assert!(inputs.topology.is_none());
    assert!(matches!(inputs.managed.as_deref(), Ok([])));
    assert!(inputs.catalog.is_ok(), "the catalog needs no project");
}

#[test]
fn project_files_reread_only_after_a_change() {
    let project = TempFolder::new("project-stamps");
    let runtime = project.write("runtime.toml", &runtime_toml("cell_a", SOCKET, None));
    let folders = [project.path.clone()];
    let mut cache = ProjectCache::default();
    let mut runtime_name = |mode: GatherMode| {
        let inputs = cache.read(&folders, None, mode);
        let topology = inputs
            .topology
            .expect("a chosen project")
            .expect("a configured topology");
        topology["hosts"][0]["runtimes"][0]["name"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(runtime_name(GatherMode::Auto), "cell_a");

    // The same length and modification time: the file counts as unchanged and is not read again.
    let modified = fs::metadata(&runtime)
        .and_then(|metadata| metadata.modified())
        .expect("a modification time");
    fs::write(&runtime, runtime_toml("cell_b", SOCKET, None)).expect("rewrite runtime.toml");
    set_modified(&runtime, modified);
    assert_eq!(runtime_name(GatherMode::Auto), "cell_a");
    assert_eq!(runtime_name(GatherMode::Cached), "cell_a");

    // A new modification time is a change.
    let later = modified + Duration::from_secs(5);
    set_modified(&runtime, later);
    assert_eq!(runtime_name(GatherMode::Auto), "cell_b");

    // `now` reads again without a change.
    fs::write(&runtime, runtime_toml("cell_c", SOCKET, None)).expect("rewrite runtime.toml");
    set_modified(&runtime, later);
    assert_eq!(runtime_name(GatherMode::Auto), "cell_b");
    assert_eq!(runtime_name(GatherMode::Now), "cell_c");

    // A `fleet.toml` that appears is a change too.
    project.write(
        "fleet.toml",
        &fleet_toml(&[("cell", "tcp://127.0.0.1:19940", 19_941)]),
    );
    let inputs = cache.read(&folders, None, GatherMode::Auto);
    let managed = inputs.managed.expect("a valid fleet.toml");
    assert_eq!(managed.len(), 1);
    assert_eq!(managed[0].name, "cell");
}

#[test]
fn chosen_project_is_the_fleet_root() {
    // A project with its own runtime and a fleet: the runtime `cell` in its subfolder.
    let plant = TempFolder::new("fleet-root");
    plant.write("runtime.toml", &runtime_toml("line", SOCKET, None));
    plant.write(
        "fleet.toml",
        &fleet_toml(&[("cell", "tcp://127.0.0.1:19950", 19_951)]),
    );
    plant.write(
        "cell/runtime.toml",
        &runtime_toml("cell", "tcp://127.0.0.1:19950", Some("cell-token")),
    );
    let folders = [plant.path.clone()];
    let fleet_of = |requested: Option<&Path>| {
        let inputs = ProjectCache::default().read(&folders, requested, GatherMode::Auto);
        let managed = inputs
            .managed
            .expect("a valid fleet.toml")
            .into_iter()
            .map(|runtime| runtime.name)
            .collect::<Vec<_>>();
        (inputs.chosen, managed)
    };

    // Until a project is chosen the first is shown, and its fleet.toml lists the runtimes that
    // this computer manages.
    assert_eq!(
        fleet_of(None),
        (Some(plant.path.clone()), vec!["cell".to_string()])
    );
    // The managed runtime's folder is a project of its own; chosen, it is a fleet of one.
    let cell = plant.join("cell");
    assert_eq!(
        fleet_of(Some(cell.as_path())),
        (Some(cell.clone()), Vec::<String>::new())
    );
    // A chosen project that is no longer listed falls back to the first.
    assert_eq!(
        fleet_of(Some(Path::new("/nowhere/else"))).0,
        Some(plant.path.clone())
    );

    // A fleet.toml in a workspace folder that is no project is not read: the fleet root is
    // always the chosen project.
    let parent = TempFolder::new("fleet-parent");
    parent.write(
        "fleet.toml",
        &fleet_toml(&[("x", "tcp://127.0.0.1:19960", 19_961)]),
    );
    parent.write("x/runtime.toml", &runtime_toml("x", SOCKET, None));
    let inputs =
        ProjectCache::default().read(std::slice::from_ref(&parent.path), None, GatherMode::Auto);
    assert_eq!(inputs.chosen, Some(parent.join("x")));
    assert!(matches!(inputs.managed.as_deref(), Ok([])));
    assert_eq!(inputs.projects, [parent.join("x")]);
}

//! The variables a project's services may write (specification 48 RU-63; specification 44 §5.14):
//! a Structured Text project without a crate whose `trust.toml` gives its services `writes` builds
//! an image whose service table lists each service's paths, and the build refuses a path that no
//! service's write can reach, naming the service and the path.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod rust_pous_support;

use std::path::{Path, PathBuf};

use rust_pous_support::sources;
use trust_runtime::native::rust_pous::write_generated;
use trust_runtime::native::{build_project, Build, Options, Target, Toolchain};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime_core::native::image::{Image, ImageService, SectionId};

/// A program with a variable of each kind a path can name: two elementary values, a structure, a
/// timer, which is an instance, and a constant.
const PROGRAM: &str = "TYPE Point :
STRUCT
    x : REAL;
    y : REAL;
END_STRUCT;
END_TYPE

PROGRAM Line
VAR
    target : REAL;
    speed : INT;
    point : Point;
    timer : TON;
END_VAR
VAR CONSTANT
    limit : INT := 5;
END_VAR
speed := speed + 1;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

/// A project of [`PROGRAM`] and a `trust.toml` of `services`, with its generated heartbeats.
fn project(name: &str, services: &str) -> PathBuf {
    let root = trust_test_support::unique_temp_path(name);
    for (path, text) in [("trust.toml", services), ("src/main.st", PROGRAM)] {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("the folder");
        std::fs::write(file, text).expect("the file");
    }
    write_generated(&root).expect("the generated files write");
    root
}

/// The project built for this computer.
fn build(root: &Path) -> Result<Build, String> {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    build_project(
        root,
        &sources(root),
        &[],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: false,
        },
        ConversionProfile::Iec,
        &toolchain,
        &root.join("build/native"),
    )
    .map_err(|error| error.to_string())
}

#[test]
fn the_image_lists_each_services_writable_variables() {
    // RU-63 and §5.14: recipe may write the target and the speed, spelled as the project spells
    // them, dosing a member of the structure; monitor writes nothing and is not listed. Catches a
    // build that drops `writes`, respells a path, or lists a service that writes nothing.
    let root = project(
        "service-writes",
        "[services.recipe]\nwrites = [\"Main.target\", \"main.SPEED\"]\n\n\
         [services.monitor]\nheartbeat = \"1s\"\n\n\
         [services.dosing]\nwrites = [\"Main.point.x\"]\n",
    );
    let built = build(&root).unwrap_or_else(|error| panic!("the project builds: {error}"));
    let image = Image::parse(&built.image).expect("the image");
    assert_eq!(
        image.services().expect("the service table"),
        vec![
            ImageService {
                name: "recipe",
                paths: vec!["Main.target", "main.SPEED"],
            },
            ImageService {
                name: "dosing",
                paths: vec!["Main.point.x"],
            },
        ]
    );
}

#[test]
fn a_project_whose_services_write_nothing_has_no_service_table() {
    // §5.14: an image without the table lets no service write a variable. Catches an empty table
    // written for services that write nothing.
    let root = project(
        "service-writes-none",
        "[services.monitor]\nheartbeat = \"1s\"\n",
    );
    let built = build(&root).unwrap_or_else(|error| panic!("the project builds: {error}"));
    let image = Image::parse(&built.image).expect("the image");
    assert_eq!(image.section(SectionId::Services), None);
}

#[test]
fn the_build_refuses_each_path_no_service_may_write() {
    // RU-63: a path that names no member, a constant, the timer, which is an instance, and the
    // structure, which holds no single value, are refused, and so are the target given twice by
    // recipe, in another case, and given again by dosing, as a variable has one service that writes
    // it; every refusal names its service and its path. The two acceptable paths, the target once
    // and the speed, are not named. Catches a build that stops at the first refusal, accepts a
    // constant or a structure, or lets two services write one variable.
    let root = project(
        "service-writes-refused",
        "[services.recipe]\nwrites = [\"Main.nothing\", \"Main.limit\", \"Main.timer\", \
         \"Main.point\", \"Main.target\", \"MAIN.TARGET\", \"Main.speed\"]\n\n\
         [services.dosing]\nwrites = [\"Main.target\"]\n",
    );
    let Err(error) = build(&root) else {
        panic!("the paths are refused");
    };
    let named: Vec<&str> = error
        .lines()
        .filter(|line| line.contains("trust.toml: [services."))
        .collect();
    let expected = [
        ("recipe", "Main.nothing", "names no member"),
        ("recipe", "Main.limit", "is a constant"),
        (
            "recipe",
            "Main.timer",
            "is an instance, which holds no value",
        ),
        ("recipe", "Main.point", "is not an elementary variable"),
        ("recipe", "MAIN.TARGET", "twice"),
        (
            "dosing",
            "Main.target",
            "a variable has one service that writes it",
        ),
    ];
    assert_eq!(named.len(), expected.len(), "{error}");
    for ((service, path, reason), line) in expected.iter().zip(&named) {
        assert!(
            line.contains(&format!("[services.{service}] writes \"{path}\""))
                && line.contains(reason)
                && line.contains("specification 48 RU-63"),
            "{service} {path} {reason}: {line}"
        );
    }
}

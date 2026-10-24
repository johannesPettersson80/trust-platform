//! Signing on a computer (specification 44 §5.11, §10.2 step 4): `trust-runtime build` signs the
//! image it writes with the user's own key, which the first build creates in truST's configuration
//! folder, and `trust-runtime validate` admits the project's image as the runtime does, against the
//! keys of the computer's `device.toml` in that folder.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`. Each command runs
//! with a configuration folder of the test's own, never the user's.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value as JsonValue;
use smol_str::SmolStr;
use trust_runtime::bundle_template::{build_io_config_auto, render_io_toml, render_runtime_toml};
use trust_runtime::deployment::DEPLOYMENT_RECORD_FILE;
use trust_runtime::native::{read_key, PROGRAM_IMAGE_FILE};
use trust_runtime_core::native::image::{Image, SectionId};

/// A counter in a 10 ms task.
const COUNTER: &str = "
PROGRAM Counter
VAR
    Count : UDINT;
END_VAR
Count := Count + 1;
END_PROGRAM

CONFIGURATION Signed
RESOURCE Signed ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Counter;
END_RESOURCE
END_CONFIGURATION
";

/// A project with the counter, and a home and configuration folder for the commands, all removed
/// when dropped.
struct Fixture {
    folder: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let folder = std::env::temp_dir().join(format!(
            "trust-native-signing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        let project = folder.join("project");
        std::fs::create_dir_all(project.join("src")).expect("the project folder");
        std::fs::create_dir_all(folder.join("home")).expect("the home folder");
        std::fs::create_dir_all(folder.join("config")).expect("the configuration folder");
        let runtime = render_runtime_toml(&SmolStr::new("signed"), 10);
        #[cfg(windows)]
        let runtime = runtime.replacen(
            "endpoint = \"unix:///tmp/trust-runtime.sock\"",
            "endpoint = \"tcp://127.0.0.1:19093\"\nauth_token = \"signing-test-token\"",
            1,
        );
        std::fs::write(project.join("runtime.toml"), runtime).expect("runtime.toml");
        let io = render_io_toml(&build_io_config_auto("loopback").expect("loopback I/O"));
        std::fs::write(project.join("io.toml"), io).expect("io.toml");
        std::fs::write(project.join("src/main.st"), COUNTER).expect("the program");
        Self { folder }
    }

    fn project(&self) -> PathBuf {
        self.folder.join("project")
    }

    /// truST's configuration folder of the user the commands run as (specification 44 §5.11).
    fn trust_folder(&self) -> PathBuf {
        if cfg!(target_os = "macos") {
            self.folder.join("home/Library/Application Support/trust")
        } else {
            self.folder.join("config/trust")
        }
    }

    /// `trust-runtime` with `args`, as a user whose home and configuration folders are the
    /// fixture's.
    fn run(&self, args: &[&str]) -> Output {
        Command::new(
            std::env::var_os("CARGO_BIN_EXE_trust-runtime")
                .expect("Cargo provides the trust-runtime binary to integration tests"),
        )
        .args(args)
        .arg("--project")
        .arg(self.project())
        .env("XDG_CONFIG_HOME", self.folder.join("config"))
        .env("APPDATA", self.folder.join("config"))
        .env("HOME", self.folder.join("home"))
        .output()
        .expect("run trust-runtime")
    }

    /// `trust-runtime build --ci` for this computer: the signer its report names.
    fn build(&self) -> String {
        let output = self.run(&["build", "--ci"]);
        assert!(output.status.success(), "{}", failure(&output));
        let report: JsonValue = serde_json::from_slice(&output.stdout).expect("the build's report");
        report["signer"]
            .as_str()
            .unwrap_or_else(|| panic!("no signer in {report}"))
            .to_string()
    }

    /// The computer's `device.toml` trusting `keys`.
    fn trust(&self, keys: &[&str]) {
        let listed: Vec<String> = keys.iter().map(|key| format!("\"{key}\"")).collect();
        std::fs::create_dir_all(self.trust_folder()).expect("the trust folder");
        std::fs::write(
            self.trust_folder().join("device.toml"),
            format!("[signing]\ntrusted_keys = [{}]\n", listed.join(", ")),
        )
        .expect("device.toml");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}

fn failure(output: &Output) -> String {
    format!(
        "status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// `trust-runtime validate` refused the project's image with `reason`.
fn refused(output: &Output, reason: &str) {
    assert!(!output.status.success(), "admitted: {}", failure(output));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(reason),
        "expected '{reason}': {}",
        failure(output)
    );
}

/// Specification 44 §5.11 and §10.2 step 4, end to end on this computer: the first build creates
/// the user's key, readable by its owner alone, and signs with it, the report naming its public
/// key; a second build signs with the same key. Without a `device.toml` the computer admits the
/// signed image; trusting another key it refuses it (`the program is not signed by a key this
/// device trusts`); trusting the signer it admits it; and an image changed after signing, with a
/// record written for its new bytes, is refused (`the program's signature does not verify: it was
/// changed after signing`). Catches a build that does not sign, a new key for every build (which
/// would orphan every device trusting the first), a key file others can read, a computer that
/// ignores its trusted keys, and a signature that covers less than the image.
#[test]
fn a_computer_runs_only_images_signed_by_a_key_it_trusts() {
    trust_runtime::native::Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let fixture = Fixture::new();

    let signer = fixture.build();
    let key_file = fixture.trust_folder().join("signing-key.pem");
    let key = read_key(&key_file).expect("the first build made the user's key");
    assert_eq!(signer, hex::encode(key.verifying_key().as_bytes()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&key_file)
            .expect("the key file")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    assert_eq!(
        fixture.build(),
        signer,
        "a second build signs with the same key"
    );

    let validate = || fixture.run(&["validate"]);
    let admitted = validate();
    assert!(admitted.status.success(), "{}", failure(&admitted));

    fixture.trust(&[&"11".repeat(32)]);
    refused(
        &validate(),
        "the program is not signed by a key this device trusts",
    );

    fixture.trust(&[&"11".repeat(32), &signer]);
    let trusted = validate();
    assert!(trusted.status.success(), "{}", failure(&trusted));

    let project = fixture.project();
    let mut image = std::fs::read(project.join(PROGRAM_IMAGE_FILE)).expect("program.tpi");
    let code = Image::parse(&image)
        .expect("the image parses")
        .section(SectionId::Code)
        .expect("a CODE section")
        .offset;
    let code = usize::try_from(code).expect("an offset");
    image[code] ^= 0x01;
    std::fs::write(project.join(PROGRAM_IMAGE_FILE), &image).expect("the changed image");
    write_record(&project, &image);
    refused(
        &validate(),
        "the program's signature does not verify: it was changed after signing",
    );
}

/// The record of `image` in `project`, as the build writes it, so that only the signature tells
/// the changed image from the built one.
fn write_record(project: &Path, image: &[u8]) {
    std::fs::write(
        project.join(DEPLOYMENT_RECORD_FILE),
        trust_runtime::deployment::image_record(project, image).expect("the record"),
    )
    .expect("the record file");
}

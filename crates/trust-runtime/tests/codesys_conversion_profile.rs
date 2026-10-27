use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::Value;

// docs/specs/07-standard-functions.md 2.10: a compile session with the CODESYS conversion
// profile uses it in the bytecode VM, and a reload keeps it.
const SOURCE: &str = r#"
PROGRAM Main
VAR
    half : REAL := 2.5;
    level : DINT := 200;
    raw : DWORD := 16#0000_0005;
    status : BYTE := 16#80;
    flagged : BOOL;
    rounded : INT;
    narrowed : SINT;
    scaled : REAL;
    packed : DWORD;
END_VAR
rounded := REAL_TO_INT(half);
narrowed := DINT_TO_SINT(level);
scaled := DWORD_TO_REAL(raw) * 2.0;
packed := REAL_TO_DWORD(half);
flagged := BYTE_TO_BOOL(status AND BYTE#16#80);
END_PROGRAM
"#;

#[test]
fn codesys_conversion_profile_applies_to_program_code_and_survives_reload() {
    let session =
        CompileSession::from_source(SOURCE).with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).unwrap();
    harness.cycle();

    assert_eq!(harness.get_output("rounded"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("narrowed"), Some(Value::SInt(-56)));
    assert_eq!(harness.get_output("scaled"), Some(Value::Real(10.0)));
    assert_eq!(harness.get_output("packed"), Some(Value::DWord(3)));
    assert_eq!(harness.get_output("flagged"), Some(Value::Bool(true)));

    harness.reload_sources(&[SOURCE]).unwrap();
    harness.cycle();
    assert_eq!(harness.get_output("narrowed"), Some(Value::SInt(-56)));
}

// The CODESYS rules of docs/specs/07-standard-functions.md 2.10 in a compiled program, worked by
// hand from its table: `REAL_TO_INT(2.5) = 3` (the IEC rules round the tie to 2), `DINT_TO_SINT(200)
// = -56` (the low-order byte of 16#C8; the IEC rules overflow), `DWORD_TO_REAL(5) * 2.0 = 10.0`
// (the IEC rules transfer the bits, a denormal) and `REAL_TO_DWORD(2.5) = 3` (the IEC rules give the
// bits 16#4020_0000).
const NUMBERS: &str = r#"
PROGRAM Main
VAR
    half : REAL := 2.5;
    level : DINT := 200;
    raw : DWORD := 16#0000_0005;
    rounded : INT;
    narrowed : SINT;
    scaled : REAL;
    packed : DWORD;
END_VAR
rounded := REAL_TO_INT(half);
narrowed := DINT_TO_SINT(level);
scaled := DWORD_TO_REAL(raw) * 2.0;
packed := REAL_TO_DWORD(half);
END_PROGRAM
"#;

/// The image a computer runs of `source`, built with the IEC rules, for an online change.
fn iec_image(source: &str) -> Vec<u8> {
    use trust_runtime::native::{build, Options, Target, Toolchain};
    let toolchain = Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain");
    let target = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!("trust-codesys-iec-{}", std::process::id()));
    let built = build(
        &[trust_runtime::harness::SourceFile::new(source)],
        &Options {
            target,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the IEC image builds");
    let _ = std::fs::remove_dir_all(&work);
    built.image
}

/// Catches a compiled program that converts by the IEC rules under the CODESYS profile, a reload
/// that switches the rules back, and an online change to an image of the other rules that the
/// runtime takes: it is refused, names both rules, and the running program goes on with its own.
#[test]
fn a_compiled_program_converts_by_the_codesys_rules_and_keeps_them_over_changes() {
    let session =
        CompileSession::from_source(NUMBERS).with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS image builds");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    assert_eq!(harness.get_output("rounded"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("narrowed"), Some(Value::SInt(-56)));
    assert_eq!(harness.get_output("scaled"), Some(Value::Real(10.0)));
    assert_eq!(harness.get_output("packed"), Some(Value::DWord(3)));

    harness
        .reload_sources(&[NUMBERS])
        .expect("a reload keeps the rules");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    assert_eq!(harness.get_output("rounded"), Some(Value::Int(3)));

    let refusal = harness
        .runtime_mut()
        .apply_online_change_bytes(&iec_image(NUMBERS))
        .expect_err("a change to the IEC rules is refused");
    let text = refusal.to_string();
    assert!(
        text.contains("with the iec conversion rules")
            && text.contains("running program with the codesys rules"),
        "{text}"
    );
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    assert_eq!(harness.get_output("narrowed"), Some(Value::SInt(-56)));
}

/// A project's `[project] vendor_profile` in `trust-lsp.toml` chooses the rules its build compiles
/// with, and the image's header records them (docs/specs/07-standard-functions.md 2.10;
/// specification 44 §5.2): `twincat` the CODESYS rules, none the IEC rules; a name that is no
/// vendor profile fails the build and the check with the supported names. Catches a build that
/// ignores the setting, a header that does not record it, and a misspelling built by other rules.
#[test]
fn a_project_build_takes_the_rules_of_its_vendor_profile() {
    use trust_runtime::bundle_builder::{build_program_image, check_program};
    use trust_runtime::native::{Target, Toolchain};
    use trust_runtime_core::native::image::Image;
    let toolchain = Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain");
    let target = Target::this_cpu().expect("a computer target");
    let project = |name: &str, settings: Option<&str>| {
        let root = std::env::temp_dir().join(format!(
            "trust-codesys-project-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("the project folder");
        std::fs::write(root.join("src/main.st"), NUMBERS).expect("the source");
        if let Some(settings) = settings {
            std::fs::write(root.join("trust-lsp.toml"), settings).expect("the settings");
        }
        root
    };
    for (name, settings, expected) in [
        (
            "twincat",
            Some("[project]\nvendor_profile = \"twincat\"\n"),
            ConversionProfile::Codesys,
        ),
        ("none", None, ConversionProfile::Iec),
    ] {
        let root = project(name, settings);
        let report = build_program_image(&root, None, target, &toolchain, false, true)
            .unwrap_or_else(|error| panic!("{name} builds: {error:#}"));
        let image = std::fs::read(&report.image).expect("the image");
        assert_eq!(
            Image::parse(&image)
                .expect("the image parses")
                .header()
                .conversion_profile(),
            expected,
            "{name}"
        );
        let _ = std::fs::remove_dir_all(root);
    }
    let root = project(
        "misspelled",
        Some("[project]\nvendor_profile = \"codesis\"\n"),
    );
    let built = build_program_image(&root, None, target, &toolchain, false, true)
        .expect_err("a misspelled profile is refused");
    let checked = check_program(&root, None).expect_err("the check refuses it too");
    for error in [built, checked] {
        let text = format!("{error:#}");
        assert!(text.contains("unknown vendor_profile 'codesis'"), "{text}");
        assert!(
            text.contains("beckhoff, codesys, gxworks3, iec, mitsubishi, siemens, twincat"),
            "{text}"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

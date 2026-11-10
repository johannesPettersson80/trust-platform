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

/// Specification 05 §5.2.1: an untyped literal passed to a conversion takes its input's type, so
/// it converts as a variable of that type does. Under the CODESYS rules `REAL_TO_INT(2.5)` is 3 and
/// under the IEC rules 2, the tie to even, as for the `REAL` variable above. Catches the literal
/// refused with E205, and a literal compiled as the `LREAL` it defaults to.
#[test]
fn a_literal_converts_like_a_variable_of_the_input_type() {
    const LITERALS: &str = "PROGRAM Main\nVAR\n    rounded : INT;\n    widened : INT;\nEND_VAR\n\
                            rounded := REAL_TO_INT(2.5);\nwidened := SINT_TO_INT(5);\nEND_PROGRAM\n";
    for (profile, expected) in [(ConversionProfile::Codesys, 3), (ConversionProfile::Iec, 2)] {
        let session = CompileSession::from_source(LITERALS).with_conversion_profile(profile);
        let mut harness = TestHarness::from_session(session).expect("the image builds");
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "{profile:?}: {:?}", cycle.errors);
        assert_eq!(
            harness.get_output("rounded"),
            Some(Value::Int(expected)),
            "{profile:?}"
        );
        assert_eq!(
            harness.get_output("widened"),
            Some(Value::Int(5)),
            "{profile:?}"
        );
    }
}

/// The image a computer runs of `source`, built with the IEC rules, for an online change.
fn iec_image(source: &str) -> Vec<u8> {
    use trust_runtime::native::{build, Options, Target, Toolchain};
    let toolchain = Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain");
    let target = Target::this_cpu().expect("a computer target");
    let work =
        trust_test_support::unique_temp_path(&format!("trust-codesys-iec-{}", std::process::id()));
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
        let root = trust_test_support::unique_temp_path(&format!(
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
        let report = build_program_image(&root, &[], target, &toolchain, false, true)
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
    let built = build_program_image(&root, &[], target, &toolchain, false, true)
        .expect_err("a misspelled profile is refused");
    let checked = check_program(&root, &[]).expect_err("the check refuses it too");
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

// Specification 44 §6.7: a declaration's initial value converts by the program's rules
// (docs/specs/07-standard-functions.md 2.10). Worked out by hand from the vendor's tables:
// `ANY_TO_BYTE(255)` is 255; `ANY_TO_BYTE(300)` keeps the low-order byte, 44; `ANY_TO_WORD(65536)`
// keeps the low-order word, 0; `ANY_TO_REAL(2.5)` is 2.5. The build refused every one of them
// with "ANY_TO_BYTE in a declaration of a program built with the CODESYS conversion rules is not
// compiled yet" (NX-09), and the evaluator's conversion of a generic `ANY_TO_*` refused every
// concrete value with a type mismatch. Catches the build refusing what the type checker admits
// under the CODESYS profile, and a folded value that strays from the vendor's rules.
const DUT_DEFAULTS: &str = r#"
TYPE stCfg :
STRUCT
    bXMax : BYTE := ANY_TO_BYTE(255);
    bWrap : BYTE := ANY_TO_BYTE(300);
    wWide : WORD := ANY_TO_WORD(65536);
    rGamma : REAL := ANY_TO_REAL(2.5);
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    cfg : stCfg;
    bMax, bWrapped : BYTE;
    wWide : WORD;
    rGamma : REAL;
END_VAR
    bMax := cfg.bXMax;
    bWrapped := cfg.bWrap;
    wWide := cfg.wWide;
    rGamma := cfg.rGamma;
END_PROGRAM
"#;

#[test]
fn a_dut_member_default_converts_by_the_codesys_rules() {
    let session = CompileSession::from_source(DUT_DEFAULTS)
        .with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS image builds");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    assert_eq!(harness.get_output("bMax"), Some(Value::Byte(255)));
    assert_eq!(harness.get_output("bWrapped"), Some(Value::Byte(44)));
    assert_eq!(harness.get_output("wWide"), Some(Value::Word(0)));
    assert_eq!(harness.get_output("rGamma"), Some(Value::Real(2.5)));
}

/// Catches a build that works a mutable variable's value into a declaration's constant:
/// `ANY_TO_BYTE(gLevel)` names no compile-time constant, so the build refuses it as it refuses
/// the bare `gLevel` (specification 44 §6.7), instead of reading the variable's starting value.
#[test]
fn a_conversion_of_a_mutable_variable_is_refused_as_an_initial_value() {
    let source = r#"
VAR_GLOBAL
    gLevel : DINT := 300;
END_VAR

TYPE stCfg :
STRUCT
    bWrap : BYTE := ANY_TO_BYTE(gLevel);
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    cfg : stCfg;
END_VAR
END_PROGRAM
"#;
    let session =
        CompileSession::from_source(source).with_conversion_profile(ConversionProfile::Codesys);
    let error = TestHarness::from_session(session)
        .err()
        .expect("the build refuses a variable in a constant conversion");
    let text = error.to_string();
    assert!(
        text.contains("the starting value of") && !text.contains("is not compiled yet"),
        "{text}"
    );
}

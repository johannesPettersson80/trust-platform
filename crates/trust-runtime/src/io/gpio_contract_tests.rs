use super::*;

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[test]
fn gpio_contract_config_defaults_and_backend_aliases_are_stable() {
    let empty = gpio_params("");
    let config = GpioConfig::parse(&empty).unwrap();
    assert_eq!(config.backend, GpioBackendKind::Chardev);
    assert_eq!(config.chip_path, PathBuf::from("/dev/gpiochip0"));
    assert_eq!(config.sysfs_base, PathBuf::from("/sys/class/gpio"));
    assert!(config.inputs.is_empty());
    assert!(config.outputs.is_empty());

    for alias in ["chardev", "CHARDEV", "libgpiod", "LiBgPiOd"] {
        let config = GpioConfig::parse(&gpio_params(&format!(
            "backend = \"{alias}\"\nchip = \"/dev/custom-chip\"\n"
        )))
        .unwrap();
        assert_eq!(config.backend, GpioBackendKind::Chardev);
        assert_eq!(config.chip_path, PathBuf::from("/dev/custom-chip"));
    }

    let sysfs = GpioConfig::parse(&gpio_params(
        "backend = \"SySfS\"\nsysfs_base = \"/tmp/custom-gpio\"\n",
    ))
    .unwrap();
    assert_eq!(sysfs.backend, GpioBackendKind::Sysfs);
    assert_eq!(sysfs.sysfs_base, PathBuf::from("/tmp/custom-gpio"));
}

#[test]
fn gpio_contract_rejects_non_table_root_and_unknown_backend() {
    for value in [
        toml::Value::String("gpio".to_string()),
        toml::Value::Array(Vec::new()),
        toml::Value::Integer(1),
    ] {
        assert_error_contains(GpioConfig::parse(&value), "must be a table");
    }
    assert_error_contains(
        GpioConfig::parse(&gpio_params("backend = \"magic\"\n")),
        "unsupported gpio backend 'magic'",
    );
    assert_error_contains(
        GpioConfig::parse(&gpio_params("backend = 7\n")),
        "unsupported gpio backend",
    );
}

#[test]
fn gpio_contract_rejects_wrong_collection_and_entry_shapes() {
    for (text, expected) in [
        ("inputs = 1", "inputs must be an array"),
        ("outputs = \"x\"", "outputs must be an array"),
        ("inputs = [1]", "gpio input entry must be a table"),
        ("outputs = [true]", "gpio output entry must be a table"),
    ] {
        assert_error_contains(GpioConfig::parse(&gpio_params(text)), expected);
    }
}

#[test]
fn gpio_contract_requires_address_and_physical_line() {
    for (text, expected) in [
        ("inputs = [{ line = 1 }]", "gpio entry missing 'address'"),
        ("outputs = [{ address = \"%QX0.0\" }]", "requires 'line'"),
        (
            "inputs = [{ address = 7, line = 1 }]",
            "gpio entry missing 'address'",
        ),
        (
            "outputs = [{ address = \"%QX0.0\", line = \"1\" }]",
            "requires 'line'",
        ),
    ] {
        assert_error_contains(GpioConfig::parse(&gpio_params(text)), expected);
    }
}

#[test]
fn gpio_contract_line_alias_precedence_and_u32_bounds_are_closed() {
    let config = GpioConfig::parse(&gpio_params(
        r#"
inputs = [
  { address = "%IX0.0", pin = 17 },
  { address = "%IX0.1", line = 27, pin = 99 }
]
"#,
    ))
    .unwrap();
    assert_eq!(config.inputs[0].line, 17);
    assert_eq!(config.inputs[1].line, 27);

    for line in ["-1", "4294967296", "9223372036854775807"] {
        assert_error_contains(
            GpioConfig::parse(&gpio_params(&format!(
                "inputs = [{{ address = \"%IX0.0\", line = {line} }}]"
            ))),
            "line",
        );
    }
}

#[test]
fn gpio_contract_boolean_forms_and_defaults_are_deterministic() {
    let config = GpioConfig::parse(&gpio_params(
        r#"
inputs = [
  { address = "%IX0.0", line = 1 },
  { address = "%IX0.1", line = 2, invert = true },
  { address = "%IX0.2", line = 3, invert = 0 },
  { address = "%IX0.3", line = 4, invert = -2 },
  { address = "%IX0.4", line = 5, invert = " 1 " },
  { address = "%IX0.5", line = 6, invert = "FALSE" }
]
outputs = [
  { address = "%QX0.0", line = 7 },
  { address = "%QX0.1", line = 8, initial = "true", invert = "0" }
]
"#,
    ))
    .unwrap();
    assert!(!config.inputs[0].invert);
    assert!(config.inputs[1].invert);
    assert!(!config.inputs[2].invert);
    assert!(config.inputs[3].invert);
    assert!(config.inputs[4].invert);
    assert!(!config.inputs[5].invert);
    assert!(!config.outputs[0].initial);
    assert!(!config.outputs[0].invert);
    assert!(config.outputs[1].initial);
    assert!(!config.outputs[1].invert);

    for value in ["\"yes\"", "[]", "{}"] {
        assert_error_contains(
            GpioConfig::parse(&gpio_params(&format!(
                "inputs = [{{ address = \"%IX0.0\", line = 1, invert = {value} }}]"
            ))),
            "invert",
        );
    }
}

#[test]
fn gpio_contract_debounce_forms_and_bounds_are_deterministic() {
    let config = GpioConfig::parse(&gpio_params(
        r#"
inputs = [
  { address = "%IX0.0", line = 1 },
  { address = "%IX0.1", line = 2, debounce_ms = 15 },
  { address = "%IX0.2", line = 3, debounce_ms = " 27 " }
]
"#,
    ))
    .unwrap();
    assert_eq!(config.inputs[0].debounce_ms, 0);
    assert_eq!(config.inputs[1].debounce_ms, 15);
    assert_eq!(config.inputs[2].debounce_ms, 27);

    for value in ["-1", "\"-1\"", "\"abc\"", "true"] {
        assert_error_contains(
            GpioConfig::parse(&gpio_params(&format!(
                "inputs = [{{ address = \"%IX0.0\", line = 1, debounce_ms = {value} }}]"
            ))),
            "debounce_ms",
        );
    }
}

#[test]
fn gpio_contract_input_addresses_are_concrete_simple_ix_bits() {
    for (address, expected) in [
        ("%I*", "wildcard"),
        ("%IW0", "must be bit"),
        ("%QX0.0", "must be %I"),
        ("%MX0.0", "must be %I"),
        ("%IX0.1.2", "simple bit address"),
    ] {
        assert_error_contains(
            GpioDriver::validate_params(&gpio_params(&format!(
                "inputs = [{{ address = \"{address}\", line = 1 }}]"
            ))),
            expected,
        );
    }
}

#[test]
fn gpio_contract_output_addresses_are_concrete_simple_qx_bits() {
    for (address, expected) in [
        ("%Q*", "wildcard"),
        ("%QW0", "must be bit"),
        ("%IX0.0", "must be %Q"),
        ("%MX0.0", "must be %Q"),
        ("%QX0.1.2", "simple bit address"),
    ] {
        assert_error_contains(
            GpioDriver::validate_params(&gpio_params(&format!(
                "outputs = [{{ address = \"{address}\", line = 1 }}]"
            ))),
            expected,
        );
    }
}

#[test]
fn gpio_contract_rejects_duplicate_physical_lines_across_all_mappings() {
    for text in [
        r#"inputs = [
          { address = "%IX0.0", line = 1 },
          { address = "%IX0.1", line = 1 }
        ]"#,
        r#"outputs = [
          { address = "%QX0.0", line = 2 },
          { address = "%QX0.1", line = 2 }
        ]"#,
        r#"
        inputs = [{ address = "%IX0.0", line = 3 }]
        outputs = [{ address = "%QX0.0", line = 3 }]
        "#,
    ] {
        assert_error_contains(
            GpioDriver::validate_params(&gpio_params(text)),
            "physical line",
        );
    }
}

#[test]
fn gpio_contract_rejects_duplicate_process_image_bits_per_direction() {
    for text in [
        r#"inputs = [
          { address = "%IX0.0", line = 1 },
          { address = "%IX0.0", line = 2 }
        ]"#,
        r#"outputs = [
          { address = "%QX0.0", line = 3 },
          { address = "%QX0.0", line = 4 }
        ]"#,
    ] {
        assert_error_contains(
            GpioDriver::validate_params(&gpio_params(text)),
            "process-image bit",
        );
    }
}

#[test]
fn gpio_contract_validate_params_has_no_backend_side_effects() {
    let root = TempTree::new("trust-gpio-validate");
    let params = gpio_params(&format!(
        r#"
backend = "sysfs"
sysfs_base = "{}"
inputs = [{{ address = "%IX0.0", line = 17 }}]
"#,
        root.path().display()
    ));

    GpioDriver::validate_params(&params).unwrap();

    assert!(fs::read_dir(root.path()).unwrap().next().is_none());
}

#[test]
fn gpio_contract_empty_driver_construction_does_not_open_default_chardev() {
    let mut driver = GpioDriver::from_params(&gpio_params("")).unwrap();
    assert!(matches!(driver.health(), IoDriverHealth::Ok));
    driver.read_inputs(&mut []).unwrap();
    driver.write_outputs(&[]).unwrap();
    assert!(matches!(driver.health(), IoDriverHealth::Ok));
    assert_eq!(
        format!("{driver:?}"),
        "GpioDriver { inputs: 0, outputs: 0 }"
    );
}

#[test]
fn gpio_contract_simple_map_and_bit_helpers_preserve_unselected_bits() {
    let address = IoAddress::parse("%IX12.6").unwrap();
    assert_eq!(map_bit(&address).unwrap(), (12, 6));

    let mut bytes = [0b1010_1010, 0b0101_0101];
    assert!(!read_bit(&bytes, 0, 0).unwrap());
    assert!(read_bit(&bytes, 0, 1).unwrap());
    write_bit(&mut bytes, 0, 0, true).unwrap();
    assert_eq!(bytes, [0b1010_1011, 0b0101_0101]);
    write_bit(&mut bytes, 0, 1, false).unwrap();
    assert_eq!(bytes, [0b1010_1001, 0b0101_0101]);
}

#[test]
fn gpio_contract_bit_helpers_reject_byte_and_bit_bounds_without_panicking() {
    assert_error_contains(read_bit(&[], 0, 0), "outside output buffer");
    assert_error_contains(read_bit(&[0], 0, 8), "bit");

    let mut empty = [];
    assert_error_contains(write_bit(&mut empty, 0, 0, true), "outside input buffer");
    let mut byte = [0b1010_1010];
    assert_error_contains(write_bit(&mut byte, 0, 8, true), "bit");
    assert_eq!(byte, [0b1010_1010]);
}

#[test]
fn gpio_contract_file_read_helpers_trim_and_reject_invalid_numbers() {
    let root = TempTree::new("trust-gpio-read-helpers");
    let number = root.path().join("number");
    let text = root.path().join("text");
    fs::write(&number, " 42\n").unwrap();
    fs::write(&text, " label \n").unwrap();
    assert_eq!(read_u32(&number), Some(42));
    assert_eq!(read_optional_trimmed(&text), Some("label".to_string()));

    fs::write(&number, "-1").unwrap();
    fs::write(&text, " \n").unwrap();
    assert_eq!(read_u32(&number), None);
    assert_eq!(read_optional_trimmed(&text), None);
    assert_eq!(read_u32(&root.path().join("missing")), None);
    assert_eq!(read_optional_trimmed(&root.path().join("missing")), None);
}

#[test]
fn gpio_contract_synthetic_chip_collection_is_bounded_and_skips_invalid_entries() {
    let root = TempTree::new("trust-gpio-discovery");
    let chip = root.path().join("gpiochip7");
    let skipped = root.path().join("gpiochip8");
    fs::create_dir_all(&chip).unwrap();
    fs::create_dir_all(&skipped).unwrap();
    fs::create_dir_all(root.path().join("not-a-chip")).unwrap();
    fs::write(chip.join("base"), "100").unwrap();
    fs::write(chip.join("ngpio"), "513").unwrap();
    fs::write(chip.join("label"), " test-chip \n").unwrap();
    fs::write(skipped.join("ngpio"), "0").unwrap();

    let mut lines = Vec::new();
    collect_gpiochip_lines(root.path(), &mut lines).unwrap();

    assert_eq!(lines.len(), 512);
    assert_eq!(
        lines.first(),
        Some(&GpioLineInfo {
            chip: "gpiochip7".to_string(),
            label: Some("test-chip".to_string()),
            line: 100,
            direction: None,
        })
    );
    assert_eq!(lines.last().unwrap().line, 611);
}

#[test]
fn gpio_contract_chip_collection_handles_missing_and_invalid_roots() {
    let root = TempTree::new("trust-gpio-discovery-roots");
    let missing = root.path().join("missing");
    let file = root.path().join("file");
    fs::write(&file, "not a directory").unwrap();

    let mut lines = Vec::new();
    collect_gpiochip_lines(&missing, &mut lines).unwrap();
    assert!(lines.is_empty());
    assert_error_contains(collect_gpiochip_lines(&file, &mut lines), "gpio enumerate");
}

#[test]
fn gpio_contract_sysfs_backend_configures_existing_input_and_output_files() {
    let root = TempTree::new("trust-gpio-sysfs-configure");
    make_sysfs_line(root.path(), 17);
    make_sysfs_line(root.path(), 27);
    let mut backend = SysfsBackend::new(root.path().to_path_buf());

    backend.configure_input(17).unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join("gpio17/direction")).unwrap(),
        "in"
    );

    backend.configure_output(27, true).unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join("gpio27/direction")).unwrap(),
        "out"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("gpio27/value")).unwrap(),
        "1"
    );
    backend.write(27, false).unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join("gpio27/value")).unwrap(),
        "0"
    );
}

#[test]
fn gpio_contract_sysfs_export_and_exact_input_values_are_fail_closed() {
    let root = TempTree::new("trust-gpio-sysfs-values");
    fs::write(root.path().join("export"), "").unwrap();
    let mut backend = SysfsBackend::new(root.path().to_path_buf());
    backend.ensure_exported(22).unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join("export")).unwrap(),
        "22"
    );

    make_sysfs_line(root.path(), 17);
    let value_path = root.path().join("gpio17/value");
    for (text, expected) in [("0\n", false), (" 1 \n", true)] {
        fs::write(&value_path, text).unwrap();
        assert_eq!(backend.read(17).unwrap(), expected);
    }
    for malformed in ["", "2", "-1", "true", "garbage"] {
        fs::write(&value_path, malformed).unwrap();
        assert_error_contains(backend.read(17), "invalid gpio value");
    }
}

#[test]
fn gpio_contract_input_handoff_maps_inversion_and_preserves_other_bits() {
    let backend = ScriptedBackend::with_reads([Ok(true), Ok(true)]);
    let mut driver = driver(
        backend,
        vec![input(17, 0, 0, false, 0), input(18, 0, 1, true, 0)],
        Vec::new(),
    );
    let mut image = [0b1010_1010];

    driver.read_inputs(&mut image).unwrap();

    assert_eq!(image, [0b1010_1001]);
    assert!(matches!(driver.health(), IoDriverHealth::Ok));
}

#[test]
fn gpio_contract_debounce_retains_then_accepts_changed_state() {
    let backend = ScriptedBackend::with_reads([Ok(false), Ok(true), Ok(true)]);
    let mut driver = driver(backend, vec![input(17, 0, 0, false, 10)], Vec::new());
    let mut image = [0u8];

    driver.read_inputs(&mut image).unwrap();
    assert_eq!(image, [0]);
    driver.read_inputs(&mut image).unwrap();
    assert_eq!(image, [0], "change before debounce delay is retained");

    driver.inputs[0].last_change = Some(Instant::now() - StdDuration::from_millis(20));
    driver.read_inputs(&mut image).unwrap();
    assert_eq!(image, [1], "stable change after debounce delay is accepted");
}

#[test]
fn gpio_contract_output_handoff_inverts_and_suppresses_unchanged_levels() {
    let backend = ScriptedBackend::with_reads([]);
    let writes = Arc::clone(&backend.writes);
    let mut driver = driver(
        backend,
        Vec::new(),
        vec![output(27, 0, 0, false), output(28, 0, 1, true)],
    );

    driver.write_outputs(&[0b0000_0001]).unwrap();
    driver.write_outputs(&[0b0000_0001]).unwrap();
    driver.write_outputs(&[0b0000_0010]).unwrap();

    assert_eq!(
        writes.lock().unwrap().as_slice(),
        &[(27, true), (28, true), (27, false), (28, false),]
    );
}

#[test]
fn gpio_contract_failed_output_is_retried_and_health_recovers() {
    let backend = ScriptedBackend::with_reads([]);
    backend.fail_next_write.store(true, Ordering::Relaxed);
    let writes = Arc::clone(&backend.writes);
    let mut driver = driver(backend, Vec::new(), vec![output(27, 0, 0, false)]);

    assert_error_contains(driver.write_outputs(&[1]), "scripted write");
    assert!(matches!(driver.health(), IoDriverHealth::Faulted { .. }));
    assert!(writes.lock().unwrap().is_empty());

    driver.write_outputs(&[1]).unwrap();
    assert_eq!(writes.lock().unwrap().as_slice(), &[(27, true)]);
    assert!(matches!(driver.health(), IoDriverHealth::Ok));
}

#[test]
fn gpio_contract_read_failure_short_circuits_and_later_success_recovers_health() {
    let backend = ScriptedBackend::with_reads([Err("first read"), Ok(true), Ok(false)]);
    let mut driver = driver(
        backend,
        vec![input(17, 0, 0, false, 0), input(18, 0, 1, false, 0)],
        Vec::new(),
    );
    let mut image = [0u8];

    assert_error_contains(driver.read_inputs(&mut image), "first read");
    assert_eq!(image, [0]);
    assert!(matches!(driver.health(), IoDriverHealth::Faulted { .. }));

    driver.read_inputs(&mut image).unwrap();
    assert_eq!(image, [1]);
    assert!(matches!(driver.health(), IoDriverHealth::Ok));
}

#[test]
fn gpio_contract_process_image_bounds_failures_fault_health_without_mutation() {
    let backend = ScriptedBackend::with_reads([Ok(true)]);
    let mut input_driver = driver(backend, vec![input(17, 1, 0, false, 0)], Vec::new());
    let mut image = [0b1010_1010];
    assert_error_contains(input_driver.read_inputs(&mut image), "outside input buffer");
    assert_eq!(image, [0b1010_1010]);
    assert!(matches!(
        input_driver.health(),
        IoDriverHealth::Faulted { .. }
    ));

    let backend = ScriptedBackend::with_reads([]);
    let writes = Arc::clone(&backend.writes);
    let mut output_driver = driver(backend, Vec::new(), vec![output(27, 1, 0, false)]);
    assert_error_contains(
        output_driver.write_outputs(&[0b1010_1010]),
        "outside output buffer",
    );
    assert!(writes.lock().unwrap().is_empty());
    assert!(matches!(
        output_driver.health(),
        IoDriverHealth::Faulted { .. }
    ));
}

struct ScriptedBackend {
    reads: Mutex<VecDeque<Result<bool, &'static str>>>,
    writes: Arc<Mutex<Vec<(u32, bool)>>>,
    fail_next_write: AtomicBool,
}

impl ScriptedBackend {
    fn with_reads<const N: usize>(reads: [Result<bool, &'static str>; N]) -> Self {
        Self {
            reads: Mutex::new(reads.into_iter().collect()),
            writes: Arc::new(Mutex::new(Vec::new())),
            fail_next_write: AtomicBool::new(false),
        }
    }
}

impl GpioBackend for ScriptedBackend {
    fn configure_input(&mut self, _line: u32) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn configure_output(&mut self, _line: u32, _initial: bool) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn read(&mut self, _line: u32) -> Result<bool, RuntimeError> {
        match self.reads.lock().unwrap().pop_front().unwrap_or(Ok(false)) {
            Ok(value) => Ok(value),
            Err(message) => Err(RuntimeError::IoDriver(message.into())),
        }
    }

    fn write(&mut self, line: u32, value: bool) -> Result<(), RuntimeError> {
        if self.fail_next_write.swap(false, Ordering::Relaxed) {
            return Err(RuntimeError::IoDriver("scripted write failure".into()));
        }
        self.writes.lock().unwrap().push((line, value));
        Ok(())
    }
}

fn driver(
    backend: ScriptedBackend,
    inputs: Vec<GpioInput>,
    outputs: Vec<GpioOutput>,
) -> GpioDriver {
    GpioDriver {
        backend: Box::new(backend),
        inputs,
        outputs,
        health: IoDriverHealth::Ok,
    }
}

fn input(line: u32, byte: usize, bit: u8, invert: bool, debounce_ms: u64) -> GpioInput {
    GpioInput {
        line,
        byte,
        bit,
        invert,
        debounce_ms,
        last_state: false,
        last_change: None,
    }
}

fn output(line: u32, byte: usize, bit: u8, invert: bool) -> GpioOutput {
    GpioOutput {
        line,
        byte,
        bit,
        invert,
        last_written: None,
    }
}

fn gpio_params(text: &str) -> toml::Value {
    toml::from_str(text).expect("valid TOML fixture")
}

fn assert_error_contains<T: std::fmt::Debug>(result: Result<T, RuntimeError>, expected: &str) {
    let error = result.expect_err("expected GPIO error");
    assert!(
        error.to_string().contains(expected),
        "expected error containing {expected:?}, got {error}"
    );
}

fn make_sysfs_line(root: &Path, line: u32) {
    let line_path = root.join(format!("gpio{line}"));
    fs::create_dir_all(&line_path).unwrap();
    fs::write(line_path.join("direction"), "").unwrap();
    fs::write(line_path.join("value"), "").unwrap();
}

struct TempTree {
    path: PathBuf,
}

impl TempTree {
    fn new(prefix: &str) -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{sequence}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp tree");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

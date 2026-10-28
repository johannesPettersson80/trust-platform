//! The GPIO line that `identify` drives directly (specification 11, "Identify"). The line is
//! not part of `io.toml`: it belongs to no `gpio` driver entry and no process image bit, and the
//! project is refused when an entry maps it (see `check_identify_output` in the parent module's
//! `identify`).

use super::config::{GpioBackendKind, GpioConfig};
use super::{make_chardev_backend, GpioBackend, SysfsBackend};
use crate::error::RuntimeError;

/// One GPIO line that `identify` blinks: requested as an output, low, the first time `identify`
/// runs, and driven through the same backends as the `gpio` driver.
pub struct GpioIdentifyLine {
    backend: Box<dyn GpioBackend>,
    line: u32,
    requested: bool,
}

impl std::fmt::Debug for GpioIdentifyLine {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GpioIdentifyLine")
            .field("line", &self.line)
            .field("requested", &self.requested)
            .finish_non_exhaustive()
    }
}

impl GpioIdentifyLine {
    /// The line `line` of the chip and backend that `params` name, the parameters of an enabled
    /// `gpio` driver of `io.toml` (`chip`, `backend`, `sysfs_base`); without a driver,
    /// `/dev/gpiochip0` on the character-device backend, the driver's defaults. Nothing is
    /// requested from the system yet.
    ///
    /// # Errors
    ///
    /// The parameters are not the shape the `gpio` driver accepts.
    pub fn new(params: Option<&toml::Value>, line: u32) -> Result<Self, RuntimeError> {
        let defaults = toml::Value::Table(toml::Table::new());
        let config = GpioConfig::parse(params.unwrap_or(&defaults))?;
        let backend: Box<dyn GpioBackend> = match config.backend {
            GpioBackendKind::Sysfs => Box::new(SysfsBackend::new(config.sysfs_base)),
            GpioBackendKind::Chardev => make_chardev_backend(config.chip_path),
        };
        Ok(Self::on(backend, line))
    }

    fn on(backend: Box<dyn GpioBackend>, line: u32) -> Self {
        Self {
            backend,
            line,
            requested: false,
        }
    }

    /// Requests the line as an output, low, unless an earlier call did.
    ///
    /// # Errors
    ///
    /// The backend's error: the line is held by another process, the chip cannot be opened, or
    /// this computer has no GPIO character devices.
    pub fn prepare(&mut self) -> Result<(), RuntimeError> {
        if !self.requested {
            self.backend.configure_output(self.line, false)?;
            self.requested = true;
        }
        Ok(())
    }

    /// Drives the line high (`true`) or low.
    ///
    /// # Errors
    ///
    /// The backend's error, or the line was not requested yet.
    pub fn set(&mut self, high: bool) -> Result<(), RuntimeError> {
        self.backend.write(self.line, high)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use smol_str::SmolStr;

    /// What the backend was asked to do, in order.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        ConfigureOutput(u32, bool),
        Write(u32, bool),
    }

    struct RecordingBackend {
        calls: Arc<Mutex<Vec<Call>>>,
        fail_configure: bool,
    }

    impl GpioBackend for RecordingBackend {
        fn configure_input(&mut self, _line: u32) -> Result<(), RuntimeError> {
            unreachable!("identify never reads a line");
        }

        fn configure_output(&mut self, line: u32, initial: bool) -> Result<(), RuntimeError> {
            self.calls
                .lock()
                .expect("recording lock")
                .push(Call::ConfigureOutput(line, initial));
            if self.fail_configure {
                return Err(RuntimeError::IoDriver(SmolStr::new("line is busy")));
            }
            Ok(())
        }

        fn read(&mut self, _line: u32) -> Result<bool, RuntimeError> {
            unreachable!("identify never reads a line");
        }

        fn write(&mut self, line: u32, value: bool) -> Result<(), RuntimeError> {
            self.calls
                .lock()
                .expect("recording lock")
                .push(Call::Write(line, value));
            Ok(())
        }
    }

    fn line_on(fail_configure: bool) -> (GpioIdentifyLine, Arc<Mutex<Vec<Call>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let backend = RecordingBackend {
            calls: Arc::clone(&calls),
            fail_configure,
        };
        (GpioIdentifyLine::on(Box::new(backend), 17), calls)
    }

    #[test]
    fn the_line_is_requested_once_as_an_output_that_starts_low_and_is_then_driven() {
        // Specification 11, "Identify", the output: the runtime requests the line as an output,
        // low, when identify first runs, and drives it directly. Preparing twice requests it
        // once (a second request of a held line would fail), and driving writes the given level
        // to line 17. Catches a line that starts high (the LED lights before identify blinks),
        // a request repeated at every identify, and a write to another line.
        let (mut line, calls) = line_on(false);

        line.prepare().expect("the first request succeeds");
        line.prepare().expect("the second request is a no-op");
        line.set(true).expect("drive high");
        line.set(false).expect("drive low");

        assert_eq!(
            *calls.lock().expect("recording lock"),
            vec![
                Call::ConfigureOutput(17, false),
                Call::Write(17, true),
                Call::Write(17, false),
            ]
        );
    }

    #[test]
    fn a_line_that_cannot_be_requested_reports_the_backend_error_and_is_asked_again() {
        // Specification 11, "Identify", other refusals: a line that another process holds makes
        // the request fail with the reason. The failed request is not remembered as done, so the
        // next identify asks again. Catches a swallowed error and a line marked ready after a
        // failed request.
        let (mut line, calls) = line_on(true);

        let first = line.prepare().expect_err("the line is busy");
        let second = line.prepare().expect_err("the line is still busy");

        assert_eq!(first, RuntimeError::IoDriver(SmolStr::new("line is busy")));
        assert_eq!(second, first);
        assert_eq!(calls.lock().expect("recording lock").len(), 2);
    }

    /// A sysfs folder that holds line 17 with empty `direction` and `value` files, and the
    /// parameters of a `gpio` driver that names it.
    fn sysfs_line_17(prefix: &str) -> (std::path::PathBuf, toml::Value) {
        let base = std::env::temp_dir().join(format!(
            "trust-runtime-{prefix}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let line = base.join("gpio17");
        std::fs::create_dir_all(&line).expect("create the sysfs folder");
        for leaf in ["direction", "value"] {
            std::fs::write(line.join(leaf), "").expect("create a sysfs file");
        }
        let mut table = toml::Table::new();
        table.insert(
            "backend".to_owned(),
            toml::Value::String("sysfs".to_owned()),
        );
        table.insert(
            "sysfs_base".to_owned(),
            toml::Value::String(base.to_string_lossy().into_owned()),
        );
        (base, toml::Value::Table(table))
    }

    #[test]
    fn the_line_is_driven_through_the_sysfs_base_of_the_gpio_driver() {
        // Specification 11, "Identify", the output: the line is a line of the chip and backend
        // that the first enabled `gpio` driver names. With `backend = "sysfs"` and a `sysfs_base`
        // that holds line 17, preparing makes line 17 an output that starts low and driving
        // writes the level to that line's `value` file. Catches a line driven on the default chip
        // while the project names another, a driver parameter that is ignored, and another line.
        let (base, params) = sysfs_line_17("identify-sysfs");
        let value_of = |leaf: &str| {
            std::fs::read_to_string(base.join("gpio17").join(leaf)).expect("a sysfs file")
        };
        let mut line = GpioIdentifyLine::new(Some(&params), 17).expect("a sysfs line");

        line.prepare().expect("the line is requested");
        assert_eq!(
            (value_of("direction").as_str(), value_of("value").as_str()),
            ("out", "0")
        );
        line.set(true).expect("drive high");
        assert_eq!(value_of("value"), "1");
        line.set(false).expect("drive low");
        assert_eq!(value_of("value"), "0");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_parameters_of_a_gpio_driver_choose_the_backend() {
        // Specification 11, "Identify", the output: the line is a line of the chip and backend of
        // the project's first enabled `gpio` driver. Parameters that the driver would refuse
        // are refused here too, and without a driver the defaults apply. Catches a line built
        // without the driver's parameter checks.
        let sysfs: toml::Value = toml::from_str("backend = \"sysfs\"\nsysfs_base = \"/tmp/gpio\"")
            .expect("driver parameters");
        assert!(GpioIdentifyLine::new(Some(&sysfs), 17).is_ok());
        assert!(GpioIdentifyLine::new(None, 17).is_ok());

        let unknown: toml::Value = toml::from_str("backend = \"spi\"").expect("driver parameters");
        assert!(GpioIdentifyLine::new(Some(&unknown), 17).is_err());
    }
}

//! Linux `PREEMPT_RT` posture configuration and verification helpers.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use smol_str::SmolStr;

use crate::error::RuntimeError;

#[cfg(target_os = "linux")]
use std::fs;

/// Linux scheduler policy values supported by the runtime RT posture profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinuxRtSchedulerPolicy {
    /// Normal Linux `SCHED_OTHER` time-sharing scheduling.
    #[default]
    Other,
    /// Linux `SCHED_FIFO` realtime scheduling.
    Fifo,
    /// Linux `SCHED_RR` realtime round-robin scheduling.
    RoundRobin,
}

impl LinuxRtSchedulerPolicy {
    /// Parse the configured scheduler policy name from `runtime.toml`.
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "other" => Ok(Self::Other),
            "fifo" => Ok(Self::Fifo),
            "rr" | "round-robin" | "round_robin" => Ok(Self::RoundRobin),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid runtime.realtime.scheduler '{text}'").into(),
            )),
        }
    }

    /// Return the config/control-surface string for this policy.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Other => "other",
            Self::Fifo => "fifo",
            Self::RoundRobin => "rr",
        }
    }

    /// Whether this policy requires a non-zero realtime priority.
    #[must_use]
    pub const fn requires_realtime_priority(self) -> bool {
        !matches!(self, Self::Other)
    }

    /// Convert a raw Linux scheduler policy number into the runtime enum.
    #[must_use]
    pub const fn from_linux_policy_raw(policy: i32) -> Self {
        match policy {
            1 => Self::Fifo,
            2 => Self::RoundRobin,
            _ => Self::Other,
        }
    }
}

/// Startup-time Linux RT posture requested through `[runtime.realtime]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxRtConfig {
    /// Enable Linux realtime posture verification.
    pub enabled: bool,
    /// Require kernel evidence confirming `PREEMPT_RT`.
    pub require_preempt_rt_kernel: bool,
    /// Request `mlockall(MCL_CURRENT|MCL_FUTURE)` on runtime start.
    pub lock_memory: bool,
    /// Expected scheduler policy for the scan thread.
    pub scheduler: LinuxRtSchedulerPolicy,
    /// Expected realtime priority in user-space scheduler terms (`1..=99`).
    pub priority: u8,
    /// Optional scheduler-thread CPU affinity mask.
    pub cpu_affinity: Vec<usize>,
    /// Fail startup instead of reporting a degraded RT posture.
    pub strict: bool,
}

impl LinuxRtConfig {
    /// Validate the configured RT posture before runtime startup.
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if self.priority > 99 {
            return Err(RuntimeError::InvalidConfig(
                "runtime.realtime.priority must be <= 99".into(),
            ));
        }
        if self.enabled && !self.scheduler.requires_realtime_priority() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.realtime.scheduler must be 'fifo' or 'rr' when runtime.realtime.enabled=true"
                    .into(),
            ));
        }
        if self.scheduler.requires_realtime_priority() && self.priority == 0 {
            return Err(RuntimeError::InvalidConfig(
                "runtime.realtime.priority must be >= 1 when runtime.realtime.scheduler is 'fifo' or 'rr'"
                    .into(),
            ));
        }
        let mut seen = HashSet::new();
        if self.cpu_affinity.iter().any(|cpu| !seen.insert(*cpu)) {
            return Err(RuntimeError::InvalidConfig(
                "runtime.realtime.cpu_affinity entries must be unique".into(),
            ));
        }
        Ok(())
    }

    /// Return the user-facing runtime profile label for this config.
    #[must_use]
    pub const fn profile_name(&self) -> &'static str {
        if self.enabled {
            "preempt-rt"
        } else {
            "disabled"
        }
    }
}

impl Default for LinuxRtConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            require_preempt_rt_kernel: false,
            lock_memory: false,
            scheduler: LinuxRtSchedulerPolicy::Other,
            priority: 70,
            cpu_affinity: Vec::new(),
            strict: false,
        }
    }
}

/// Observed Linux RT posture captured for the scheduler thread at startup.
#[derive(Debug, Clone)]
pub struct LinuxRtRuntimeStatus {
    /// Requested startup posture.
    pub requested: LinuxRtConfig,
    /// Kernel evidence for `PREEMPT_RT` when it can be determined.
    pub kernel_realtime: Option<bool>,
    /// Whether the runtime itself applied scheduler policy.
    pub scheduler_applied_by_runtime: bool,
    /// Whether the runtime itself applied CPU affinity.
    pub affinity_applied_by_runtime: bool,
    /// Whether `mlockall` succeeded.
    pub memory_lock_applied: bool,
    /// Observed scheduler policy for the scheduler thread.
    pub active_scheduler: Option<LinuxRtSchedulerPolicy>,
    /// Observed scheduler priority in user-space scheduler terms.
    pub active_priority: Option<i32>,
    /// Observed scheduler-thread CPU affinity mask.
    pub active_cpu_affinity: Vec<usize>,
    /// Observed locked memory from `/proc/self/status`.
    pub memory_locked_kb: Option<u64>,
    /// `true` when the observed posture matches the requested posture.
    pub active: bool,
    /// Non-fatal RT posture warnings.
    pub warnings: Vec<SmolStr>,
    /// RT posture mismatches and failures.
    pub errors: Vec<SmolStr>,
}

impl LinuxRtRuntimeStatus {
    /// Initialize an empty observed-status record from the requested config.
    #[must_use]
    pub fn from_config(config: LinuxRtConfig) -> Self {
        Self {
            requested: config,
            kernel_realtime: None,
            scheduler_applied_by_runtime: false,
            affinity_applied_by_runtime: false,
            memory_lock_applied: false,
            active_scheduler: None,
            active_priority: None,
            active_cpu_affinity: Vec::new(),
            memory_locked_kb: None,
            active: false,
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }
}

/// Shared sink updated with the observed scheduler-thread RT posture at startup.
pub type LinuxRtStatusSink = Arc<Mutex<LinuxRtRuntimeStatus>>;

/// Build a scheduler-thread startup hook that applies and verifies Linux RT posture.
pub fn make_thread_init_hook(
    config: LinuxRtConfig,
    status_sink: LinuxRtStatusSink,
) -> Arc<dyn Fn() -> Result<(), RuntimeError> + Send + Sync> {
    Arc::new(move || {
        let status = apply_current_thread_profile(config.clone());
        let strict_failure = if config.strict && !status.errors.is_empty() {
            Some(RuntimeError::ControlError(SmolStr::new(format!(
                "linux rt profile verification failed: {}",
                status
                    .errors
                    .iter()
                    .map(SmolStr::as_str)
                    .collect::<Vec<_>>()
                    .join("; ")
            ))))
        } else {
            None
        };
        if let Ok(mut guard) = status_sink.lock() {
            *guard = status;
        }
        if let Some(error) = strict_failure {
            return Err(error);
        }
        Ok(())
    })
}

#[cfg(target_os = "linux")]
fn apply_current_thread_profile(config: LinuxRtConfig) -> LinuxRtRuntimeStatus {
    use rustix::mm::{mlockall, MlockAllFlags};
    use rustix::thread::{sched_getaffinity, sched_setaffinity, CpuSet};

    let mut status = LinuxRtRuntimeStatus::from_config(config.clone());
    status.kernel_realtime = detect_linux_realtime_kernel();

    if !config.enabled {
        return status;
    }

    status.warnings.push(SmolStr::new(
        "scheduler policy and priority are deployment-provided (systemd/chrt); runtime verifies them but does not set them",
    ));

    if config.lock_memory {
        match mlockall(MlockAllFlags::CURRENT | MlockAllFlags::FUTURE) {
            Ok(()) => status.memory_lock_applied = true,
            Err(err) => status.errors.push(SmolStr::new(format!(
                "mlockall(MCL_CURRENT|MCL_FUTURE) failed: {err}"
            ))),
        }
    }

    if !config.cpu_affinity.is_empty() {
        let mut cpuset = CpuSet::new();
        let mut invalid_cpu = None;
        for cpu in &config.cpu_affinity {
            if *cpu >= CpuSet::MAX_CPU {
                invalid_cpu = Some(*cpu);
                break;
            }
            cpuset.set(*cpu);
        }
        if let Some(cpu) = invalid_cpu {
            status.errors.push(SmolStr::new(format!(
                "runtime.realtime.cpu_affinity contains cpu{cpu}, but this host only exposes affinity slots up to {}",
                CpuSet::MAX_CPU.saturating_sub(1)
            )));
        } else if let Err(err) = sched_setaffinity(None, &cpuset) {
            status
                .errors
                .push(SmolStr::new(format!("sched_setaffinity failed: {err}")));
        } else {
            status.affinity_applied_by_runtime = true;
        }
        status.warnings.push(SmolStr::new(
            "runtime CPU affinity only pins the scheduler thread; place web/mesh/other worker threads with systemd CPUAffinity or cgroup policy",
        ));
    }

    if config.require_preempt_rt_kernel && status.kernel_realtime != Some(true) {
        status.errors.push(SmolStr::new(
            "requested PREEMPT_RT kernel verification failed (kernel evidence did not confirm PREEMPT_RT)",
        ));
    }

    match read_sched_policy_and_priority() {
        Ok((policy, priority)) => {
            status.active_scheduler = Some(policy);
            status.active_priority = Some(priority);
            record_scheduler_observation(&mut status, &config, policy, priority);
        }
        Err(err) => status.errors.push(SmolStr::new(format!(
            "failed to read scheduler policy/priority from /proc/thread-self/stat: {err}"
        ))),
    }

    match sched_getaffinity(None) {
        Ok(cpuset) => {
            status.active_cpu_affinity = cpuset_to_vec(&cpuset);
            if !config.cpu_affinity.is_empty() && status.active_cpu_affinity != config.cpu_affinity
            {
                status.errors.push(SmolStr::new(format!(
                    "CPU affinity mismatch: expected {:?}, observed {:?}",
                    config.cpu_affinity, status.active_cpu_affinity
                )));
            }
        }
        Err(err) => status
            .errors
            .push(SmolStr::new(format!("sched_getaffinity failed: {err}"))),
    }

    match read_process_memory_lock_kb() {
        Ok(value) => {
            status.memory_locked_kb = Some(value);
            if config.lock_memory && value == 0 {
                status.errors.push(SmolStr::new(
                    "memory lock requested but /proc/self/status reports VmLck=0 kB",
                ));
            }
        }
        Err(err) => status.errors.push(SmolStr::new(format!(
            "failed to read VmLck from /proc/self/status: {err}"
        ))),
    }

    status.active = status.errors.is_empty();
    status
}

#[cfg(not(target_os = "linux"))]
fn apply_current_thread_profile(config: LinuxRtConfig) -> LinuxRtRuntimeStatus {
    let mut status = LinuxRtRuntimeStatus::from_config(config.clone());
    if config.enabled {
        status.errors.push(SmolStr::new(
            "runtime.realtime PREEMPT_RT profile is only supported on Linux targets".to_string(),
        ));
    }
    status
}

#[cfg(target_os = "linux")]
fn record_scheduler_observation(
    status: &mut LinuxRtRuntimeStatus,
    config: &LinuxRtConfig,
    policy: LinuxRtSchedulerPolicy,
    priority: i32,
) {
    if policy != config.scheduler {
        status.errors.push(SmolStr::new(format!(
            "scheduler policy mismatch: expected {}, observed {}",
            config.scheduler.as_str(),
            policy.as_str()
        )));
    }
    if priority != i32::from(config.priority) {
        status.errors.push(SmolStr::new(format!(
            "scheduler priority mismatch: expected {}, observed {}",
            config.priority, priority
        )));
    }
}

#[cfg(target_os = "linux")]
fn detect_linux_realtime_kernel() -> Option<bool> {
    read_sys_kernel_realtime()
        .or_else(read_boot_config_realtime)
        .or_else(read_proc_version_realtime)
}

#[cfg(target_os = "linux")]
fn read_sys_kernel_realtime() -> Option<bool> {
    fs::read_to_string("/sys/kernel/realtime")
        .ok()
        .map(|value| value.trim() == "1")
}

#[cfg(target_os = "linux")]
fn read_boot_config_realtime() -> Option<bool> {
    let release = fs::read_to_string("/proc/sys/kernel/osrelease").ok()?;
    let path = format!("/boot/config-{}", release.trim());
    let body = fs::read_to_string(path).ok()?;
    parse_boot_config_realtime(&body)
}

#[cfg(target_os = "linux")]
fn parse_boot_config_realtime(body: &str) -> Option<bool> {
    for line in body.lines().map(str::trim) {
        if line == "CONFIG_PREEMPT_RT=y" {
            return Some(true);
        }
        if line == "# CONFIG_PREEMPT_RT is not set" {
            return Some(false);
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn read_proc_version_realtime() -> Option<bool> {
    let body = fs::read_to_string("/proc/version").ok()?;
    parse_proc_version_realtime(&body)
}

#[cfg(target_os = "linux")]
fn parse_proc_version_realtime(body: &str) -> Option<bool> {
    let upper = body.to_ascii_uppercase();
    if upper.contains("PREEMPT_RT") {
        return Some(true);
    }
    if upper.contains(" PREEMPT ") {
        return Some(false);
    }
    None
}

#[cfg(target_os = "linux")]
fn read_sched_policy_and_priority() -> Result<(LinuxRtSchedulerPolicy, i32), String> {
    let body = fs::read_to_string("/proc/thread-self/stat")
        .map_err(|err| format!("read /proc/thread-self/stat: {err}"))?;
    parse_proc_stat_scheduler(&body)
}

#[cfg(target_os = "linux")]
fn read_process_memory_lock_kb() -> Result<u64, String> {
    let body = fs::read_to_string("/proc/self/status")
        .map_err(|err| format!("read /proc/self/status: {err}"))?;
    read_proc_status_kb(&body, "VmLck").ok_or_else(|| "missing `VmLck` field".to_string())
}

#[cfg(target_os = "linux")]
fn parse_proc_stat_scheduler(body: &str) -> Result<(LinuxRtSchedulerPolicy, i32), String> {
    let (_, rest) = body
        .rsplit_once(')')
        .ok_or_else(|| "missing closing process name delimiter".to_string())?;
    let fields = rest.split_whitespace().collect::<Vec<_>>();
    let rt_priority = fields
        .get(37)
        .ok_or_else(|| "missing `rt_priority` field".to_string())?
        .parse::<i32>()
        .map_err(|err| format!("parse rt_priority: {err}"))?;
    let policy = fields
        .get(38)
        .ok_or_else(|| "missing `policy` field".to_string())?
        .parse::<i32>()
        .map_err(|err| format!("parse policy: {err}"))?;
    Ok((
        LinuxRtSchedulerPolicy::from_linux_policy_raw(policy),
        rt_priority,
    ))
}

#[cfg(target_os = "linux")]
fn read_proc_status_kb(body: &str, key: &str) -> Option<u64> {
    body.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim() != key {
            return None;
        }
        value
            .split_whitespace()
            .next()
            .and_then(|field| field.parse::<u64>().ok())
    })
}

#[cfg(target_os = "linux")]
fn cpuset_to_vec(cpuset: &rustix::thread::CpuSet) -> Vec<usize> {
    (0..rustix::thread::CpuSet::MAX_CPU)
        .filter(|cpu| cpuset.is_set(*cpu))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_policy_parser_accepts_documented_values() {
        assert_eq!(
            LinuxRtSchedulerPolicy::parse("fifo").expect("fifo"),
            LinuxRtSchedulerPolicy::Fifo
        );
        assert_eq!(
            LinuxRtSchedulerPolicy::parse("rr").expect("rr"),
            LinuxRtSchedulerPolicy::RoundRobin
        );
        assert_eq!(
            LinuxRtSchedulerPolicy::parse("other").expect("other"),
            LinuxRtSchedulerPolicy::Other
        );
    }

    #[test]
    fn realtime_config_requires_realtime_scheduler_when_enabled() {
        let config = LinuxRtConfig {
            enabled: true,
            ..LinuxRtConfig::default()
        };
        let err = config
            .validate()
            .expect_err("enabled config should reject scheduler=other");
        assert!(err
            .to_string()
            .contains("runtime.realtime.scheduler must be 'fifo' or 'rr'"));
    }

    #[test]
    fn realtime_config_rejects_duplicate_affinity_entries() {
        let config = LinuxRtConfig {
            enabled: true,
            scheduler: LinuxRtSchedulerPolicy::Fifo,
            priority: 70,
            cpu_affinity: vec![2, 2],
            ..LinuxRtConfig::default()
        };
        let err = config
            .validate()
            .expect_err("duplicate cpu affinity should fail");
        assert!(err
            .to_string()
            .contains("runtime.realtime.cpu_affinity entries must be unique"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn proc_stat_scheduler_parser_reads_user_space_rt_priority() {
        let mut fields = vec!["0"; 39];
        fields[37] = "70";
        fields[38] = "1";
        let sample = format!("1234 (trust-runtime) {}", fields.join(" "));
        assert_eq!(
            parse_proc_stat_scheduler(&sample).expect("parse stat"),
            (LinuxRtSchedulerPolicy::Fifo, 70)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn proc_status_parser_reads_vm_lock_field() {
        let sample = "\
Name:\ttrust-runtime\n\
VmLck:\t     128 kB\n";
        assert_eq!(read_proc_status_kb(sample, "VmLck"), Some(128));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn boot_config_parser_distinguishes_rt_and_non_rt() {
        assert_eq!(
            parse_boot_config_realtime("CONFIG_PREEMPT_RT=y\n"),
            Some(true)
        );
        assert_eq!(
            parse_boot_config_realtime("# CONFIG_PREEMPT_RT is not set\n"),
            Some(false)
        );
        assert_eq!(parse_boot_config_realtime("CONFIG_PREEMPT=y\n"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn proc_version_parser_distinguishes_rt_and_non_rt() {
        assert_eq!(
            parse_proc_version_realtime(
                "Linux version 6.12.0-rt #1 SMP PREEMPT_RT Debian 1:6.12.0-1"
            ),
            Some(true)
        );
        assert_eq!(
            parse_proc_version_realtime(
                "Linux version 6.12.62+rpt-rpi-2712 #1 SMP PREEMPT Debian 1:6.12.62-1+rpt1"
            ),
            Some(false)
        );
        assert_eq!(
            parse_proc_version_realtime("Linux version 6.12.0-custom #1 SMP Debian"),
            None
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn scheduler_observation_accepts_matching_fifo_priority() {
        let config = LinuxRtConfig {
            enabled: true,
            scheduler: LinuxRtSchedulerPolicy::Fifo,
            priority: 70,
            ..LinuxRtConfig::default()
        };
        let mut status = LinuxRtRuntimeStatus::from_config(config.clone());
        record_scheduler_observation(&mut status, &config, LinuxRtSchedulerPolicy::Fifo, 70);
        assert!(status.errors.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn strict_hook_returns_error_when_profile_verification_fails() {
        use rustix::thread::CpuSet;

        let config = LinuxRtConfig {
            enabled: true,
            scheduler: LinuxRtSchedulerPolicy::Fifo,
            priority: 70,
            cpu_affinity: vec![CpuSet::MAX_CPU],
            strict: true,
            ..LinuxRtConfig::default()
        };
        let sink = Arc::new(Mutex::new(LinuxRtRuntimeStatus::from_config(
            config.clone(),
        )));
        let hook = make_thread_init_hook(config, sink.clone());

        let err = hook().expect_err("strict profile should fail on invalid affinity");
        assert!(err
            .to_string()
            .contains("linux rt profile verification failed"));
        let guard = sink.lock().expect("status sink lock");
        assert!(!guard.errors.is_empty());
    }
}

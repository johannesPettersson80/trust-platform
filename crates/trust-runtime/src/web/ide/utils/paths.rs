use super::*;
use std::time::UNIX_EPOCH;

pub(in crate::web::ide) fn normalize_source_path(path: &str) -> Result<String, IdeError> {
    let normalized = normalize_workspace_path(path, false)?;
    if !normalized.to_ascii_lowercase().ends_with(".st") {
        return Err(IdeError::new(
            IdeErrorKind::InvalidInput,
            "only .st files are allowed",
        ));
    }
    Ok(normalized)
}

pub(in crate::web::ide) fn normalize_workspace_file_path(path: &str) -> Result<String, IdeError> {
    normalize_workspace_path(path, false)
}

pub(in crate::web::ide) fn normalize_workspace_path(
    path: &str,
    allow_root: bool,
) -> Result<String, IdeError> {
    let trimmed = path.trim();
    if trimmed.is_empty() && !allow_root {
        return Err(IdeError::new(
            IdeErrorKind::InvalidInput,
            "workspace path is required",
        ));
    }
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if trimmed.contains('\\') {
        return Err(IdeError::new(
            IdeErrorKind::Forbidden,
            "workspace paths must use relative forward-slash syntax",
        ));
    }

    let raw = Path::new(trimmed);
    if raw.is_absolute() || raw.has_root() {
        return Err(IdeError::new(
            IdeErrorKind::Forbidden,
            "absolute workspace paths are not allowed",
        ));
    }

    let mut parts = Vec::new();
    for component in raw.components() {
        match component {
            Component::Normal(value) => {
                let text = value.to_string_lossy();
                if text.starts_with('.') {
                    return Err(IdeError::new(
                        IdeErrorKind::Forbidden,
                        "hidden workspace paths are not allowed",
                    ));
                }
                parts.push(text.to_string());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(IdeError::new(
                    IdeErrorKind::Forbidden,
                    "workspace path escapes project root",
                ));
            }
        }
    }

    if parts.is_empty() {
        if allow_root {
            return Ok(String::new());
        }
        return Err(IdeError::new(
            IdeErrorKind::InvalidInput,
            "workspace path is required",
        ));
    }

    Ok(parts.join("/"))
}

pub(in crate::web::ide) fn normalize_project_root(path: &str) -> Result<PathBuf, IdeError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(IdeError::new(
            IdeErrorKind::InvalidInput,
            "project root path is required",
        ));
    }
    let raw = PathBuf::from(trimmed);
    let absolute = if raw.is_absolute() {
        raw
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(raw)
    };
    Ok(absolute)
}

pub(in crate::web::ide) fn pathbuf_to_display(path: PathBuf) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(in crate::web::ide) fn closest_existing_parent(
    mut cursor: Option<&Path>,
    canonical_root: &Path,
) -> Result<PathBuf, IdeError> {
    while let Some(path) = cursor {
        if path.exists() {
            return path
                .canonicalize()
                .map_err(|_| IdeError::new(IdeErrorKind::NotFound, "workspace folder not found"));
        }
        cursor = path.parent();
    }
    Ok(canonical_root.to_path_buf())
}

pub(in crate::web::ide) fn source_fingerprint(path: &Path) -> Result<SourceFingerprint, IdeError> {
    let metadata = std::fs::metadata(path)
        .map_err(|_| IdeError::new(IdeErrorKind::NotFound, "source file not found"))?;
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_millis());
    Ok(SourceFingerprint {
        size_bytes: metadata.len(),
        modified_ms,
    })
}

pub(in crate::web::ide) fn read_source_with_limit(
    path: &Path,
    max_file_bytes: usize,
) -> Result<String, IdeError> {
    let text = std::fs::read_to_string(path)
        .map_err(|_| IdeError::new(IdeErrorKind::NotFound, "source file not found"))?;
    if text.len() > max_file_bytes {
        return Err(IdeError::new(
            IdeErrorKind::TooLarge,
            format!(
                "source file exceeds limit ({} > {} bytes)",
                text.len(),
                max_file_bytes
            ),
        ));
    }
    Ok(text)
}

pub(in crate::web::ide) fn project_template_source(template: &str, _project_name: &str) -> String {
    match template {
        "blinky" => {
            "PROGRAM Main\n  VAR\n    blink : BOOL := FALSE;\n  END_VAR\n\n  blink := NOT blink;\nEND_PROGRAM\n"
                .to_string()
        }
        "pid_loop" => {
            "PROGRAM Main\n  VAR\n    setpoint : REAL := 50.0;\n    process_value : REAL := 0.0;\n    output : REAL := 0.0;\n    pid : PID_FB;\n  END_VAR\n\n  pid(Setpoint := setpoint, ProcessValue := process_value);\n  output := pid.Output;\nEND_PROGRAM\n"
                .to_string()
        }
        "motor_control" => {
            "PROGRAM Main\n  VAR\n    start_cmd : BOOL := FALSE;\n    stop_cmd : BOOL := FALSE;\n    safety_trip : BOOL := FALSE;\n    motor : MOTOR_FB;\n  END_VAR\n\n  motor(StartCmd := start_cmd, StopCmd := stop_cmd, SafetyTrip := safety_trip);\nEND_PROGRAM\n"
                .to_string()
        }
        _ => "PROGRAM Main\nVAR\nEND_VAR\nEND_PROGRAM\n".to_string(),
    }
}

pub(in crate::web::ide) fn project_template_extra_sources(template: &str) -> Vec<(String, String)> {
    match template {
        "pid_loop" => vec![(
            "pid_fb.st".to_string(),
            "FUNCTION_BLOCK PID_FB
VAR_INPUT
  Setpoint : REAL;
  ProcessValue : REAL;
END_VAR
VAR_OUTPUT
  Output : REAL;
END_VAR
VAR
  Integral : REAL := 0.0;
  Kp : REAL := 1.0;
  Ki : REAL := 0.1;
END_VAR

Integral := Integral + (Setpoint - ProcessValue);
Output := (Kp * (Setpoint - ProcessValue)) + (Ki * Integral);
END_FUNCTION_BLOCK
"
            .to_string(),
        )],
        "motor_control" => vec![
            (
                "motor_fb.st".to_string(),
                "FUNCTION_BLOCK MOTOR_FB
VAR_INPUT
  StartCmd : BOOL;
  StopCmd : BOOL;
  SafetyTrip : BOOL;
END_VAR
VAR_OUTPUT
  MotorRunning : BOOL;
  Fault : BOOL;
END_VAR

IF SafetyTrip THEN
  Fault := TRUE;
  MotorRunning := FALSE;
ELSIF StopCmd THEN
  MotorRunning := FALSE;
ELSIF StartCmd THEN
  MotorRunning := TRUE;
END_IF;
END_FUNCTION_BLOCK
"
                .to_string(),
            ),
            (
                "safety.st".to_string(),
                "FUNCTION_BLOCK SAFETY_FB
VAR_INPUT
  EStop : BOOL;
END_VAR
VAR_OUTPUT
  Trip : BOOL;
END_VAR

Trip := EStop;
END_FUNCTION_BLOCK
"
                .to_string(),
            ),
        ],
        _ => Vec::new(),
    }
}

pub(in crate::web::ide) fn compile_glob_pattern(
    raw: Option<&str>,
    field: &str,
) -> Result<Option<Pattern>, IdeError> {
    let Some(trimmed) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    Pattern::new(trimmed).map(Some).map_err(|err| {
        IdeError::new(
            IdeErrorKind::InvalidInput,
            format!("invalid {field} glob pattern '{trimmed}': {err}"),
        )
    })
}

#[cfg(test)]
#[path = "paths/contract_tests.rs"]
mod contract_tests;

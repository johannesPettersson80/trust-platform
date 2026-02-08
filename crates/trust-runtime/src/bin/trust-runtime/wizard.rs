//! Guided wizard for creating a new runtime bundle.

use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use smol_str::SmolStr;
use trust_runtime::bundle_template::{
    build_io_config_auto, render_io_toml, render_runtime_toml, IoConfigTemplate, IoDriverTemplate,
};
use trust_runtime::harness::{CompileSession, SourceFile};

use crate::git::git_init;
use crate::prompt::{prompt_choice, prompt_path, prompt_string, prompt_u64, prompt_yes_no};
use crate::run;
use crate::style;

pub fn run_wizard(path: Option<PathBuf>, start: bool) -> anyhow::Result<()> {
    println!(
        "{}",
        style::accent("Welcome to trueST! Creating a new PLC project...")
    );
    let bundle_path = create_bundle(path)?;
    println!(
        "{}",
        style::success(format!("✓ Project created at {}", bundle_path.display()))
    );
    println!(
        "{}",
        style::accent(format!(
            "Next: trust-runtime --project {}",
            bundle_path.display()
        ))
    );
    println!("(Project folder contains runtime.toml, io.toml, program.stbc)");
    let should_start = if start {
        true
    } else {
        prompt_yes_no("Start the runtime now?", true)?
    };
    if should_start {
        run::run_runtime(
            Some(bundle_path),
            None,
            None,
            "cold".to_string(),
            false,
            true,
            run::ConsoleMode::Auto,
            false,
            false,
            1,
        )?;
    }
    Ok(())
}

pub fn create_bundle(path: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("wizard requires an interactive terminal");
    }
    let cwd = std::env::current_dir()?;
    let root = prompt_path("Project directory", path.as_ref().unwrap_or(&cwd).as_path())?;
    fs::create_dir_all(&root)?;
    ensure_gitignore(&root)?;

    let runtime_path = root.join("runtime.toml");
    let io_path = root.join("io.toml");
    let program_path = root.join("program.stbc");
    if (runtime_path.exists() || io_path.exists() || program_path.exists())
        && !prompt_yes_no("Project files already exist. Overwrite?", false)?
    {
        anyhow::bail!("Aborted. Tip: choose a new project folder or re-run the wizard.");
    }

    let project_name = prompt_string("Project name", "trust-plc")?;
    if prompt_yes_no("Initialize git repository?", true)? {
        if let Err(err) = git_init(&root) {
            eprintln!("{err}; continuing without a repository.");
        }
    }
    let resource_name = SmolStr::new(format_resource_name(&project_name));
    let cycle_ms = prompt_u64("Cycle time (ms)", 100)?;
    let driver_default = default_driver();
    let driver = prompt_choice(
        "I/O driver",
        &["loopback", "gpio", "simulated", "modbus-tcp", "mqtt"],
        driver_default.as_str(),
    )?;
    let io_config = build_io_config(&driver)?;

    let runtime_text = render_runtime_toml(&resource_name, cycle_ms);
    let io_text = render_io_toml(&io_config);

    fs::write(&runtime_path, runtime_text)?;
    fs::write(&io_path, io_text)?;

    let sources_dir = root.join("sources");
    fs::create_dir_all(&sources_dir)?;
    let main_text = render_main_source();
    let config_text = render_config_source(&resource_name, cycle_ms);
    fs::write(sources_dir.join("main.st"), &main_text)?;
    fs::write(sources_dir.join("config.st"), &config_text)?;

    let session = CompileSession::from_sources(vec![
        SourceFile::with_path("config.st", config_text),
        SourceFile::with_path("main.st", main_text),
    ]);
    let bytecode = session.build_bytecode_bytes()?;
    fs::write(&program_path, bytecode)?;

    Ok(root)
}

pub fn create_bundle_auto(path: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let root = path.unwrap_or(cwd);
    fs::create_dir_all(&root)?;
    ensure_gitignore(&root)?;

    let runtime_path = root.join("runtime.toml");
    let io_path = root.join("io.toml");
    let program_path = root.join("program.stbc");
    let sources_dir = root.join("sources");

    let (resource_name, cycle_ms) = if runtime_path.exists() {
        let runtime = trust_runtime::config::RuntimeConfig::load(&runtime_path)?;
        (
            SmolStr::new(runtime.resource_name.to_string()),
            runtime.cycle_interval.as_millis() as u64,
        )
    } else {
        let project_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("trust-plc");
        let resource_name = SmolStr::new(format_resource_name(project_name));
        let cycle_ms = 100;
        let runtime_text = render_runtime_toml(&resource_name, cycle_ms);
        fs::write(&runtime_path, runtime_text)?;
        (resource_name, cycle_ms)
    };

    let system_io = trust_runtime::config::load_system_io_config()?;
    if !io_path.exists() && system_io.is_none() {
        let driver = default_auto_driver();
        let io_config = build_io_config_auto(&driver)?;
        let io_text = render_io_toml(&io_config);
        fs::write(&io_path, io_text)?;
    }

    if !sources_dir.exists() {
        fs::create_dir_all(&sources_dir)?;
        write_default_sources(&sources_dir, &resource_name, cycle_ms)?;
    } else {
        upgrade_legacy_sources(&sources_dir, &resource_name, cycle_ms)?;
    }

    if !program_path.exists() {
        remove_legacy_global_io(&sources_dir)?;
        let sources = collect_sources(&sources_dir)?;
        let sources = if sources.is_empty() {
            write_default_sources(&sources_dir, &resource_name, cycle_ms)?;
            collect_sources(&sources_dir)?
        } else {
            sources
        };
        let session = CompileSession::from_sources(sources);
        let bytecode = session.build_bytecode_bytes()?;
        fs::write(&program_path, bytecode)?;
    }

    Ok(root)
}

pub(crate) fn default_resource_name(root: &Path) -> SmolStr {
    let project_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("trust-plc");
    SmolStr::new(format_resource_name(project_name))
}

pub(crate) fn write_runtime_toml(
    runtime_path: &Path,
    resource_name: &SmolStr,
    cycle_ms: u64,
) -> anyhow::Result<()> {
    let runtime_text = render_runtime_toml(resource_name, cycle_ms);
    fs::write(runtime_path, runtime_text)?;
    Ok(())
}

pub(crate) fn write_io_toml_with_driver(io_path: &Path, driver: &str) -> anyhow::Result<()> {
    let io_config = build_io_config_auto(driver)?;
    let io_text = render_io_toml(&io_config);
    fs::write(io_path, io_text)?;
    Ok(())
}

pub(crate) fn remove_io_toml(io_path: &Path) -> anyhow::Result<()> {
    if io_path.exists() {
        fs::remove_file(io_path)?;
    }
    Ok(())
}

const DEFAULT_GITIGNORE: &str = "\
# Build artifacts\n\
/target/\n\
/program.stbc\n\
*.stbc\n\
\n\
# Runtime data\n\
/retain.bin\n\
/retain.store\n\
*.log\n\
\n\
# Editor/OS noise\n\
.DS_Store\n\
*.swp\n\
*.swo\n\
";

fn ensure_gitignore(root: &Path) -> anyhow::Result<()> {
    let path = root.join(".gitignore");
    if path.exists() {
        return Ok(());
    }
    fs::write(path, DEFAULT_GITIGNORE)?;
    Ok(())
}

fn build_io_config(driver: &str) -> anyhow::Result<IoConfigTemplate> {
    if driver.eq_ignore_ascii_case("gpio") {
        let safe_state = vec![("%QX0.0".to_string(), "FALSE".to_string())];
        let input_line = prompt_u64("GPIO input line for %IX0.0", 17)?;
        let output_line = prompt_u64("GPIO output line for %QX0.0", 27)?;
        let mut params = toml::map::Map::new();
        params.insert("backend".into(), toml::Value::String("sysfs".to_string()));
        let inputs = toml::Value::Array(vec![toml::Value::Table(toml::map::Map::from_iter([
            ("address".into(), toml::Value::String("%IX0.0".to_string())),
            ("line".into(), toml::Value::Integer(input_line as i64)),
        ]))]);
        let outputs = toml::Value::Array(vec![toml::Value::Table(toml::map::Map::from_iter([
            ("address".into(), toml::Value::String("%QX0.0".to_string())),
            ("line".into(), toml::Value::Integer(output_line as i64)),
        ]))]);
        params.insert("inputs".into(), inputs);
        params.insert("outputs".into(), outputs);
        return Ok(IoConfigTemplate {
            drivers: vec![IoDriverTemplate {
                name: "gpio".to_string(),
                params: toml::Value::Table(params),
            }],
            safe_state,
        });
    }
    build_io_config_auto(driver)
}

fn render_main_source() -> String {
    r#"PROGRAM Main
VAR
    Count : INT := 0;
END_VAR

VAR_EXTERNAL
    InSignal : BOOL;
    OutSignal : BOOL;
END_VAR

IF InSignal THEN
    Count := Count + 1;
END_IF;
OutSignal := (Count MOD 2) = 1;
END_PROGRAM
"#
    .to_string()
}

fn render_config_source(resource_name: &SmolStr, cycle_ms: u64) -> String {
    format!(
        "CONFIGURATION Config\nVAR_GLOBAL\n    InSignal AT %IX0.0 : BOOL;\n    OutSignal AT %QX0.0 : BOOL;\nEND_VAR\nRESOURCE {resource_name} ON PLC\n    TASK MainTask (INTERVAL := T#{cycle_ms}ms, PRIORITY := 1);\n    PROGRAM P1 WITH MainTask : Main;\nEND_RESOURCE\nEND_CONFIGURATION\n"
    )
}

fn render_legacy_main_source() -> &'static str {
    r#"PROGRAM Main
VAR
    Count : INT := 0;
END_VAR

IF InSignal THEN
    Count := Count + 1;
END_IF;
OutSignal := (Count MOD 2) = 1;
END_PROGRAM
"#
}

fn render_legacy_config_source(resource_name: &SmolStr, cycle_ms: u64) -> String {
    format!(
        "CONFIGURATION Config\nRESOURCE {resource_name} ON PLC\n    TASK MainTask (INTERVAL := T#{cycle_ms}ms, PRIORITY := 1);\n    PROGRAM P1 WITH MainTask : Main;\nEND_RESOURCE\nEND_CONFIGURATION\n"
    )
}

fn write_default_sources(
    root: &Path,
    resource_name: &SmolStr,
    cycle_ms: u64,
) -> anyhow::Result<()> {
    let main_text = render_main_source();
    let config_text = render_config_source(resource_name, cycle_ms);
    fs::write(root.join("main.st"), main_text)?;
    fs::write(root.join("config.st"), config_text)?;
    Ok(())
}

fn upgrade_legacy_sources(
    root: &Path,
    resource_name: &SmolStr,
    cycle_ms: u64,
) -> anyhow::Result<()> {
    let main_path = root.join("main.st");
    if main_path.is_file() {
        let text = std::fs::read_to_string(&main_path)?;
        if text.trim() == render_legacy_main_source().trim() {
            std::fs::write(&main_path, render_main_source())?;
        }
    }
    let config_path = root.join("config.st");
    if config_path.is_file() {
        let text = std::fs::read_to_string(&config_path)?;
        if text.trim() == render_legacy_config_source(resource_name, cycle_ms).trim() {
            std::fs::write(&config_path, render_config_source(resource_name, cycle_ms))?;
        }
    }
    Ok(())
}

fn collect_sources(root: &Path) -> anyhow::Result<Vec<SourceFile>> {
    let mut files = Vec::new();
    let patterns = ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"];
    for pattern in patterns {
        for entry in glob::glob(&format!("{}/{}", root.display(), pattern))? {
            let path = entry?;
            let path_string = path.display().to_string();
            if files
                .iter()
                .any(|file: &SourceFile| file.path.as_deref() == Some(path_string.as_str()))
            {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            if is_legacy_global_io(&path_string, &text) {
                continue;
            }
            files.push(SourceFile::with_path(path_string, text));
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn remove_legacy_global_io(root: &Path) -> anyhow::Result<()> {
    let path = root.join("io.st");
    if !path.is_file() {
        return Ok(());
    }
    let text = std::fs::read_to_string(&path)?;
    if is_legacy_global_io(path.display().to_string().as_str(), &text) {
        let _ = std::fs::remove_file(&path);
    }
    Ok(())
}

fn is_legacy_global_io(path: &str, text: &str) -> bool {
    if !path.ends_with("io.st") {
        return false;
    }
    text.trim() == legacy_global_io_source().trim()
}

fn legacy_global_io_source() -> &'static str {
    r#"VAR_GLOBAL
    InSignal AT %IX0.0 : BOOL;
    OutSignal AT %QX0.0 : BOOL;
END_VAR
"#
}

fn format_resource_name(project: &str) -> String {
    let trimmed = project.trim();
    if trimmed.is_empty() {
        return "Res".to_string();
    }
    let mut out = String::new();
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        }
    }
    if out.is_empty() {
        "Res".to_string()
    } else {
        out
    }
}

fn default_driver() -> String {
    if trust_runtime::setup::is_raspberry_pi_hint() {
        "gpio".to_string()
    } else {
        "loopback".to_string()
    }
}

fn default_auto_driver() -> String {
    "loopback".to_string()
}

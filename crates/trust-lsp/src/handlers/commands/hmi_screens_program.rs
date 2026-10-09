//! Plan the screen service using parsed ST. The editor applies these texts with the new page.
use super::hmi_files::{refused, single_argument};
use serde::Deserialize;
use serde_json::{json, Value};
use trust_syntax::{SyntaxKind, SyntaxNode};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    file: String,
    text: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    #[serde(default)]
    existing_task: bool,
    #[serde(default)]
    screen_task: Option<String>,
    sources: Vec<Source>,
}
const PROGRAM:&str="// Serves the HMI at the end of its task.\nPROGRAM Screens\nVAR\n    Display : Hmi.Display;\nEND_VAR\n    Display();\nEND_PROGRAM\n";
fn compact(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.text().to_ascii_uppercase())
        .collect()
}
fn name(node: &SyntaxNode) -> Option<String> {
    node.children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .map(|child| child.text().to_string().trim().to_owned())
}

/// Collect sources through the normal project resolver, including unsaved editor contents.
pub(super) fn prepare_project<C: crate::handlers::context::ServerContext>(
    context: &C,
    arguments: Vec<Value>,
) -> Value {
    let collect = || -> Result<Value, String> {
        let selection: super::HmiBindingsCommandArgs =
            single_argument(arguments, super::HMI_SCREENS_PROGRAM_COMMAND)?;
        let root = super::resolve_hmi_bindings_project_root(context, &selection)
            .ok_or("Open a truST project first")?;
        let open = super::hmi_snapshot::paths(&root, &selection.open)?;
        let collected = if root.join("src").is_dir() {
            super::hmi_authoring::sources::collect(&root, &open)?
        } else {
            Vec::new()
        };
        let mut sources = Vec::new();
        for source in collected {
            let Some(path) = source.path.map(std::path::PathBuf::from) else {
                continue;
            };
            let path = if path.is_absolute() {
                path
            } else {
                root.join(path)
            };
            let Ok(relative) = path.strip_prefix(&root) else {
                continue;
            };
            if relative.starts_with("libraries") || relative.starts_with(".trust") {
                continue;
            }
            let text = open
                .get(&path)
                .cloned()
                .or_else(|| {
                    tower_lsp::lsp_types::Url::from_file_path(&path)
                        .ok()
                        .and_then(|uri| context.get_document(&uri))
                        .filter(|document| document.is_open)
                        .map(|document| document.content)
                })
                .unwrap_or(source.text);
            sources.push(json!({"file":relative.to_string_lossy().replace('\\',"/"),"text":text}));
        }
        Ok(prepare(vec![json!({"sources":sources})]))
    };
    collect().unwrap_or_else(refused)
}
pub(super) fn prepare(arguments: Vec<Value>) -> Value {
    let plan = || -> Result<Value, String> {
        let args: Args = single_argument(arguments, super::HMI_SCREENS_PROGRAM_COMMAND)?;
        if args.sources.len() > 2048
            || args
                .sources
                .iter()
                .map(|source| source.text.len())
                .sum::<usize>()
                > 16 * 1024 * 1024
        {
            return Err("The source selection is too large".into());
        }
        let mut program_count = 0usize;
        let mut resources = Vec::new();
        let mut existing_tasks = Vec::new();
        let mut names = std::collections::BTreeSet::new();
        let mut display_programs = Vec::new();
        let mut configured = std::collections::BTreeSet::new();
        for source in &args.sources {
            if !(source.file.to_ascii_lowercase().ends_with(".st")
                || source.file.to_ascii_lowercase().ends_with(".pou"))
                || std::path::Path::new(&source.file)
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_)))
            {
                return Err("Expected project-relative ST source paths".into());
            }
            let parsed = trust_syntax::parser::parse(&source.text);
            if !parsed.errors().is_empty() {
                return Err(format!(
                    "Fix the ST syntax in {} before adding the screen service",
                    source.file
                ));
            }
            let root = parsed.syntax();
            for node in root.descendants() {
                if matches!(
                    node.kind(),
                    SyntaxKind::Program | SyntaxKind::TaskConfig | SyntaxKind::ProgramConfig
                ) {
                    if let Some(name) = name(&node) {
                        names.insert(name.to_ascii_uppercase());
                    }
                }
                if node.kind() == SyntaxKind::TaskConfig
                    && node
                        .children()
                        .filter(|child| child.kind() == SyntaxKind::TaskInit)
                        .any(|init| {
                            init.children().any(|child| {
                                child.kind() == SyntaxKind::Name
                                    && child
                                        .text()
                                        .to_string()
                                        .trim()
                                        .eq_ignore_ascii_case("INTERVAL")
                            })
                        })
                {
                    if let (Some(task), Some(resource)) = (
                        name(&node),
                        node.parent()
                            .filter(|parent| parent.kind() == SyntaxKind::Resource),
                    ) {
                        if let Some(end) = resource
                            .children_with_tokens()
                            .filter_map(|element| element.into_token())
                            .find(|token| token.kind() == SyntaxKind::KwEndResource)
                        {
                            existing_tasks.push((
                                &source.file,
                                &source.text,
                                usize::from(end.text_range().start()),
                                task,
                            ));
                        }
                    }
                }
                if node.kind() == SyntaxKind::Resource {
                    let end = node
                        .children_with_tokens()
                        .filter_map(|element| element.into_token())
                        .find(|token| token.kind() == SyntaxKind::KwEndResource)
                        .ok_or("Resource end is missing")?;
                    resources.push((
                        &source.file,
                        &source.text,
                        usize::from(end.text_range().start()),
                    ));
                }
                if node.kind() == SyntaxKind::Program {
                    program_count += 1;
                    for declaration in node
                        .descendants()
                        .filter(|child| child.kind() == SyntaxKind::VarDecl)
                    {
                        if !declaration.descendants().any(|child| {
                            child.kind() == SyntaxKind::TypeRef && compact(&child) == "HMI.DISPLAY"
                        }) {
                            continue;
                        }
                        let Some(instance) = name(&declaration) else {
                            continue;
                        };
                        if !node.descendants().any(|call| {
                            call.kind() == SyntaxKind::CallExpr
                                && call.children().next().is_some_and(|target| {
                                    compact(&target) == instance.to_ascii_uppercase()
                                })
                        }) {
                            return Err(format!("Call {instance}() in {} so the existing Hmi.Display serves the page",source.file));
                        }
                        if let Some(program) = name(&node) {
                            display_programs.push(program.to_ascii_uppercase());
                        }
                    }
                }
            }
            for (_, kind) in super::control_links::program_types(&[source.text.as_str()]) {
                configured.insert(kind.to_ascii_uppercase());
            }
        }
        if !display_programs.is_empty() {
            if !resources.is_empty()
                && !display_programs
                    .iter()
                    .any(|program| configured.contains(program))
            {
                return Err("Assign the existing Hmi.Display program to a runtime task".into());
            }
            return Ok(json!({"ok":true,"creates":[],"changes":[]}));
        }
        if names.contains("SCREENS")
            || names.contains("SCREENTASK")
            || args.sources.iter().any(|source| {
                // An empty file of that name, left by an unsaved earlier run, holds nothing.
                source.file.eq_ignore_ascii_case("src/screens.st") && !source.text.trim().is_empty()
            })
        {
            return Err("Screens or ScreenTask is already used. Rename that declaration or add Hmi.Display to an existing task.".into());
        }
        if resources.len() > 1 && !args.existing_task {
            return Err("This project has several resources. Add Hmi.Display to the intended runtime task before creating the page.".into());
        }
        if args.existing_task {
            if existing_tasks.is_empty() {
                return Err("Configure a periodic PLC task before creating the screen service; no task period is invented".into());
            }
            let task_key = |(file, _text, end, task): &(&String, &String, usize, String)| {
                format!("{file}:{end}:{task}")
            };
            let selected = if let Some(key) = args.screen_task.as_ref() {
                existing_tasks
                    .iter()
                    .find(|task| task_key(task) == *key)
                    .ok_or("The selected task changed; choose it again")?
            } else if existing_tasks.len() == 1 {
                &existing_tasks[0]
            } else {
                let choices: Vec<_> = existing_tasks
                    .iter()
                    .map(
                        |entry| json!({"id":task_key(entry),"label":entry.3,"description":entry.0}),
                    )
                    .collect();
                return Ok(json!({"ok":false,"needsScreenTask":choices}));
            };
            let (file, text, end, task) = selected;
            let start = text[..*end].rfind('\n').map_or(0, |index| index + 1);
            let insert = if text[start..*end].trim().is_empty() {
                start
            } else {
                *end
            };
            let next = format!(
                "{}    PROGRAM Screens WITH {task} : Screens;\n{}",
                &text[..insert],
                &text[insert..]
            );
            return Ok(
                json!({"ok":true,"creates":[{"file":"src/screens.st","text":PROGRAM}],"changes":[{"file":file,"expected":text,"text":next}]}),
            );
        }
        let mut creates = vec![json!({"file":"src/screens.st","text":PROGRAM})];
        if program_count == 0 {
            if names.contains("MAIN")
                || names.contains("CONTROLTASK")
                || args
                    .sources
                    .iter()
                    .any(|source| source.file.eq_ignore_ascii_case("src/plant.st"))
            {
                return Err("A Main or ControlTask declaration already exists; choose the intended control program in ST".into());
            }
            let mut control = String::from("PROGRAM Main\nEND_PROGRAM\n");
            if resources.is_empty() {
                control.push_str("\nCONFIGURATION PlantConfiguration\nRESOURCE Cpu ON PLC\n    TASK ControlTask (INTERVAL := T#50ms, PRIORITY := 1);\n    PROGRAM Main WITH ControlTask : Main;\n    TASK ScreenTask (INTERVAL := T#250ms, PRIORITY := 10);\n    PROGRAM Screens WITH ScreenTask : Screens;\nEND_RESOURCE\nEND_CONFIGURATION\n");
            }
            creates.push(json!({"file":"src/plant.st","text":control}));
        }
        let mut changes = Vec::new();
        if let Some((file, text, end)) = resources.first() {
            let line_start = text[..*end].rfind('\n').map_or(0, |index| index + 1);
            let insert = if text[line_start..*end].trim().is_empty() {
                line_start
            } else {
                *end
            };
            let screen_task="    TASK ScreenTask (INTERVAL := T#250ms, PRIORITY := 10);\n    PROGRAM Screens WITH ScreenTask : Screens;\n";
            let added = if program_count == 0 {
                format!("    TASK ControlTask (INTERVAL := T#50ms, PRIORITY := 1);\n    PROGRAM Main WITH ControlTask : Main;\n{screen_task}")
            } else {
                screen_task.to_owned()
            };
            changes.push(json!({"file":file,"expected":text,"text":format!("{}{}{}",&text[..insert],added,&text[insert..])}));
        }
        Ok(json!({"ok":true,"creates":creates,"changes":changes}))
    };
    plan().unwrap_or_else(refused)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn task_insertion_ignores_comments_and_preserves_manual_program() {
        let text="PROGRAM Control\nEND_PROGRAM\nCONFIGURATION C\nRESOURCE Cpu ON PLC\nTASK Fast(INTERVAL:=T#10ms,PRIORITY:=1);\nPROGRAM Main WITH Fast : Control;\n(* END_RESOURCE Hmi.Display *)\nEND_RESOURCE\nEND_CONFIGURATION\n";
        let result = prepare(vec![
            json!({"sources":[{"file":"src/main.st","text":text}]}),
        ]);
        assert_eq!(result["ok"], true, "{result}");
        let changed = result["changes"][0]["text"].as_str().unwrap();
        assert!(changed.contains("(* END_RESOURCE Hmi.Display *)\n    TASK ScreenTask"));
        assert!(changed.starts_with("PROGRAM Control\nEND_PROGRAM\n"));
    }
    #[test]
    fn display_declaration_without_a_call_is_not_silently_accepted() {
        let result = prepare(vec![
            json!({"sources":[{"file":"src/main.st","text":"PROGRAM Main\nVAR\n Screen : Hmi.Display;\nEND_VAR\nEND_PROGRAM\n"}]}),
        ]);
        assert_eq!(result["ok"], false, "{result}");
    }
    #[test]
    fn an_empty_screens_file_left_by_an_unsaved_run_is_created_again() {
        let main = "PROGRAM Main\nEND_PROGRAM\nCONFIGURATION C\nRESOURCE Cpu ON PLC\nTASK Fast(INTERVAL:=T#10ms,PRIORITY:=1);\nPROGRAM Main WITH Fast : Main;\nEND_RESOURCE\nEND_CONFIGURATION\n";
        let result = prepare(vec![json!({"sources":[
            {"file":"src/main.st","text":main},
            {"file":"src/screens.st","text":""},
        ]})]);
        assert_eq!(result["ok"], true, "{result}");
        assert_eq!(result["creates"][0]["file"], "src/screens.st", "{result}");
        let taken = prepare(vec![json!({"sources":[
            {"file":"src/main.st","text":main},
            {"file":"src/screens.st","text":"PROGRAM Other\nEND_PROGRAM\n"},
        ]})]);
        assert_eq!(
            taken["ok"], false,
            "a screens.st with its own code is still refused: {taken}"
        );
    }
    #[test]
    fn existing_called_display_is_not_duplicated() {
        let result = prepare(vec![
            json!({"sources":[{"file":"src/main.st","text":"PROGRAM Main\nVAR\n Screen : Hmi.Display;\nEND_VAR\nScreen();\nEND_PROGRAM\n"}]}),
        ]);
        assert_eq!(result["ok"], true, "{result}");
        assert_eq!(result["creates"], json!([]));
    }
}

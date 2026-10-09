//! The literal period of each configured program's task, from the parsed configuration
//! (specification 39, §10.3 item 3). Nothing is evaluated.

/// Retain a literal task period as source, without evaluating customer expressions. The
/// normal Build command remains responsible for IEC range/constant validation.
pub(super) fn task_intervals(sources: &[super::super::LoadedSource]) -> Vec<(String, String)> {
    use trust_syntax::{parser::parse, SyntaxKind as K};
    let mut result = Vec::new();
    for source in sources {
        let root = parse(&source.text).syntax();
        for scope in root
            .descendants()
            .filter(|n| matches!(n.kind(), K::Resource | K::Configuration))
        {
            let mut tasks = std::collections::BTreeMap::new();
            for task in scope.children().filter(|n| n.kind() == K::TaskConfig) {
                let Some(name) = task.children().find(|n| n.kind() == K::Name) else {
                    continue;
                };
                let Some(init) = task.children().find(|n| n.kind() == K::TaskInit) else {
                    continue;
                };
                let mut key = String::new();
                for child in init.children() {
                    if child.kind() == K::Name {
                        key = child.text().to_string().trim().to_ascii_uppercase();
                    } else if key == "INTERVAL" {
                        let value = child.text().to_string().trim().to_string();
                        // Expressions whose scope would change must not be copied into a PROGRAM.
                        if matches!(child.kind(), K::Literal) {
                            tasks
                                .insert(name.text().to_string().trim().to_ascii_uppercase(), value);
                        }
                        key.clear();
                    }
                }
            }
            for program in scope.children().filter(|n| n.kind() == K::ProgramConfig) {
                let names: Vec<_> = program
                    .children()
                    .filter(|n| n.kind() == K::Name)
                    .map(|n| n.text().to_string().trim().to_string())
                    .collect();
                if names.len() == 2 {
                    if let Some(interval) = tasks.get(&names[1].to_ascii_uppercase()) {
                        result.push((names[0].clone(), interval.clone()));
                    }
                }
            }
        }
    }
    result
}

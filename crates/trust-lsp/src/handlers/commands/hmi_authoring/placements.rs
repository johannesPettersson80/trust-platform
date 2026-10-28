//! Editor placement checks extend native field fitting with unbound generic panels.
use trust_runtime::hmi::{
    binding_checks::BindingProblem,
    bundle::HmiBundle,
    page_files::{HmiPage, PLACED_FACEPLATE},
};
use trust_runtime::memory::VariableStorage;

pub(crate) fn check(
    page: &HmiPage,
    bundle: &HmiBundle,
    storage: &VariableStorage,
) -> Vec<BindingProblem> {
    let mut problems = Vec::new();
    for object in &page.objects {
        if object.kind != PLACED_FACEPLATE {
            continue;
        }
        let problem = |property: &str, message: String| BindingProblem {
            object: object.id.clone(),
            property: Some(property.into()),
            message,
            point: None,
        };
        let Some(name) = &object.faceplate else {
            problems.push(problem("faceplate", "Choose a panel definition".into()));
            continue;
        };
        let Some((file, bytes)) = super::bundle::faceplate_text(bundle, name) else {
            problems.push(problem(
                "faceplate",
                format!("Panel {name} is missing from this project and its libraries"),
            ));
            continue;
        };
        let text = String::from_utf8_lossy(bytes);
        let model = match super::documents::parse("faceplate", file, &text) {
            Ok(model) => model,
            Err(errors) => {
                problems.extend(errors.into_iter().map(|e| problem("faceplate", e.message)));
                continue;
            }
        };
        let Some(instance) = &object.instance else {
            if super::documents::requires_instance(&model) {
                problems.push(problem(
                    "instance",
                    format!("Choose the equipment instance for {name}"),
                ));
            }
            continue;
        };
        if let Ok(panel) = super::documents::faceplate(file, &text) {
            problems.extend(
                trust_runtime::hmi::faceplate_fit::misfits(
                    &panel,
                    &super::bundle::pairs_of(bundle, name),
                    instance,
                    storage,
                )
                .into_iter()
                .map(|message| problem("instance", message)),
            );
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generic_panels_need_no_instance_but_equipment_panels_do() {
        let text="format=1\ntitle='Main'\n[[object]]\nid='sidebar'\ntype='faceplate'\nfaceplate='navigation'\nat=[0,0]\nsize=[200,720]\n";
        let (page, _) = super::super::documents::page("main", text, false).unwrap();
        let runtime =
            trust_runtime::harness::CompileSession::from_source("PROGRAM Main END_PROGRAM")
                .declarations()
                .unwrap();
        let mut bundle = HmiBundle::default();
        bundle.files.insert(
            "hmi/panels/navigation.toml".into(),
            b"format=1\ntitle='Navigation'\nsize=[200,720]\n".to_vec(),
        );
        assert!(check(&page, &bundle, runtime.storage()).is_empty());
        bundle.files.insert(
            "hmi/panels/navigation.toml".into(),
            b"format=1\ntitle='Pump'\nsize=[200,720]\nsettings=true\n".to_vec(),
        );
        let problems = check(&page, &bundle, runtime.storage());
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].property.as_deref(), Some("instance"));
    }
}

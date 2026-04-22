fn parse_codesys_project_structure(root: roxmltree::Node<'_, '_>) -> CodesysProjectStructureMap {
    fn walk_object(
        node: roxmltree::Node<'_, '_>,
        parent_path: &[String],
        object_paths_by_id: &mut BTreeMap<String, Vec<String>>,
        object_paths_by_name: &mut BTreeMap<String, Vec<Vec<String>>>,
        object_count: &mut usize,
    ) {
        let name = attribute_ci_any(&node, &["Name", "name"])
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "Object".to_string());
        let mut path = parent_path.to_vec();
        path.push(sanitize_path_segment(&name, "Object"));
        *object_count += 1;

        if let Some(object_id) = attribute_ci_any(&node, &["ObjectId", "objectId", "id"])
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            object_paths_by_id.insert(object_id, path.clone());
        }

        let key = name.trim().to_ascii_lowercase();
        object_paths_by_name
            .entry(key)
            .or_default()
            .push(path.clone());

        for child in node
            .children()
            .filter(|entry| is_element_named_ci(*entry, "Object"))
        {
            walk_object(
                child,
                &path,
                object_paths_by_id,
                object_paths_by_name,
                object_count,
            );
        }
    }

    let mut object_paths_by_id = BTreeMap::new();
    let mut object_paths_by_name: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
    let mut object_count = 0usize;

    for data in root
        .descendants()
        .filter(|node| is_element_named_ci(*node, "data"))
    {
        let Some(name) = attribute_ci(data, "name") else {
            continue;
        };
        if !name.to_ascii_lowercase().contains("projectstructure")
            && !name.eq_ignore_ascii_case(CODESYS_PROJECTSTRUCTURE_DATA_NAME)
        {
            continue;
        }
        for project_structure in data
            .descendants()
            .filter(|node| is_element_named_ci(*node, "ProjectStructure"))
        {
            for object in project_structure
                .children()
                .filter(|child| is_element_named_ci(*child, "Object"))
            {
                walk_object(
                    object,
                    &[],
                    &mut object_paths_by_id,
                    &mut object_paths_by_name,
                    &mut object_count,
                );
            }
        }
    }

    let unique_object_paths_by_name = object_paths_by_name
        .into_iter()
        .filter_map(|(name, paths)| {
            if paths.len() == 1 {
                Some((name, paths[0].clone()))
            } else {
                None
            }
        })
        .collect();

    CodesysProjectStructureMap {
        object_paths_by_id,
        unique_object_paths_by_name,
        object_count,
    }
}

fn resolve_codesys_folder_segments_for_node(
    node: roxmltree::Node<'_, '_>,
    fallback_name: &str,
    project_structure_map: &CodesysProjectStructureMap,
) -> Vec<String> {
    if let Some(object_id) = extract_object_id_from_node(node) {
        if let Some(path) = project_structure_map.object_paths_by_id.get(&object_id) {
            if path.len() >= 2 {
                return path[..path.len() - 1].to_vec();
            }
        }
    }

    let name_key = fallback_name.trim().to_ascii_lowercase();
    if let Some(path) = project_structure_map
        .unique_object_paths_by_name
        .get(&name_key)
    {
        if path.len() >= 2 {
            return path[..path.len() - 1].to_vec();
        }
    }

    Vec::new()
}

#[derive(Debug, Clone)]
struct InterfaceVarSection {
    header: String,
    declarations: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct PouInterfaceMetadata {
    function_return_type: Option<String>,
    header_hint: Option<String>,
    sections: Vec<InterfaceVarSection>,
}

impl PouInterfaceMetadata {
    fn has_details(&self) -> bool {
        self.function_return_type.is_some()
            || self.header_hint.is_some()
            || self
                .sections
                .iter()
                .any(|section| !section.declarations.is_empty())
    }
}

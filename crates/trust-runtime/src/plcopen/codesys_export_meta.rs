fn append_indent(xml: &mut String, spaces: usize) {
    for _ in 0..spaces {
        xml.push(' ');
    }
}

fn append_task_xml(xml: &mut String, task: &TaskDecl, indent: usize) {
    append_indent(xml, indent);
    xml.push_str(&format!("<task name=\"{}\"", escape_xml_attr(&task.name)));
    if let Some(interval) = &task.interval {
        xml.push_str(&format!(" interval=\"{}\"", escape_xml_attr(interval)));
    }
    if let Some(single) = &task.single {
        xml.push_str(&format!(" single=\"{}\"", escape_xml_attr(single)));
    }
    if let Some(priority) = &task.priority {
        xml.push_str(&format!(" priority=\"{}\"", escape_xml_attr(priority)));
    }
    xml.push_str(" />\n");
}

fn append_program_instance_xml(xml: &mut String, program: &ProgramBindingDecl, indent: usize) {
    append_indent(xml, indent);
    xml.push_str(&format!(
        "<pouInstance name=\"{}\" typeName=\"{}\"",
        escape_xml_attr(&program.instance_name),
        escape_xml_attr(&program.type_name)
    ));
    if let Some(task_name) = &program.task_name {
        xml.push_str(&format!(" task=\"{}\"", escape_xml_attr(task_name)));
    }
    xml.push_str(" />\n");
}

fn source_folder_segments_for_codesys(path: &Path) -> Vec<String> {
    let mut segments = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if segments
        .first()
        .is_some_and(|segment| segment.eq_ignore_ascii_case("src"))
    {
        segments.remove(0);
    }
    if segments
        .first()
        .is_some_and(|segment| segment.eq_ignore_ascii_case("Application"))
    {
        segments.remove(0);
    }
    if !segments.is_empty() {
        segments.pop();
    }

    segments
        .into_iter()
        .map(|segment| sanitize_path_segment(&segment, "folder"))
        .collect()
}

fn deterministic_codesys_object_id(kind: &str, name: &str, folder_segments: &[String]) -> String {
    let seed = format!(
        "{kind}:{}:{}",
        folder_segments.join("/"),
        name.to_ascii_lowercase()
    );
    let h1 = crc32fast::hash(format!("a:{seed}").as_bytes());
    let h2 = crc32fast::hash(format!("b:{seed}").as_bytes());
    let h3 = crc32fast::hash(format!("c:{seed}").as_bytes());
    let h4 = crc32fast::hash(format!("d:{seed}").as_bytes());
    let tail = ((u64::from(h3 & 0xffff)) << 32) | u64::from(h4);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        h1,
        (h2 >> 16) as u16,
        (h2 & 0xffff) as u16,
        (h3 >> 16) as u16,
        tail & 0x0000_ffff_ffff_ffff
    )
}

fn build_codesys_project_structure_tree(
    entries: &[CodesysExportObjectEntry],
) -> CodesysProjectObjectNode {
    #[derive(Default)]
    struct NodeBuilder {
        object_id: String,
        children: BTreeMap<String, NodeBuilder>,
    }

    fn insert_entry(node: &mut NodeBuilder, key_segments: &[String], object_id: &str) {
        if key_segments.is_empty() {
            return;
        }
        let key = key_segments[0].clone();
        let child = node.children.entry(key).or_default();
        if key_segments.len() == 1 {
            child.object_id = object_id.to_string();
            return;
        }
        insert_entry(child, &key_segments[1..], object_id);
    }

    fn finalize_node(
        name: &str,
        builder: NodeBuilder,
        parent_path: &[String],
    ) -> CodesysProjectObjectNode {
        let mut path = parent_path.to_vec();
        path.push(name.to_string());
        let children = builder
            .children
            .into_iter()
            .map(|(child_name, child_builder)| finalize_node(&child_name, child_builder, &path))
            .collect::<Vec<_>>();
        let object_id = if builder.object_id.is_empty() {
            deterministic_codesys_object_id("folder", name, &path)
        } else {
            builder.object_id
        };
        CodesysProjectObjectNode {
            name: name.to_string(),
            object_id,
            children,
        }
    }

    let mut root_builder = NodeBuilder {
        object_id: deterministic_codesys_object_id("root", "Application", &[]),
        children: BTreeMap::new(),
    };

    for entry in entries {
        let mut path = entry.folder_segments.clone();
        path.extend(entry.parent_segments.clone());
        path.push(entry.name.clone());
        insert_entry(&mut root_builder, &path, &entry.object_id);
    }

    finalize_node("Application", root_builder, &[])
}

fn count_project_structure_nodes(node: &CodesysProjectObjectNode) -> usize {
    1 + node
        .children
        .iter()
        .map(count_project_structure_nodes)
        .sum::<usize>()
}

fn build_codesys_export_metadata(
    declarations: &[PouDecl],
    global_var_lists: &[GlobalVarDecl],
    warnings: &mut Vec<String>,
) -> CodesysExportMetadata {
    let pou_entries = declarations
        .iter()
        .map(|decl| {
            let source_path = PathBuf::from(&decl.source);
            let folder_segments = source_folder_segments_for_codesys(&source_path);
            let object_id = deterministic_codesys_object_id("pou", &decl.name, &folder_segments);
            (
                decl.clone(),
                CodesysExportObjectEntry {
                    name: decl.name.clone(),
                    object_id,
                    folder_segments,
                    parent_segments: Vec::new(),
                },
            )
        })
        .collect::<Vec<_>>();

    let method_entries = declarations
        .iter()
        .flat_map(|decl| {
            let source_path = PathBuf::from(&decl.source);
            let folder_segments = source_folder_segments_for_codesys(&source_path);
            decl.methods.iter().map(move |method| {
                let mut object_path = folder_segments.clone();
                object_path.push(decl.name.clone());
                let object_id =
                    deterministic_codesys_object_id("method", &method.name, &object_path);
                (
                    method.clone(),
                    CodesysExportObjectEntry {
                        name: method.name.clone(),
                        object_id,
                        folder_segments: folder_segments.clone(),
                        parent_segments: vec![decl.name.clone()],
                    },
                )
            })
        })
        .collect::<Vec<_>>();

    let mut global_entries = Vec::new();
    for decl in global_var_lists {
        let folder_segments = source_folder_segments_for_codesys(&decl.source_path);
        let object_id = deterministic_codesys_object_id("gvl", &decl.name, &folder_segments);
        if decl.variables.is_empty() {
            warnings.push(format!(
                "{}:{} global list '{}' has no parseable declaration entries; exporting plaintext-only metadata",
                decl.source, decl.line, decl.name
            ));
        }
        global_entries.push((
            decl.clone(),
            CodesysExportObjectEntry {
                name: decl.name.clone(),
                object_id,
                folder_segments,
                parent_segments: Vec::new(),
            },
        ));
    }

    let mut all_entries = pou_entries
        .iter()
        .map(|(_, entry)| entry.clone())
        .collect::<Vec<_>>();
    all_entries.extend(method_entries.iter().map(|(_, entry)| entry.clone()));
    all_entries.extend(global_entries.iter().map(|(_, entry)| entry.clone()));
    let project_structure_root = build_codesys_project_structure_tree(&all_entries);
    let exported_project_structure_nodes = count_project_structure_nodes(&project_structure_root);

    let mut folder_paths = all_entries
        .iter()
        .filter(|entry| !entry.folder_segments.is_empty())
        .map(|entry| entry.folder_segments.join("/"))
        .collect::<HashSet<_>>();
    if !all_entries.is_empty() {
        folder_paths.insert("Application".to_string());
    }
    let exported_folder_paths = folder_paths.len();

    CodesysExportMetadata {
        global_var_lists: global_entries,
        pou_entries,
        method_entries,
        project_structure_root,
        exported_project_structure_nodes,
        exported_folder_paths,
    }
}

fn append_codesys_variable_xml(
    xml: &mut String,
    variable: &InterfaceVariableDecl,
    indent: usize,
    warnings: &mut Vec<String>,
    source: &str,
    line: usize,
) {
    append_indent(xml, indent);
    xml.push_str(&format!(
        "<variable name=\"{}\">\n",
        escape_xml_attr(&variable.name)
    ));
    append_indent(xml, indent + 2);
    xml.push_str("<type>\n");
    if let Some(type_xml) = type_expression_to_plcopen_base_type_xml(&variable.type_expr) {
        for line_text in type_xml.lines() {
            append_indent(xml, indent + 4);
            xml.push_str(line_text);
            xml.push('\n');
        }
    } else {
        warnings.push(format!(
            "{source}:{line} unsupported interface type '{}' in '{}'; exported as derived",
            variable.type_expr, variable.name
        ));
        append_indent(xml, indent + 4);
        xml.push_str(&format!(
            "<derived name=\"{}\" />\n",
            escape_xml_attr(&variable.type_expr)
        ));
    }
    append_indent(xml, indent + 2);
    xml.push_str("</type>\n");
    if let Some(initial_value) = variable
        .initial_value
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        append_indent(xml, indent + 2);
        xml.push_str("<initialValue>\n");
        append_indent(xml, indent + 4);
        xml.push_str(&format!(
            "<simpleValue value=\"{}\" />\n",
            escape_xml_attr(&initial_value)
        ));
        append_indent(xml, indent + 2);
        xml.push_str("</initialValue>\n");
    }
    append_indent(xml, indent);
    xml.push_str("</variable>\n");
}

fn append_codesys_method_xml(
    xml: &mut String,
    method: &CodesysMethodDecl,
    object_entry: &CodesysExportObjectEntry,
    indent: usize,
    warnings: &mut Vec<String>,
) {
    append_indent(xml, indent);
    xml.push_str(&format!(
        "<Method name=\"{}\" ObjectId=\"{}\">\n",
        escape_xml_attr(&method.name),
        escape_xml_attr(&object_entry.object_id)
    ));
    append_indent(xml, indent + 2);
    xml.push_str("<interface>\n");
    if let Some(return_type) = method
        .return_type
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        append_indent(xml, indent + 4);
        xml.push_str("<returnType>\n");
        if let Some(type_xml) = type_expression_to_plcopen_base_type_xml(&return_type) {
            for line_text in type_xml.lines() {
                append_indent(xml, indent + 6);
                xml.push_str(line_text);
                xml.push('\n');
            }
        } else {
            warnings.push(format!(
                "{}:{} unsupported method return type '{}' in '{}.{}'; exported as derived",
                method.source, method.line, return_type, method.owner_name, method.name
            ));
            append_indent(xml, indent + 6);
            xml.push_str(&format!("<derived name=\"{}\" />\n", escape_xml_attr(&return_type)));
        }
        append_indent(xml, indent + 4);
        xml.push_str("</returnType>\n");
    }
    for section in &method.sections {
        append_indent(xml, indent + 4);
        xml.push_str(&format!("<{}", section.xml_name));
        if section.constant {
            xml.push_str(" constant=\"true\"");
        }
        if section.retain {
            xml.push_str(" retain=\"true\"");
        }
        if section.nonretain {
            xml.push_str(" nonretain=\"true\"");
        }
        if section.persistent {
            xml.push_str(" persistent=\"true\"");
        }
        if section.nonpersistent {
            xml.push_str(" nonpersistent=\"true\"");
        }
        xml.push_str(">\n");
        for variable in &section.variables {
            append_codesys_variable_xml(
                xml,
                variable,
                indent + 6,
                warnings,
                &method.source,
                method.line,
            );
        }
        append_indent(xml, indent + 4);
        xml.push_str(&format!("</{}>\n", section.xml_name));
    }
    append_indent(xml, indent + 2);
    xml.push_str("</interface>\n");
    append_indent(xml, indent + 2);
    xml.push_str("<body>\n");
    append_indent(xml, indent + 4);
    xml.push_str("<ST>\n");
    append_indent(xml, indent + 6);
    xml.push_str(&format!(
        "<xhtml xmlns=\"http://www.w3.org/1999/xhtml\">{}</xhtml>\n",
        escape_xml_attr(method.body.trim())
    ));
    append_indent(xml, indent + 4);
    xml.push_str("</ST>\n");
    append_indent(xml, indent + 2);
    xml.push_str("</body>\n");
    append_indent(xml, indent + 2);
    xml.push_str("<addData>\n");
    append_indent(xml, indent + 4);
    xml.push_str(&format!(
        "<data name=\"{}\" handleUnknown=\"implementation\">\n",
        CODESYS_INTERFACE_PLAINTEXT_DATA_NAME
    ));
    append_indent(xml, indent + 6);
    xml.push_str("<InterfaceAsPlainText>\n");
    append_indent(xml, indent + 8);
    xml.push_str(&format!(
        "<xhtml xmlns=\"http://www.w3.org/1999/xhtml\">{}</xhtml>\n",
        escape_xml_attr(&method.interface_plaintext)
    ));
    append_indent(xml, indent + 6);
    xml.push_str("</InterfaceAsPlainText>\n");
    append_indent(xml, indent + 4);
    xml.push_str("</data>\n");
    append_indent(xml, indent + 4);
    xml.push_str(&format!(
        "<data name=\"{}\" handleUnknown=\"discard\">\n",
        CODESYS_OBJECT_ID_DATA_NAME
    ));
    append_indent(xml, indent + 6);
    xml.push_str(&format!("<ObjectId>{}</ObjectId>\n", escape_xml_attr(&object_entry.object_id)));
    append_indent(xml, indent + 4);
    xml.push_str("</data>\n");
    append_indent(xml, indent + 2);
    xml.push_str("</addData>\n");
    append_indent(xml, indent);
    xml.push_str("</Method>\n");
}

fn append_codesys_project_structure_node(
    xml: &mut String,
    node: &CodesysProjectObjectNode,
    indent: usize,
) {
    append_indent(xml, indent);
    xml.push_str(&format!(
        "<Object Name=\"{}\" ObjectId=\"{}\"",
        escape_xml_attr(&node.name),
        escape_xml_attr(&node.object_id)
    ));
    if node.children.is_empty() {
        xml.push_str(" />\n");
        return;
    }

    xml.push_str(">\n");
    for child in &node.children {
        append_codesys_project_structure_node(xml, child, indent + 2);
    }
    append_indent(xml, indent);
    xml.push_str("</Object>\n");
}

fn append_codesys_export_add_data(
    xml: &mut String,
    metadata: &CodesysExportMetadata,
    warnings: &mut Vec<String>,
) {
    if metadata.pou_entries.is_empty() && metadata.global_var_lists.is_empty() {
        return;
    }

    if !metadata.pou_entries.is_empty() || !metadata.global_var_lists.is_empty() {
        xml.push_str(&format!(
            "    <data name=\"{}\" handleUnknown=\"implementation\">\n",
            CODESYS_APPLICATION_DATA_NAME
        ));
        xml.push_str("      <resource name=\"Application\">\n");
        for (decl, object_entry) in &metadata.global_var_lists {
            xml.push_str(&format!(
                "        <globalVars name=\"{}\">\n",
                escape_xml_attr(&decl.name)
            ));
            for variable in &decl.variables {
                xml.push_str(&format!(
                    "          <variable name=\"{}\">\n",
                    escape_xml_attr(&variable.name)
                ));
                xml.push_str("            <type>\n");
                if let Some(type_xml) =
                    type_expression_to_plcopen_base_type_xml(&variable.type_expr)
                {
                    for line in type_xml.lines() {
                        xml.push_str("              ");
                        xml.push_str(line);
                        xml.push('\n');
                    }
                } else {
                    warnings.push(format!(
                        "{}:{} unsupported global type '{}' in '{}'; exported as derived",
                        decl.source, decl.line, variable.type_expr, variable.name
                    ));
                    xml.push_str(&format!(
                        "              <derived name=\"{}\" />\n",
                        escape_xml_attr(&variable.type_expr)
                    ));
                }
                xml.push_str("            </type>\n");
                if let Some(initial_value) = variable
                    .initial_value
                    .as_ref()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                {
                    xml.push_str("            <initialValue>\n");
                    xml.push_str(&format!(
                        "              <simpleValue value=\"{}\" />\n",
                        escape_xml_attr(&initial_value)
                    ));
                    xml.push_str("            </initialValue>\n");
                }
                xml.push_str("          </variable>\n");
            }
            xml.push_str("          <addData>\n");
            xml.push_str(&format!(
                "            <data name=\"{}\" handleUnknown=\"implementation\">\n",
                CODESYS_INTERFACE_PLAINTEXT_DATA_NAME
            ));
            xml.push_str("              <InterfaceAsPlainText>\n");
            xml.push_str(&format!(
                "                <xhtml xmlns=\"http://www.w3.org/1999/xhtml\">{}</xhtml>\n",
                escape_xml_attr(&decl.body)
            ));
            xml.push_str("              </InterfaceAsPlainText>\n");
            xml.push_str("            </data>\n");
            xml.push_str(&format!(
                "            <data name=\"{}\" handleUnknown=\"discard\">\n",
                CODESYS_OBJECT_ID_DATA_NAME
            ));
            xml.push_str(&format!(
                "              <ObjectId>{}</ObjectId>\n",
                escape_xml_attr(&object_entry.object_id)
            ));
            xml.push_str("            </data>\n");
            xml.push_str("          </addData>\n");
            xml.push_str("        </globalVars>\n");
        }
        if !metadata.pou_entries.is_empty() {
            xml.push_str("        <addData>\n");
            for (decl, object_entry) in &metadata.pou_entries {
                let methods = metadata
                    .method_entries
                    .iter()
                    .filter(|(method, _)| method.owner_name.eq_ignore_ascii_case(&decl.name))
                    .collect::<Vec<_>>();
                xml.push_str(&format!(
                    "          <data name=\"{}\" handleUnknown=\"implementation\">\n",
                    CODESYS_POU_DATA_NAME
                ));
                xml.push_str(&format!(
                    "            <pou name=\"{}\" pouType=\"{}\">\n",
                    escape_xml_attr(&decl.name),
                    decl.pou_type.as_xml()
                ));
                xml.push_str("              <body>\n");
                xml.push_str("                <ST>\n");
                xml.push_str(&format!(
                    "                  <xhtml xmlns=\"http://www.w3.org/1999/xhtml\">{}</xhtml>\n",
                    escape_xml_attr(&decl.body)
                ));
                xml.push_str("                </ST>\n");
                xml.push_str("              </body>\n");
                xml.push_str("              <addData>\n");
                xml.push_str(&format!(
                    "                <data name=\"{}\" handleUnknown=\"implementation\">\n",
                    CODESYS_INTERFACE_PLAINTEXT_DATA_NAME
                ));
                xml.push_str("                  <InterfaceAsPlainText>\n");
                xml.push_str(&format!(
                    "                    <xhtml xmlns=\"http://www.w3.org/1999/xhtml\">{}</xhtml>\n",
                    escape_xml_attr(&decl.body)
                ));
                xml.push_str("                  </InterfaceAsPlainText>\n");
                xml.push_str("                </data>\n");
                if !methods.is_empty() {
                    xml.push_str(&format!(
                        "                <data name=\"{}\" handleUnknown=\"implementation\">\n",
                        CODESYS_METHOD_DATA_NAME
                    ));
                    for (method, method_entry) in methods {
                        append_codesys_method_xml(xml, method, method_entry, 18, warnings);
                    }
                    xml.push_str("                </data>\n");
                }
                xml.push_str(&format!(
                    "                <data name=\"{}\" handleUnknown=\"discard\">\n",
                    CODESYS_OBJECT_ID_DATA_NAME
                ));
                xml.push_str(&format!(
                    "                  <ObjectId>{}</ObjectId>\n",
                    escape_xml_attr(&object_entry.object_id)
                ));
                xml.push_str("                </data>\n");
                xml.push_str("              </addData>\n");
                xml.push_str("            </pou>\n");
                xml.push_str("          </data>\n");
            }
            xml.push_str("        </addData>\n");
        }
        xml.push_str("      </resource>\n");
        xml.push_str("    </data>\n");
    }

    xml.push_str(&format!(
        "    <data name=\"{}\" handleUnknown=\"discard\">\n",
        CODESYS_PROJECTSTRUCTURE_DATA_NAME
    ));
    xml.push_str("      <ProjectStructure>\n");
    append_codesys_project_structure_node(xml, &metadata.project_structure_root, 8);
    xml.push_str("      </ProjectStructure>\n");
    xml.push_str("    </data>\n");
}

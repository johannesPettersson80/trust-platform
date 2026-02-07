//! PLCopen XML interchange (strict subset for ST projects).

#![allow(missing_docs)]

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use trust_syntax::parser;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

const PLCOPEN_NAMESPACE: &str = "http://www.plcopen.org/xml/tc6_0200";
const PROFILE_NAME: &str = "trust-st-strict-v1";
const SOURCE_MAP_DATA_NAME: &str = "trust.sourceMap";
const VENDOR_EXT_DATA_NAME: &str = "trust.vendorExtensions";
const VENDOR_EXTENSION_HOOK_FILE: &str = "plcopen.vendor-extensions.xml";
const IMPORTED_VENDOR_EXTENSION_FILE: &str = "plcopen.vendor-extensions.imported.xml";

#[derive(Debug, Clone, Serialize)]
pub struct PlcopenProfile {
    pub namespace: &'static str,
    pub profile: &'static str,
    pub version: &'static str,
    pub strict_subset: Vec<&'static str>,
    pub unsupported_nodes: Vec<&'static str>,
    pub source_mapping: &'static str,
    pub vendor_extension_hook: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlcopenExportReport {
    pub output_path: PathBuf,
    pub source_map_path: PathBuf,
    pub pou_count: usize,
    pub source_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlcopenImportReport {
    pub project_root: PathBuf,
    pub written_sources: Vec<PathBuf>,
    pub imported_pous: usize,
    pub warnings: Vec<String>,
    pub unsupported_nodes: Vec<String>,
    pub preserved_vendor_extensions: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct LoadedSource {
    path: PathBuf,
    text: String,
}

#[derive(Debug, Clone)]
struct PouDecl {
    name: String,
    pou_type: PlcopenPouType,
    body: String,
    source: String,
    line: usize,
}

#[derive(Debug, Clone, Copy)]
enum PlcopenPouType {
    Program,
    Function,
    FunctionBlock,
}

impl PlcopenPouType {
    fn as_xml(self) -> &'static str {
        match self {
            Self::Program => "program",
            Self::Function => "function",
            Self::FunctionBlock => "functionBlock",
        }
    }

    fn from_xml(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "program" => Some(Self::Program),
            "function" => Some(Self::Function),
            "functionblock" => Some(Self::FunctionBlock),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SourceMapEntry {
    name: String,
    pou_type: String,
    source: String,
    line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SourceMapPayload {
    profile: String,
    namespace: String,
    entries: Vec<SourceMapEntry>,
}

pub fn supported_profile() -> PlcopenProfile {
    PlcopenProfile {
        namespace: PLCOPEN_NAMESPACE,
        profile: PROFILE_NAME,
        version: "TC6 XML v2.0 (strict subset)",
        strict_subset: vec![
            "project/fileHeader/contentHeader",
            "types/pous/pou[pouType=program|function|functionBlock]",
            "pou/body/ST plain-text bodies",
            "addData/data[name=trust.sourceMap|trust.vendorExtensions]",
        ],
        unsupported_nodes: vec![
            "dataTypes",
            "instances/configurations/resources",
            "graphical bodies (FBD/LD/SFC)",
            "vendor-specific nodes (preserved via hooks, not interpreted)",
        ],
        source_mapping: "Export writes deterministic source-map sidecar JSON and embeds trust.sourceMap in addData.",
        vendor_extension_hook:
            "Import preserves unknown addData/vendor nodes to plcopen.vendor-extensions.imported.xml; export re-injects plcopen.vendor-extensions.xml.",
    }
}

pub fn export_project_to_xml(
    project_root: &Path,
    output_path: &Path,
) -> anyhow::Result<PlcopenExportReport> {
    let sources_root = project_root.join("sources");
    if !sources_root.is_dir() {
        anyhow::bail!(
            "invalid project folder '{}': missing sources/ directory",
            project_root.display()
        );
    }

    let sources = load_sources(project_root, &sources_root)?;
    if sources.is_empty() {
        anyhow::bail!("no ST sources found under {}", sources_root.display());
    }

    let mut warnings = Vec::new();
    let mut declarations = Vec::new();

    for source in &sources {
        let (mut declared, mut source_warnings) = extract_pou_declarations(source);
        declarations.append(&mut declared);
        warnings.append(&mut source_warnings);
    }

    if declarations.is_empty() {
        anyhow::bail!(
            "no PLCopen-compatible POU declarations discovered (supported: PROGRAM/FUNCTION/FUNCTION_BLOCK)"
        );
    }

    declarations.sort_by(|left, right| {
        left.pou_type
            .as_xml()
            .cmp(right.pou_type.as_xml())
            .then(left.name.cmp(&right.name))
            .then(left.source.cmp(&right.source))
    });

    let source_map = SourceMapPayload {
        profile: PROFILE_NAME.to_string(),
        namespace: PLCOPEN_NAMESPACE.to_string(),
        entries: declarations
            .iter()
            .map(|decl| SourceMapEntry {
                name: decl.name.clone(),
                pou_type: decl.pou_type.as_xml().to_string(),
                source: decl.source.clone(),
                line: decl.line,
            })
            .collect(),
    };
    let source_map_json = serde_json::to_string_pretty(&source_map)?;

    let project_name = project_root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("project");
    let generated_at = now_iso8601();

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str(&format!(
        "<project xmlns=\"{}\" profile=\"{}\">\n",
        PLCOPEN_NAMESPACE, PROFILE_NAME
    ));
    xml.push_str(&format!(
        "  <fileHeader companyName=\"truST\" productName=\"trust-runtime\" productVersion=\"{}\" creationDateTime=\"{}\"/>\n",
        escape_xml_attr(env!("CARGO_PKG_VERSION")),
        escape_xml_attr(&generated_at)
    ));
    xml.push_str(&format!(
        "  <contentHeader name=\"{}\"/>\n",
        escape_xml_attr(project_name)
    ));
    xml.push_str("  <types>\n");
    xml.push_str("    <pous>\n");

    for decl in &declarations {
        xml.push_str(&format!(
            "      <pou name=\"{}\" pouType=\"{}\">\n",
            escape_xml_attr(&decl.name),
            decl.pou_type.as_xml()
        ));
        xml.push_str("        <body>\n");
        xml.push_str("          <ST><![CDATA[");
        xml.push_str(&escape_cdata(&decl.body));
        xml.push_str("]]></ST>\n");
        xml.push_str("        </body>\n");
        xml.push_str("      </pou>\n");
    }

    xml.push_str("    </pous>\n");
    xml.push_str("  </types>\n");
    xml.push_str("  <addData>\n");
    xml.push_str(&format!(
        "    <data name=\"{}\" handleUnknown=\"implementation\"><text><![CDATA[{}]]></text></data>\n",
        SOURCE_MAP_DATA_NAME,
        escape_cdata(&source_map_json)
    ));

    let vendor_hook_path = project_root.join(VENDOR_EXTENSION_HOOK_FILE);
    if vendor_hook_path.is_file() {
        let vendor_text = std::fs::read_to_string(&vendor_hook_path).with_context(|| {
            format!(
                "failed to read vendor extension hook '{}'",
                vendor_hook_path.display()
            )
        })?;
        xml.push_str(&format!(
            "    <data name=\"{}\" handleUnknown=\"implementation\"><text><![CDATA[{}]]></text></data>\n",
            VENDOR_EXT_DATA_NAME,
            escape_cdata(&vendor_text)
        ));
    }
    xml.push_str("  </addData>\n");
    xml.push_str("</project>\n");

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create PLCopen output directory '{}'",
                parent.display()
            )
        })?;
    }

    std::fs::write(output_path, xml)
        .with_context(|| format!("failed to write '{}'", output_path.display()))?;

    let source_map_path = output_path.with_extension("source-map.json");
    std::fs::write(&source_map_path, format!("{}\n", source_map_json)).with_context(|| {
        format!(
            "failed to write source-map sidecar '{}'",
            source_map_path.display()
        )
    })?;

    Ok(PlcopenExportReport {
        output_path: output_path.to_path_buf(),
        source_map_path,
        pou_count: declarations.len(),
        source_count: sources.len(),
        warnings,
    })
}

pub fn import_xml_to_project(
    xml_path: &Path,
    project_root: &Path,
) -> anyhow::Result<PlcopenImportReport> {
    let xml_text = std::fs::read_to_string(xml_path)
        .with_context(|| format!("failed to read PLCopen XML '{}'", xml_path.display()))?;
    let document = roxmltree::Document::parse(&xml_text)
        .with_context(|| format!("failed to parse PLCopen XML '{}'", xml_path.display()))?;

    let root = document.root_element();
    if root.tag_name().name() != "project" {
        anyhow::bail!(
            "invalid PLCopen XML: expected root <project>, found <{}>",
            root.tag_name().name()
        );
    }

    let mut warnings = Vec::new();
    let mut unsupported_nodes = Vec::new();
    let mut written_sources = Vec::new();
    let mut seen_files = HashSet::new();

    if let Some(namespace) = root.tag_name().namespace() {
        if namespace != PLCOPEN_NAMESPACE {
            warnings.push(format!(
                "unexpected namespace '{}'; expected '{}'",
                namespace, PLCOPEN_NAMESPACE
            ));
        }
    }

    inspect_unsupported_structure(root, &mut unsupported_nodes, &mut warnings);

    let source_map = parse_embedded_source_map(root);

    let sources_root = project_root.join("sources");
    std::fs::create_dir_all(&sources_root)
        .with_context(|| format!("failed to create '{}'", sources_root.display()))?;

    for pou in root
        .descendants()
        .filter(|node| is_element_named(*node, "pou"))
    {
        let Some(name) = pou.attribute("name") else {
            warnings.push("skipping <pou> without name attribute".to_string());
            continue;
        };
        let Some(pou_type_raw) = pou.attribute("pouType") else {
            warnings.push(format!("skipping pou '{}': missing pouType", name));
            continue;
        };
        let Some(_pou_type) = PlcopenPouType::from_xml(pou_type_raw) else {
            warnings.push(format!(
                "skipping pou '{}': unsupported pouType '{}'",
                name, pou_type_raw
            ));
            unsupported_nodes.push(format!("pouType:{}", pou_type_raw));
            continue;
        };

        let Some(st_node) = pou
            .children()
            .find(|child| is_element_named(*child, "body"))
            .and_then(|body| {
                body.children()
                    .find(|candidate| is_element_named(*candidate, "ST"))
            })
        else {
            warnings.push(format!("skipping pou '{}': missing body/ST", name));
            continue;
        };

        let body = st_node.text().map(str::trim).unwrap_or_default();
        if body.is_empty() {
            warnings.push(format!("skipping pou '{}': empty ST body", name));
            continue;
        }

        let mut file_name = sanitize_filename(name);
        if file_name.is_empty() {
            file_name = "unnamed".to_string();
        }
        let mut candidate = sources_root.join(format!("{file_name}.st"));
        let mut duplicate_index = 2usize;
        while !seen_files.insert(candidate.clone()) {
            candidate = sources_root.join(format!("{file_name}_{duplicate_index}.st"));
            duplicate_index += 1;
        }

        let normalized_body = normalize_body_text(body);
        std::fs::write(&candidate, normalized_body)
            .with_context(|| format!("failed to write '{}'", candidate.display()))?;
        written_sources.push(candidate);

        if let Some(entry) = source_map.as_ref().and_then(|map| {
            map.entries
                .iter()
                .find(|entry| entry.name.eq_ignore_ascii_case(name))
        }) {
            warnings.push(format!(
                "source map: pou '{}' originated from {}:{}",
                name, entry.source, entry.line
            ));
        }
    }

    if written_sources.is_empty() {
        anyhow::bail!(
            "no importable PLCopen ST POUs found in {}",
            xml_path.display()
        );
    }

    let preserved_vendor_extensions =
        preserve_vendor_extensions(root, &xml_text, project_root, &mut warnings)?;

    Ok(PlcopenImportReport {
        project_root: project_root.to_path_buf(),
        imported_pous: written_sources.len(),
        written_sources,
        warnings,
        unsupported_nodes,
        preserved_vendor_extensions,
    })
}

fn load_sources(project_root: &Path, sources_root: &Path) -> anyhow::Result<Vec<LoadedSource>> {
    let mut paths = BTreeSet::new();
    for pattern in ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"] {
        let glob_pattern = format!("{}/{}", sources_root.display(), pattern);
        for entry in glob::glob(&glob_pattern)
            .with_context(|| format!("invalid source glob '{}'", glob_pattern))?
        {
            paths.insert(entry?);
        }
    }

    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read source '{}'", path.display()))?;
        let relative = path
            .strip_prefix(project_root)
            .map_or_else(|_| path.clone(), Path::to_path_buf);
        sources.push(LoadedSource {
            path: relative,
            text,
        });
    }
    Ok(sources)
}

fn extract_pou_declarations(source: &LoadedSource) -> (Vec<PouDecl>, Vec<String>) {
    let mut declarations = Vec::new();
    let mut warnings = Vec::new();

    let parsed = parser::parse(&source.text);
    let syntax = parsed.syntax();

    for node in syntax.children() {
        let Some(pou_type) = node_to_pou_type(&node) else {
            if is_unsupported_top_level(&node) {
                let line = line_for_node(&source.text, &node);
                warnings.push(format!(
                    "{}:{} unsupported top-level node '{:?}' skipped for PLCopen strict subset",
                    source.path.display(),
                    line,
                    node.kind()
                ));
            }
            continue;
        };

        let Some(name) = declaration_name(&node) else {
            continue;
        };

        if is_test_pou(&node) {
            let line = line_for_node(&source.text, &node);
            warnings.push(format!(
                "{}:{} test POU '{}' exported as standard '{}'",
                source.path.display(),
                line,
                name,
                pou_type.as_xml()
            ));
        }

        let line = line_for_node(&source.text, &node);
        declarations.push(PouDecl {
            name,
            pou_type,
            body: normalize_body_text(node.text().to_string()),
            source: source.path.display().to_string(),
            line,
        });
    }

    (declarations, warnings)
}

fn node_to_pou_type(node: &SyntaxNode) -> Option<PlcopenPouType> {
    match node.kind() {
        SyntaxKind::Program => Some(PlcopenPouType::Program),
        SyntaxKind::Function => Some(PlcopenPouType::Function),
        SyntaxKind::FunctionBlock => Some(PlcopenPouType::FunctionBlock),
        _ => None,
    }
}

fn is_unsupported_top_level(node: &SyntaxNode) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::Class
            | SyntaxKind::Interface
            | SyntaxKind::Namespace
            | SyntaxKind::Configuration
            | SyntaxKind::TypeDecl
            | SyntaxKind::Action
    )
}

fn is_test_pou(node: &SyntaxNode) -> bool {
    first_non_trivia_token(node).is_some_and(|kind| {
        matches!(
            kind,
            SyntaxKind::KwTestProgram | SyntaxKind::KwTestFunctionBlock
        )
    })
}

fn first_non_trivia_token(node: &SyntaxNode) -> Option<SyntaxKind> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
}

fn declaration_name(node: &SyntaxNode) -> Option<String> {
    node.children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .map(|name| name.text().to_string().trim().to_string())
        .filter(|text| !text.is_empty())
}

fn line_for_node(source: &str, node: &SyntaxNode) -> usize {
    let offset = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .map(|token| usize::from(token.text_range().start()))
        .unwrap_or(0);
    source[..offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn now_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{timestamp}Z")
}

fn escape_xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&apos;")
}

fn escape_cdata(value: &str) -> String {
    value.replace("]]>", "]]]]><![CDATA[>")
}

fn normalize_body_text(text: impl Into<String>) -> String {
    let mut normalized = text.into().replace("\r\n", "\n").replace('\r', "\n");
    if !normalized.ends_with('\n') {
        normalized.push('\n');
    }
    normalized
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
}

fn is_element_named(node: roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.is_element() && node.tag_name().name() == name
}

fn inspect_unsupported_structure(
    root: roxmltree::Node<'_, '_>,
    unsupported_nodes: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    for child in root.children().filter(|child| child.is_element()) {
        let name = child.tag_name().name();
        if !matches!(name, "fileHeader" | "contentHeader" | "types" | "addData") {
            unsupported_nodes.push(name.to_string());
            warnings.push(format!(
                "unsupported PLCopen node '<{}>' preserved as metadata only",
                name
            ));
        }
        if name == "types" {
            for type_child in child.children().filter(|entry| entry.is_element()) {
                let type_name = type_child.tag_name().name();
                if type_name != "pous" {
                    unsupported_nodes.push(format!("types/{}", type_name));
                    warnings.push(format!(
                        "unsupported PLCopen node '<types>/<{}>' skipped (strict subset)",
                        type_name
                    ));
                }
            }
        }
    }
}

fn parse_embedded_source_map(root: roxmltree::Node<'_, '_>) -> Option<SourceMapPayload> {
    let payload = root
        .descendants()
        .find(|node| {
            is_element_named(*node, "data")
                && node
                    .attribute("name")
                    .is_some_and(|name| name == SOURCE_MAP_DATA_NAME)
        })
        .and_then(|node| {
            node.children()
                .find(|child| is_element_named(*child, "text"))
                .and_then(|text| text.text())
        })?;
    serde_json::from_str::<SourceMapPayload>(payload).ok()
}

fn preserve_vendor_extensions(
    root: roxmltree::Node<'_, '_>,
    xml_text: &str,
    project_root: &Path,
    warnings: &mut Vec<String>,
) -> anyhow::Result<Option<PathBuf>> {
    let mut preserved = Vec::new();

    for node in root.descendants().filter(|node| {
        is_element_named(*node, "data")
            && node
                .attribute("name")
                .is_none_or(|name| name != SOURCE_MAP_DATA_NAME)
    }) {
        let range = node.range();
        if let Some(slice) = xml_text.get(range) {
            preserved.push(slice.trim().to_string());
        }
    }

    if preserved.is_empty() {
        return Ok(None);
    }

    let output = project_root.join(IMPORTED_VENDOR_EXTENSION_FILE);
    let mut content = String::from("<vendorExtensions>\n");
    for fragment in preserved {
        content.push_str("  ");
        content.push_str(&fragment);
        content.push('\n');
    }
    content.push_str("</vendorExtensions>\n");
    std::fs::write(&output, content)
        .with_context(|| format!("failed to write '{}'", output.display()))?;
    warnings.push(format!(
        "preserved vendor extension nodes in {}",
        output.display()
    ));
    Ok(Some(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("trust-runtime-{prefix}-{stamp}"));
        std::fs::create_dir_all(&dir).expect("create temp directory");
        dir
    }

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, content).expect("write file");
    }

    fn pou_signatures(xml: &str) -> Vec<(String, String, String)> {
        let doc = roxmltree::Document::parse(xml).expect("parse XML");
        let mut items = doc
            .descendants()
            .filter(|node| is_element_named(*node, "pou"))
            .filter_map(|pou| {
                let name = pou.attribute("name")?.to_string();
                let pou_type = pou.attribute("pouType")?.to_string();
                let body = pou
                    .children()
                    .find(|child| is_element_named(*child, "body"))
                    .and_then(|body| {
                        body.children()
                            .find(|child| is_element_named(*child, "ST"))
                            .and_then(|st| st.text())
                    })
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_string();
                Some((name, pou_type, body))
            })
            .collect::<Vec<_>>();
        items.sort();
        items
    }

    #[test]
    fn round_trip_export_import_export_preserves_pou_subset() {
        let source_project = temp_dir("plcopen-roundtrip-src");
        write(
            &source_project.join("sources/main.st"),
            r#"
PROGRAM Main
VAR
    speed : REAL := 42.5;
END_VAR
END_PROGRAM
"#,
        );
        write(
            &source_project.join("sources/calc.st"),
            r#"
FUNCTION Calc : INT
VAR_INPUT
    A : INT;
END_VAR
Calc := A + 1;
END_FUNCTION
"#,
        );

        let xml_a = source_project.join("build/plcopen.xml");
        let export_a = export_project_to_xml(&source_project, &xml_a).expect("export A");
        assert_eq!(export_a.pou_count, 2);
        assert!(export_a.source_map_path.is_file());

        let import_project = temp_dir("plcopen-roundtrip-import");
        let import = import_xml_to_project(&xml_a, &import_project).expect("import");
        assert_eq!(import.imported_pous, 2);

        let xml_b = import_project.join("build/plcopen.xml");
        let export_b = export_project_to_xml(&import_project, &xml_b).expect("export B");
        assert_eq!(export_b.pou_count, 2);

        let a_text = std::fs::read_to_string(&xml_a).expect("read xml A");
        let b_text = std::fs::read_to_string(&xml_b).expect("read xml B");
        assert_eq!(pou_signatures(&a_text), pou_signatures(&b_text));

        let _ = std::fs::remove_dir_all(source_project);
        let _ = std::fs::remove_dir_all(import_project);
    }

    #[test]
    fn import_reports_unsupported_nodes_and_preserves_vendor_extensions() {
        let project = temp_dir("plcopen-import-unsupported");
        let xml_path = project.join("input.xml");
        write(
            &xml_path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <dataTypes>
      <dataType name="POINT"/>
    </dataTypes>
    <pous>
      <pou name="Main" pouType="program">
        <body>
          <ST><![CDATA[
PROGRAM Main
VAR
  speed : REAL := 10.0;
END_VAR
END_PROGRAM
]]></ST>
        </body>
      </pou>
    </pous>
  </types>
  <addData>
    <data name="vendor.raw"><text><![CDATA[<vendorNode id="1"/>]]></text></data>
  </addData>
</project>
"#,
        );

        let report = import_xml_to_project(&xml_path, &project).expect("import XML");
        assert_eq!(report.imported_pous, 1);
        assert!(!report.unsupported_nodes.is_empty());
        assert!(report
            .unsupported_nodes
            .iter()
            .any(|entry| entry.contains("types/dataTypes")));
        let source = std::fs::read_to_string(&report.written_sources[0]).expect("read source");
        assert!(source.contains("PROGRAM Main"));
        let vendor = report
            .preserved_vendor_extensions
            .expect("vendor extension path");
        let vendor_text = std::fs::read_to_string(vendor).expect("read vendor ext");
        assert!(vendor_text.contains("vendor.raw"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn import_rejects_malformed_xml() {
        let project = temp_dir("plcopen-malformed");
        let xml_path = project.join("broken.xml");
        write(&xml_path, "<project><types><pous><pou>");

        let result = import_xml_to_project(&xml_path, &project);
        assert!(result.is_err(), "malformed XML must return error");

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn export_reinjects_vendor_extension_hook_file() {
        let project = temp_dir("plcopen-export-vendor-hook");
        write(
            &project.join("sources/main.st"),
            r#"
PROGRAM Main
END_PROGRAM
"#,
        );
        write(
            &project.join(VENDOR_EXTENSION_HOOK_FILE),
            r#"<vendorData source="external"/>"#,
        );

        let output = project.join("out/plcopen.xml");
        export_project_to_xml(&project, &output).expect("export XML");
        let text = std::fs::read_to_string(output).expect("read output XML");
        assert!(text.contains(VENDOR_EXT_DATA_NAME));
        assert!(text.contains("vendorData"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn profile_declares_strict_subset_contract() {
        let profile = supported_profile();
        assert_eq!(profile.namespace, PLCOPEN_NAMESPACE);
        assert_eq!(profile.profile, PROFILE_NAME);
        assert!(profile
            .strict_subset
            .iter()
            .any(|item| item.contains("types/pous/pou")));
    }
}

//! Trust-twin topology compiler.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const BUILTIN_LIBRARY: &str = include_str!("../library/v1/components.toml");
const VIEW_CONTRACT_VERSION: u32 = 2;
const RENDERER_CONTRACT_VERSION: u32 = 2;
const TOPOLOGY_HASH_DOMAIN: &str = "trust-twin-topology:v2";
const TOPOLOGY_HASH_HEADER_PREFIX: &str = "# trust-twin-topology-hash:v2:sha256:";

#[derive(Debug, Clone, Copy, PartialEq)]
struct CameraProfile {
    target_x_offset: f64,
    eye_x_offset: f64,
    height: f64,
    distance: f64,
    target_y: f64,
    target_z: f64,
    fov_degrees: f64,
}

impl CameraProfile {
    const DEFAULT: Self = Self {
        target_x_offset: 0.0,
        eye_x_offset: 0.0,
        height: 4.0,
        distance: 8.0,
        target_y: 0.0,
        target_z: 0.0,
        fov_degrees: 45.0,
    };

    const ROBOT_CELL: Self = Self {
        target_x_offset: -0.25,
        eye_x_offset: 0.55,
        height: 4.35,
        distance: 5.05,
        target_y: 0.1,
        target_z: 1.1,
        fov_degrees: 34.0,
    };
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentLibrary {
    pub version: u32,
    pub grid: GridDefaults,
    #[serde(rename = "kind")]
    pub kinds: Vec<ComponentKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GridDefaults {
    pub cell_size: f64,
    pub origin: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentKind {
    pub name: String,
    pub default_mesh_asset: String,
    pub primitive: String,
    pub material: String,
    pub scale: [f64; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_rotation: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<BoundsDefinition>,
    #[serde(default, rename = "surfaces")]
    pub surfaces: Vec<SurfaceDefinition>,
    #[serde(default, rename = "mounts")]
    pub mounts: Vec<MountDefinition>,
    #[serde(default, rename = "visual_nodes")]
    pub visual_nodes: Vec<ComponentVisualNode>,
    #[serde(default, rename = "ports")]
    pub ports: Vec<PortDefinition>,
    #[serde(default, rename = "signals")]
    pub signals: Vec<SignalDefinition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundsDefinition {
    pub size: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceDefinition {
    pub name: String,
    pub origin: [f64; 3],
    pub normal: [f64; 3],
    pub extents: [f64; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MountDefinition {
    pub name: String,
    pub origin: [f64; 3],
    pub axis: [f64; 3],
    pub up: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentVisualNode {
    pub suffix: String,
    pub primitive: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_asset: Option<String>,
    pub label_suffix: String,
    pub position_offset: [f64; 3],
    pub scale: [f64; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_rotation: Option<[f64; 3]>,
    pub material: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_suffix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pivot: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortDefinition {
    pub name: String,
    pub domain: String,
    pub direction: PortDirection,
    pub size: String,
    pub origin: [f64; 3],
    pub axis: [f64; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortDirection {
    In,
    Out,
    InOut,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_suffix: Option<String>,
    pub property: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub map: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_node: Option<SignalVisualNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding_scale: Option<BindingScale>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalVisualNode {
    pub primitive: String,
    pub label_suffix: String,
    pub position_offset: [f64; 3],
    pub scale: [f64; 3],
    pub material: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BindingScale {
    pub min: f64,
    pub max: f64,
    pub output_min: f64,
    pub output_max: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopologyCompileOptions {
    pub strict_warnings: bool,
}

impl Default for TopologyCompileOptions {
    fn default() -> Self {
        Self {
            strict_warnings: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompiledTopology {
    pub topology_hash: String,
    pub view_hash: String,
    pub view_toml: String,
    pub diagnostics: Vec<TopologyDiagnostic>,
    pub doctor_results: Vec<DoctorRuleResult>,
    pub stats: TopologyCompileStats,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TopologyCompileStats {
    pub component_count: usize,
    pub connection_count: usize,
    pub binding_count: usize,
    pub generated_node_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorRuleResult {
    pub rule: String,
    pub passed: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TopologyDiagnostic {
    pub code: String,
    pub severity: TopologyDiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TopologyDiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompiledViewFreshness {
    pub topology_hash: String,
    pub compiled_hash: Option<String>,
    pub matches: bool,
}

#[derive(Debug, Error)]
pub enum TopologyCompileError {
    #[error("invalid component library TOML: {0}")]
    LibraryToml(#[from] toml::de::Error),
    #[error("failed to read component library file {path}: {source}")]
    LibraryRead {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("topology validation failed")]
    Diagnostics {
        diagnostics: Vec<TopologyDiagnostic>,
    },
}

impl TopologyCompileError {
    pub fn diagnostics(&self) -> &[TopologyDiagnostic] {
        match self {
            Self::Diagnostics { diagnostics } => diagnostics,
            Self::LibraryToml(_) | Self::LibraryRead { .. } => &[],
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct TopologyDocument {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    metadata: BTreeMap<String, toml::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grid: Option<TopologyGrid>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    components: Vec<TopologyComponent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    connections: Vec<TopologyConnection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    bindings: Vec<TopologyBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    interactions: Vec<TopologyInteraction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct TopologyGrid {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cell_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<[f64; 3]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TopologyComponent {
    id: String,
    kind: String,
    at: TopologyPlacement,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    params: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TopologyPlacement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    right_of: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    offset: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    xyz: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    justification: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attach_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    placement: Option<AttachmentPlacement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AttachmentPlacement {
    TopCenter,
    Center,
    FrontFace,
    BackFace,
    LeftFace,
    RightFace,
    Mount,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TopologyConnection {
    id: String,
    from: String,
    to: String,
    medium: String,
    diameter: String,
    route: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TopologyBinding {
    component: String,
    signal: String,
    source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    access: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TopologyInteraction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    component: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    node: Option<String>,
    event: String,
    action: String,
    id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value: Option<toml::Value>,
    required_role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    confirmation: Option<TopologyInteractionConfirmation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TopologyInteractionConfirmation {
    title: String,
    message: String,
}

#[derive(Debug, Clone, PartialEq)]
struct ResolvedComponent<'a> {
    source: &'a TopologyComponent,
    kind: &'a ComponentKind,
    position: [f64; 3],
    parent: Option<String>,
    local_position: [f64; 3],
    pivot: [f64; 3],
}

#[derive(Debug, Clone, PartialEq)]
struct ResolvedConnection<'a> {
    source: &'a TopologyConnection,
    from_position: [f64; 3],
    to_position: [f64; 3],
    from_port: &'a PortDefinition,
    to_port: &'a PortDefinition,
}

#[derive(Debug, Clone, PartialEq)]
struct ResolvedBinding<'a> {
    source: &'a TopologyBinding,
    component_id: String,
    component_position: [f64; 3],
    signal: &'a SignalDefinition,
}

#[derive(Debug, Clone, PartialEq)]
struct ResolvedInteraction<'a> {
    source: &'a TopologyInteraction,
    node_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SceneViewNodeBounds {
    position: [f64; 3],
    scale: [f64; 3],
}

impl SceneViewNodeBounds {
    fn from_toml(node: &toml::Value) -> Option<Self> {
        let transform = node.get("transform")?;
        Some(Self {
            position: toml_vec3(transform.get("position")?)?,
            scale: toml_vec3(transform.get("scale")?)?,
        })
    }
}

impl ComponentLibrary {
    pub fn load_builtin() -> Result<Self, TopologyCompileError> {
        Ok(toml::from_str(BUILTIN_LIBRARY)?)
    }

    pub fn load_from_dir(path: impl AsRef<Path>) -> Result<Self, TopologyCompileError> {
        let file = path.as_ref().join("components.toml");
        let text =
            std::fs::read_to_string(&file).map_err(|source| TopologyCompileError::LibraryRead {
                path: file.display().to_string(),
                source,
            })?;
        Ok(toml::from_str(&text)?)
    }

    pub fn kind(&self, name: &str) -> Option<&ComponentKind> {
        self.kinds.iter().find(|kind| kind.name == name)
    }
}

impl ComponentKind {
    pub fn port(&self, name: &str) -> Option<&PortDefinition> {
        self.ports.iter().find(|port| port.name == name)
    }

    pub fn signal(&self, name: &str) -> Option<&SignalDefinition> {
        self.signals.iter().find(|signal| signal.name == name)
    }

    pub fn surface(&self, name: &str) -> Option<&SurfaceDefinition> {
        self.surfaces.iter().find(|surface| surface.name == name)
    }

    pub fn mount(&self, name: &str) -> Option<&MountDefinition> {
        self.mounts.iter().find(|mount| mount.name == name)
    }

    fn physical_size(&self) -> [f64; 3] {
        self.bounds.map_or(self.scale, |bounds| bounds.size)
    }

    fn bottom_surface_origin(&self) -> [f64; 3] {
        self.surface("bottom")
            .map_or([0.0, -self.physical_size()[1] / 2.0, 0.0], |surface| {
                surface.origin
            })
    }
}

pub fn compile_topology_to_view(
    topology_source: &str,
    library: &ComponentLibrary,
    _options: &TopologyCompileOptions,
) -> Result<CompiledTopology, TopologyCompileError> {
    let document = parse_topology(topology_source)?;
    let topology_hash = hash_topology_document(&document)?;
    let validation = validate_topology(&document, library);
    if !validation.diagnostics.is_empty() {
        return Err(TopologyCompileError::Diagnostics {
            diagnostics: validation.diagnostics,
        });
    }

    let view_toml = render_view(&topology_hash, &validation);
    let view_hash = sha256_hex(view_toml.as_bytes());
    let generated_node_count = validation.generated_node_count();
    Ok(CompiledTopology {
        topology_hash,
        view_hash,
        view_toml,
        diagnostics: Vec::new(),
        doctor_results: validation.doctor_results,
        stats: TopologyCompileStats {
            component_count: document.components.len(),
            connection_count: document.connections.len(),
            binding_count: document.bindings.len(),
            generated_node_count,
        },
    })
}

pub fn verify_compiled_view_fresh(
    topology_source: &str,
    compiled_view_toml: &str,
) -> Result<CompiledViewFreshness, TopologyCompileError> {
    let document = parse_topology(topology_source)?;
    let topology_hash = hash_topology_document(&document)?;
    let compiled_hash = compiled_view_toml
        .lines()
        .next()
        .and_then(|line| line.strip_prefix(TOPOLOGY_HASH_HEADER_PREFIX))
        .map(ToOwned::to_owned);
    let matches = compiled_hash.as_deref() == Some(topology_hash.as_str());
    Ok(CompiledViewFreshness {
        topology_hash,
        compiled_hash,
        matches,
    })
}

pub fn diagnose_scene_view_physical_issues(
    view_source: &str,
) -> Result<Vec<TopologyDiagnostic>, TopologyCompileError> {
    let view: toml::Value = toml::from_str(view_source)?;
    let mut nodes = BTreeMap::new();
    if let Some(node_values) = view.get("node").and_then(toml::Value::as_array) {
        for node in node_values {
            let Some(id) = node.get("id").and_then(toml::Value::as_str) else {
                continue;
            };
            if let Some(bounds) = SceneViewNodeBounds::from_toml(node) {
                nodes.insert(id.to_string(), bounds);
            }
        }
    }

    let mut diagnostics = Vec::new();
    if let (Some(workpiece), Some(pickup)) = (nodes.get("box.workpiece"), nodes.get("zone.pickup"))
    {
        let workpiece_bottom = workpiece.position[1] - (workpiece.scale[1] / 2.0);
        let pickup_top = pickup.position[1] + (pickup.scale[1] / 2.0);
        if (workpiece_bottom - pickup_top).abs() > 0.02 {
            diagnostics.push(diagnostic(
                "workpiece-rests-on-surface",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "box.workpiece bottom y={} does not rest on zone.pickup top y={}",
                    number(workpiece_bottom),
                    number(pickup_top)
                ),
            ));
        }
    }

    if nodes.iter().any(|(id, bounds)| {
        id.starts_with("robot.")
            && id != "robot.base"
            && bounds.position[1] - (bounds.scale[1] / 2.0) < -0.001
    }) {
        diagnostics.push(diagnostic(
            "link-above-floor",
            TopologyDiagnosticSeverity::Error,
            "robot link or tooling extends below floor/table y=0".to_string(),
        ));
    }

    if let Some(workpiece) = nodes.get("box.workpiece") {
        let workpiece_bottom = workpiece.position[1] - (workpiece.scale[1] / 2.0);
        let jaw_under_workpiece = ["robot.gripper.left", "robot.gripper.right"]
            .iter()
            .filter_map(|id| nodes.get(*id))
            .any(|jaw| jaw.position[1] < workpiece_bottom);
        if jaw_under_workpiece {
            diagnostics.push(diagnostic(
                "gripper-approach-sane",
                TopologyDiagnosticSeverity::Error,
                "gripper jaw center approaches from below the workpiece bottom".to_string(),
            ));
        }
    }

    Ok(diagnostics)
}

fn parse_topology(source: &str) -> Result<TopologyDocument, TopologyCompileError> {
    toml::from_str(source).map_err(TopologyCompileError::LibraryToml)
}

fn hash_topology_document(document: &TopologyDocument) -> Result<String, TopologyCompileError> {
    let canonical =
        toml::to_string_pretty(document).map_err(|err| TopologyCompileError::Diagnostics {
            diagnostics: vec![diagnostic(
                "canonical-topology",
                TopologyDiagnosticSeverity::Error,
                format!("failed to serialize canonical topology: {err}"),
            )],
        })?;
    Ok(sha256_hex(
        format!("{TOPOLOGY_HASH_DOMAIN}\n{canonical}").as_bytes(),
    ))
}

#[derive(Debug, Default)]
struct TopologyValidation<'a> {
    diagnostics: Vec<TopologyDiagnostic>,
    doctor_results: Vec<DoctorRuleResult>,
    metadata: BTreeMap<String, toml::Value>,
    components: Vec<ResolvedComponent<'a>>,
    connections: Vec<ResolvedConnection<'a>>,
    bindings: Vec<ResolvedBinding<'a>>,
    interactions: Vec<ResolvedInteraction<'a>>,
}

impl TopologyValidation<'_> {
    fn generated_node_count(&self) -> usize {
        let visual_binding_nodes = self
            .bindings
            .iter()
            .filter(|binding| binding.signal.visual_node.is_some())
            .count();
        let component_visual_nodes = self
            .components
            .iter()
            .map(|component| component.kind.visual_nodes.len())
            .sum::<usize>();
        let contextual_nodes = if is_robot_cell_view(&self.metadata) {
            2
        } else {
            0
        };
        self.components.len()
            + component_visual_nodes
            + self.connections.len()
            + visual_binding_nodes
            + contextual_nodes
    }
}

fn validate_topology<'a>(
    document: &'a TopologyDocument,
    library: &'a ComponentLibrary,
) -> TopologyValidation<'a> {
    let mut validation = TopologyValidation {
        metadata: document.metadata.clone(),
        ..TopologyValidation::default()
    };
    let mut ids = BTreeSet::new();
    let mut grid_cells = BTreeMap::<String, String>::new();
    let mut components_by_id = BTreeMap::<String, (&TopologyComponent, &ComponentKind)>::new();

    for component in &document.components {
        if !ids.insert(component.id.clone()) {
            validation.diagnostics.push(diagnostic(
                "duplicate-component-id",
                TopologyDiagnosticSeverity::Error,
                format!("component id '{}' is declared more than once", component.id),
            ));
        }
        let Some(kind) = library.kind(&component.kind) else {
            validation.diagnostics.push(diagnostic(
                "unknown-component-kind",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "component '{}' references unknown kind '{}'",
                    component.id, component.kind
                ),
            ));
            continue;
        };
        components_by_id.insert(component.id.clone(), (component, kind));

        if let Some(grid) = &component.at.grid {
            if let Some(previous) = grid_cells.insert(grid.clone(), component.id.clone()) {
                validation.diagnostics.push(diagnostic(
                    "duplicate-grid-cell",
                    TopologyDiagnosticSeverity::Error,
                    format!(
                        "components '{}' and '{}' both use grid cell '{}'",
                        previous, component.id, grid
                    ),
                ));
            }
        }
        if component.at.xyz.is_some()
            && component
                .at
                .justification
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            validation.diagnostics.push(diagnostic(
                "raw-xyz-without-justification",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "component '{}' uses raw xyz placement without justification",
                    component.id
                ),
            ));
        }
        if component.at.attach_to.is_some() && component.at.placement.is_none() {
            validation.diagnostics.push(diagnostic(
                "missing-attachment-placement",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "component '{}' uses attach_to without a placement value",
                    component.id
                ),
            ));
        }
    }

    let positions = resolve_positions(document, library, &components_by_id, &mut validation);
    for component in &document.components {
        if let Some((_, kind)) = components_by_id.get(&component.id) {
            if let Some(position) = positions.get(&component.id).copied() {
                let (parent, local_position) = component_parent_frame(
                    component,
                    kind,
                    position,
                    &positions,
                    &components_by_id,
                );
                validation.components.push(ResolvedComponent {
                    source: component,
                    kind,
                    position,
                    parent,
                    local_position,
                    pivot: [0.0, 0.0, 0.0],
                });
            }
        }
    }
    let resolved_by_id = validation
        .components
        .iter()
        .map(|component| {
            (
                component.source.id.as_str(),
                (component.kind, component.position),
            )
        })
        .collect::<BTreeMap<_, _>>();

    validate_physical_scene(
        document,
        library,
        &components_by_id,
        &positions,
        &mut validation,
    );

    let mut booked_ports = BTreeSet::new();
    for connection in &document.connections {
        let Some((from_id, from_port_name)) = parse_endpoint(&connection.from) else {
            validation.diagnostics.push(diagnostic(
                "invalid-endpoint",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "connection '{}' has invalid from endpoint '{}'",
                    connection.id, connection.from
                ),
            ));
            continue;
        };
        let Some((to_id, to_port_name)) = parse_endpoint(&connection.to) else {
            validation.diagnostics.push(diagnostic(
                "invalid-endpoint",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "connection '{}' has invalid to endpoint '{}'",
                    connection.id, connection.to
                ),
            ));
            continue;
        };
        let Some((from_kind, from_position)) = resolved_by_id.get(from_id.as_str()).copied() else {
            validation.diagnostics.push(diagnostic(
                "dangling-component-id",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "connection '{}' references missing component '{}'",
                    connection.id, from_id
                ),
            ));
            continue;
        };
        let Some((to_kind, to_position)) = resolved_by_id.get(to_id.as_str()).copied() else {
            validation.diagnostics.push(diagnostic(
                "dangling-component-id",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "connection '{}' references missing component '{}'",
                    connection.id, to_id
                ),
            ));
            continue;
        };
        let Some(from_port) = from_kind.port(&from_port_name) else {
            validation.diagnostics.push(diagnostic(
                "unknown-port",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "connection '{}' references unknown port '{}.{}'",
                    connection.id, from_id, from_port_name
                ),
            ));
            continue;
        };
        let Some(to_port) = to_kind.port(&to_port_name) else {
            validation.diagnostics.push(diagnostic(
                "unknown-port",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "connection '{}' references unknown port '{}.{}'",
                    connection.id, to_id, to_port_name
                ),
            ));
            continue;
        };
        if from_port.direction == PortDirection::In || to_port.direction == PortDirection::Out {
            validation.diagnostics.push(diagnostic(
                "invalid-port-direction",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "connection '{}' must route from an output-capable port to an input-capable port",
                    connection.id
                ),
            ));
        }
        if from_port.domain != to_port.domain
            || from_port.domain != connection.medium
            || to_port.domain != connection.medium
        {
            validation.diagnostics.push(diagnostic(
                "domain-mismatch",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "connection '{}' uses medium '{}' but endpoints are '{}.{}' ({}) and '{}.{}' ({})",
                    connection.id,
                    connection.medium,
                    from_id,
                    from_port_name,
                    from_port.domain,
                    to_id,
                    to_port_name,
                    to_port.domain
                ),
            ));
        }
        for endpoint in [&connection.from, &connection.to] {
            if !booked_ports.insert(endpoint.clone()) {
                validation.diagnostics.push(diagnostic(
                    "double-booked-port",
                    TopologyDiagnosticSeverity::Error,
                    format!(
                        "connection '{}' reuses endpoint '{}'",
                        connection.id, endpoint
                    ),
                ));
            }
        }
        validation.connections.push(ResolvedConnection {
            source: connection,
            from_position,
            to_position,
            from_port,
            to_port,
        });
    }

    for binding in &document.bindings {
        let Some((kind, position)) = resolved_by_id.get(binding.component.as_str()).copied() else {
            validation.diagnostics.push(diagnostic(
                "dangling-component-id",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "binding '{}.{}' references missing component '{}'",
                    binding.component, binding.signal, binding.component
                ),
            ));
            continue;
        };
        let Some(signal) = kind.signal(&binding.signal) else {
            validation.diagnostics.push(diagnostic(
                "unknown-signal",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "binding '{}.{}' references unknown signal '{}'",
                    binding.component, binding.signal, binding.signal
                ),
            ));
            continue;
        };
        if binding
            .access
            .as_deref()
            .is_some_and(|access| access != "read")
        {
            validation.diagnostics.push(diagnostic(
                "unsafe-write-binding",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "binding '{}.{}' requests non-read access",
                    binding.component, binding.signal
                ),
            ));
        }
        validation.bindings.push(ResolvedBinding {
            source: binding,
            component_id: binding.component.clone(),
            component_position: position,
            signal,
        });
    }

    let generated_node_ids = generated_node_ids(&validation);
    for interaction in &document.interactions {
        let component = interaction
            .component
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(component) = component {
            if !resolved_by_id.contains_key(component) {
                validation.diagnostics.push(diagnostic(
                    "dangling-component-id",
                    TopologyDiagnosticSeverity::Error,
                    format!("interaction references missing component '{component}'"),
                ));
                continue;
            }
        }
        let node_id = interaction
            .node
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or(component);
        let Some(node_id) = node_id else {
            validation.diagnostics.push(diagnostic(
                "missing-interaction-target",
                TopologyDiagnosticSeverity::Error,
                "interaction must reference component or node".to_string(),
            ));
            continue;
        };
        if !generated_node_ids.contains(node_id) {
            validation.diagnostics.push(diagnostic(
                "unknown-interaction-node",
                TopologyDiagnosticSeverity::Error,
                format!("interaction references unknown scene node '{node_id}'"),
            ));
            continue;
        }
        if normalize_interaction_event(&interaction.event).is_none() {
            validation.diagnostics.push(diagnostic(
                "unsupported-interaction-event",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "interaction on node '{}' uses unsupported event '{}'",
                    node_id, interaction.event
                ),
            ));
            continue;
        }
        if normalize_interaction_action(&interaction.action).as_deref() != Some("hmi.write") {
            validation.diagnostics.push(diagnostic(
                "unsupported-interaction-action",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "interaction on node '{}' uses unsupported action '{}'",
                    node_id, interaction.action
                ),
            ));
            continue;
        }
        if normalize_interaction_required_role(&interaction.required_role).as_deref()
            != Some("Engineer")
        {
            validation.diagnostics.push(diagnostic(
                "unsafe-write-interaction-role",
                TopologyDiagnosticSeverity::Error,
                format!("hmi.write interaction on node '{node_id}' must require Engineer role"),
            ));
            continue;
        }
        if interaction.id.trim().is_empty() {
            validation.diagnostics.push(diagnostic(
                "missing-interaction-target-id",
                TopologyDiagnosticSeverity::Error,
                format!("hmi.write interaction on node '{node_id}' must set id"),
            ));
            continue;
        }
        let Some(value) = interaction.value.as_ref() else {
            validation.diagnostics.push(diagnostic(
                "missing-interaction-value",
                TopologyDiagnosticSeverity::Error,
                format!("hmi.write interaction on node '{node_id}' must set value"),
            ));
            continue;
        };
        if !supported_interaction_value(value) {
            validation.diagnostics.push(diagnostic(
                "unsupported-interaction-value",
                TopologyDiagnosticSeverity::Error,
                format!("hmi.write interaction on node '{node_id}' uses unsupported value type"),
            ));
            continue;
        }
        validation.interactions.push(ResolvedInteraction {
            source: interaction,
            node_id: node_id.to_string(),
        });
    }

    if validation.diagnostics.is_empty() {
        validation.doctor_results = vec![
            doctor("component-kind", "all component kinds resolve"),
            doctor(
                "port-exists",
                "all connection endpoints reference known ports",
            ),
            doctor(
                "port-domain-compatible",
                "all connection endpoint domains match",
            ),
            doctor(
                "port-not-double-booked",
                "all connection endpoints are unique",
            ),
            doctor("grid-cell-unique", "all grid cells are unique"),
            doctor(
                "auto-route-feasible",
                "all auto routes generated straight-line pipe nodes",
            ),
            doctor("binding-valid", "all component signals compile to bind3d"),
            doctor(
                "interaction-safe-write",
                "all operator interactions route through hmi.write with Engineer role",
            ),
            doctor(
                "attachment-target-exists",
                "all attach_to targets reference known surfaces or mount frames",
            ),
            doctor(
                "workpiece-rests-on-surface",
                "all workpieces rest on declared surfaces or carry accepted justification",
            ),
            doctor(
                "parent-transform-propagates",
                "attached child transforms derive from parent placement frames",
            ),
            doctor(
                "scale-vs-grid-cell",
                "grid-placed component footprints fit their declared grid cells",
            ),
            doctor(
                "link-above-floor",
                "robot links and tooling remain above floor/table envelope",
            ),
            doctor(
                "gripper-approach-sane",
                "gripper approach axes point from a plausible side/top direction",
            ),
        ];
    }

    validation
}

fn resolve_positions<'a>(
    document: &'a TopologyDocument,
    library: &'a ComponentLibrary,
    components_by_id: &BTreeMap<String, (&'a TopologyComponent, &'a ComponentKind)>,
    validation: &mut TopologyValidation<'a>,
) -> BTreeMap<String, [f64; 3]> {
    let cell_size = document
        .grid
        .and_then(|grid| grid.cell_size)
        .unwrap_or(library.grid.cell_size);
    let origin = document
        .grid
        .and_then(|grid| grid.origin)
        .unwrap_or(library.grid.origin);
    let mut positions = BTreeMap::new();

    for (id, (component, _)) in components_by_id {
        if let Some(grid) = &component.at.grid {
            match grid_position(grid, origin, cell_size) {
                Some(position) => {
                    positions.insert(id.clone(), position);
                }
                None => validation.diagnostics.push(diagnostic(
                    "invalid-grid-cell",
                    TopologyDiagnosticSeverity::Error,
                    format!("component '{}' has invalid grid cell '{}'", id, grid),
                )),
            }
        } else if component.at.anchor.as_deref() == Some("origin") {
            positions.insert(id.clone(), origin);
        } else if let Some(xyz) = component.at.xyz {
            positions.insert(id.clone(), xyz);
        }
    }

    let mut unresolved = components_by_id
        .iter()
        .filter_map(|(id, (component, _))| {
            component.at.right_of.as_ref().map(|right_of| {
                (
                    id.clone(),
                    right_of.clone(),
                    component.at.offset.unwrap_or(1.0),
                )
            })
        })
        .collect::<Vec<_>>();
    while !unresolved.is_empty() {
        let mut progressed = false;
        unresolved.retain(|(id, right_of, offset)| {
            if let Some(base) = positions.get(right_of).copied() {
                positions.insert(
                    id.clone(),
                    [base[0] + (cell_size * *offset), base[1], base[2]],
                );
                progressed = true;
                false
            } else {
                true
            }
        });
        if !progressed {
            for (id, right_of, _) in unresolved {
                validation.diagnostics.push(diagnostic(
                    "unresolved-relative-placement",
                    TopologyDiagnosticSeverity::Error,
                    format!("component '{id}' is right_of missing component '{right_of}'"),
                ));
            }
            break;
        }
    }

    let mut unresolved_attachments = components_by_id
        .iter()
        .filter_map(|(id, (component, kind))| {
            component
                .at
                .attach_to
                .as_ref()
                .map(|target| (id.clone(), target.clone(), *kind))
        })
        .collect::<Vec<_>>();
    while !unresolved_attachments.is_empty() {
        let mut progressed = false;
        let mut failed = BTreeSet::new();
        unresolved_attachments.retain(|(id, target, child_kind)| {
            let Some((parent_id, target_name)) = parse_endpoint(target) else {
                validation.diagnostics.push(diagnostic(
                    "attachment-target-exists",
                    TopologyDiagnosticSeverity::Error,
                    format!("component '{id}' has invalid attach_to target '{target}'"),
                ));
                failed.insert(id.clone());
                return false;
            };
            let Some((_, parent_kind)) = components_by_id.get(&parent_id) else {
                validation.diagnostics.push(diagnostic(
                    "attachment-target-exists",
                    TopologyDiagnosticSeverity::Error,
                    format!(
                        "component '{id}' attaches to missing component '{}'",
                        parent_id
                    ),
                ));
                failed.insert(id.clone());
                return false;
            };
            let Some(parent_position) = positions.get(&parent_id).copied() else {
                return true;
            };
            let Some(placement) = components_by_id
                .get(id)
                .and_then(|(component, _)| component.at.placement)
            else {
                failed.insert(id.clone());
                return false;
            };
            let context = AttachmentResolution {
                child_id: id,
                target,
                target_name: &target_name,
                placement,
                child_kind,
                parent_kind,
                parent_position,
            };
            let Some(position) = resolve_attachment_position(context, validation) else {
                failed.insert(id.clone());
                return false;
            };
            positions.insert(id.clone(), position);
            progressed = true;
            false
        });
        if !progressed {
            for (id, target, _) in unresolved_attachments {
                if !failed.contains(&id) {
                    validation.diagnostics.push(diagnostic(
                        "unresolved-attachment-placement",
                        TopologyDiagnosticSeverity::Error,
                        format!("component '{id}' attaches to unresolved target '{target}'"),
                    ));
                }
            }
            break;
        }
    }

    for id in components_by_id.keys() {
        if !positions.contains_key(id) {
            validation.diagnostics.push(diagnostic(
                "unsupported-placement",
                TopologyDiagnosticSeverity::Error,
                format!("component '{id}' has no supported v1 placement"),
            ));
        }
    }

    positions
}

struct AttachmentResolution<'a> {
    child_id: &'a str,
    target: &'a str,
    target_name: &'a str,
    placement: AttachmentPlacement,
    child_kind: &'a ComponentKind,
    parent_kind: &'a ComponentKind,
    parent_position: [f64; 3],
}

fn resolve_attachment_position(
    context: AttachmentResolution<'_>,
    validation: &mut TopologyValidation<'_>,
) -> Option<[f64; 3]> {
    match context.placement {
        AttachmentPlacement::TopCenter => {
            let Some(surface) = context.parent_kind.surface(context.target_name) else {
                validation.diagnostics.push(diagnostic(
                    "attachment-target-exists",
                    TopologyDiagnosticSeverity::Error,
                    format!(
                        "component '{}' uses top_center but target '{}' is not a known surface",
                        context.child_id, context.target
                    ),
                ));
                return None;
            };
            let target_origin = add_vec3(context.parent_position, surface.origin);
            Some(sub_vec3(
                target_origin,
                context.child_kind.bottom_surface_origin(),
            ))
        }
        AttachmentPlacement::Mount => {
            let Some(parent_mount) = context.parent_kind.mount(context.target_name) else {
                validation.diagnostics.push(diagnostic(
                    "attachment-target-exists",
                    TopologyDiagnosticSeverity::Error,
                    format!(
                        "component '{}' uses mount but target '{}' is not a known mount frame",
                        context.child_id, context.target
                    ),
                ));
                return None;
            };
            let child_mount_origin = context
                .child_kind
                .mount("wrist")
                .or_else(|| context.child_kind.mount("mount"))
                .map_or([0.0, 0.0, 0.0], |mount| mount.origin);
            Some(sub_vec3(
                add_vec3(context.parent_position, parent_mount.origin),
                child_mount_origin,
            ))
        }
        AttachmentPlacement::Center
        | AttachmentPlacement::FrontFace
        | AttachmentPlacement::BackFace
        | AttachmentPlacement::LeftFace
        | AttachmentPlacement::RightFace => {
            let Some(origin) = context
                .parent_kind
                .surface(context.target_name)
                .map(|surface| surface.origin)
                .or_else(|| {
                    context
                        .parent_kind
                        .mount(context.target_name)
                        .map(|mount| mount.origin)
                })
            else {
                validation.diagnostics.push(diagnostic(
                    "attachment-target-exists",
                    TopologyDiagnosticSeverity::Error,
                    format!(
                        "component '{}' attaches to unknown surface or mount '{}'",
                        context.child_id, context.target
                    ),
                ));
                return None;
            };
            Some(add_vec3(context.parent_position, origin))
        }
    }
}

fn component_parent_frame(
    component: &TopologyComponent,
    _kind: &ComponentKind,
    position: [f64; 3],
    positions: &BTreeMap<String, [f64; 3]>,
    components_by_id: &BTreeMap<String, (&TopologyComponent, &ComponentKind)>,
) -> (Option<String>, [f64; 3]) {
    let Some(target) = component.at.attach_to.as_deref() else {
        return (None, position);
    };
    let Some((parent_id, target_name)) = parse_endpoint(target) else {
        return (None, position);
    };
    let Some((_, parent_kind)) = components_by_id.get(&parent_id) else {
        return (None, position);
    };
    let Some(parent_position) = positions.get(&parent_id).copied() else {
        return (None, position);
    };
    let (parent_node_id, parent_frame_position) =
        attachment_parent_frame(&parent_id, &target_name, parent_kind, parent_position);
    (
        Some(parent_node_id),
        sub_vec3(position, parent_frame_position),
    )
}

fn attachment_parent_frame(
    parent_id: &str,
    target_name: &str,
    parent_kind: &ComponentKind,
    parent_position: [f64; 3],
) -> (String, [f64; 3]) {
    if parent_kind.name == "robot_arm" && target_name == "tool" {
        if let Some(tool_frame) = ["wrist_flange", "tool", "wrist_housing", "wrist"]
            .iter()
            .find_map(|suffix| {
                parent_kind
                    .visual_nodes
                    .iter()
                    .find(|node| node.suffix == *suffix)
            })
        {
            return (
                format!("{parent_id}.{}", tool_frame.suffix),
                add_vec3(parent_position, tool_frame.position_offset),
            );
        }
    }
    (parent_id.to_string(), parent_position)
}

fn validate_physical_scene<'a>(
    document: &'a TopologyDocument,
    library: &'a ComponentLibrary,
    components_by_id: &BTreeMap<String, (&'a TopologyComponent, &'a ComponentKind)>,
    _positions: &BTreeMap<String, [f64; 3]>,
    validation: &mut TopologyValidation<'a>,
) {
    let cell_size = document
        .grid
        .and_then(|grid| grid.cell_size)
        .unwrap_or(library.grid.cell_size);

    for (component_id, (component, kind)) in components_by_id {
        let has_justification = component
            .at
            .justification
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());

        if kind.name == "workpiece" && component.at.attach_to.is_none() && !has_justification {
            validation.diagnostics.push(diagnostic(
                "workpiece-rests-on-surface",
                TopologyDiagnosticSeverity::Error,
                format!(
                    "workpiece '{component_id}' must attach to a declared surface or carry a floating justification"
                ),
            ));
        }

        if component.at.grid.is_some() {
            let [width, _, depth] = kind.physical_size();
            let margin = cell_size * 1.05;
            if (width > margin || depth > margin) && !has_justification {
                validation.diagnostics.push(diagnostic(
                    "scale-vs-grid-cell",
                    TopologyDiagnosticSeverity::Error,
                    format!(
                        "component '{component_id}' footprint [{width}, {depth}] exceeds grid cell size {cell_size}"
                    ),
                ));
            }
        }

        if matches!(kind.name.as_str(), "robot_arm" | "gripper")
            && param_number(&component.params, "min_y").is_some_and(|min_y| min_y < 0.0)
        {
            validation.diagnostics.push(diagnostic(
                "link-above-floor",
                TopologyDiagnosticSeverity::Error,
                format!("component '{component_id}' has sampled minimum y below floor/table"),
            ));
        }

        if kind.name == "gripper"
            && param_vec3(&component.params, "approach_axis").is_some_and(|axis| axis[1] < -0.25)
        {
            validation.diagnostics.push(diagnostic(
                "gripper-approach-sane",
                TopologyDiagnosticSeverity::Error,
                format!("gripper '{component_id}' approach axis points from below"),
            ));
        }
    }
}

fn render_view(topology_hash: &str, validation: &TopologyValidation<'_>) -> String {
    let mut output = String::new();
    let use_packaged_assets = is_packaged_asset_view(&validation.metadata);
    writeln!(output, "{TOPOLOGY_HASH_HEADER_PREFIX}{topology_hash}").expect("write to string");
    if !validation.metadata.is_empty() {
        writeln!(output, "[metadata]").expect("write to string");
        writeln!(output, "view_contract_version = {VIEW_CONTRACT_VERSION}")
            .expect("write to string");
        writeln!(
            output,
            "renderer_contract_version = {RENDERER_CONTRACT_VERSION}"
        )
        .expect("write to string");
        for (key, value) in &validation.metadata {
            if matches!(
                key.as_str(),
                "view_contract_version" | "renderer_contract_version"
            ) {
                continue;
            }
            writeln!(output, "{key} = {}", toml_value_literal(value)).expect("write to string");
        }
        writeln!(output).expect("write to string");
    }

    if use_packaged_assets {
        write_asset_catalog(&mut output, validation);
    }

    for component in &validation.components {
        write_component_node(
            &mut output,
            component,
            &validation.components,
            &validation.interactions,
            use_packaged_assets,
        );
        for visual_node in &component.kind.visual_nodes {
            write_component_visual_node(&mut output, component, visual_node, use_packaged_assets);
        }
        for binding in validation
            .bindings
            .iter()
            .filter(|binding| binding.component_id == component.source.id)
        {
            if let Some(visual_node) = &binding.signal.visual_node {
                write_signal_visual_node(
                    &mut output,
                    binding,
                    visual_node,
                    &validation.interactions,
                );
            }
        }
    }

    for connection in &validation.connections {
        write_pipe_node(&mut output, connection, &validation.interactions);
    }

    if is_robot_cell_view(&validation.metadata) {
        write_robot_cell_floor_node(&mut output);
        write_depth_sentinel_node(&mut output);
    }

    let camera_target = camera_target(&validation.components);
    let camera_profile = camera_profile(&validation.metadata);
    let target_x = camera_target[0] + camera_profile.target_x_offset;
    let camera_x = target_x + camera_profile.eye_x_offset;
    writeln!(output, "[[camera]]").expect("write to string");
    writeln!(output, "id = \"main\"").expect("write to string");
    writeln!(
        output,
        "position = [{}, {}, {}]",
        number(camera_x),
        number(camera_profile.height),
        number(camera_profile.distance)
    )
    .expect("write to string");
    writeln!(
        output,
        "target = [{}, {}, {}]",
        number(target_x),
        number(camera_profile.target_y),
        number(camera_profile.target_z)
    )
    .expect("write to string");
    writeln!(
        output,
        "fov_degrees = {}",
        number(camera_profile.fov_degrees)
    )
    .expect("write to string");
    writeln!(output).expect("write to string");

    writeln!(output, "[[light]]").expect("write to string");
    writeln!(output, "id = \"key\"").expect("write to string");
    writeln!(output, "kind = \"directional\"").expect("write to string");
    writeln!(
        output,
        "position = [{}, {}, {}]",
        number(2.0),
        number(4.0),
        number(6.0)
    )
    .expect("write to string");
    writeln!(output, "intensity = {}", number(1.0)).expect("write to string");
    writeln!(output).expect("write to string");

    for binding in &validation.bindings {
        write_binding(&mut output, binding);
    }

    output
}

fn write_asset_catalog(output: &mut String, validation: &TopologyValidation<'_>) {
    let mut assets = BTreeSet::new();
    for component in &validation.components {
        assets.insert(component.kind.default_mesh_asset.as_str());
        for visual_node in &component.kind.visual_nodes {
            if let Some(asset) = visual_node.mesh_asset.as_deref() {
                assets.insert(asset);
            }
        }
    }
    for asset in assets {
        writeln!(output, "[[asset]]").expect("write to string");
        write_string_field(output, "id", asset);
        write_string_field(output, "uri", asset);
        write_string_field(output, "kind", "gltf");
        writeln!(output).expect("write to string");
    }
}

fn write_component_node(
    output: &mut String,
    component: &ResolvedComponent<'_>,
    components: &[ResolvedComponent<'_>],
    interactions: &[ResolvedInteraction<'_>],
    use_packaged_assets: bool,
) {
    writeln!(output, "[[node]]").expect("write to string");
    write_string_field(output, "id", &component.source.id);
    if use_packaged_assets {
        write_string_field(output, "asset", &component.kind.default_mesh_asset);
    }
    write_string_field(output, "primitive", &component.kind.primitive);
    write_string_field(output, "label", &component.source.id);
    write_node_frame(
        output,
        component.parent.as_deref(),
        component.local_position,
        component.pivot,
    );
    if let Some(asset_rotation) = component.kind.asset_rotation {
        write_vec3_field(output, "asset_rotation", asset_rotation);
    }
    writeln!(output).expect("write to string");
    writeln!(output, "[node.transform]").expect("write to string");
    write_vec3_field(output, "position", component.local_position);
    write_vec3_field(output, "scale", component.kind.scale);
    writeln!(output).expect("write to string");
    writeln!(output, "[node.material]").expect("write to string");
    write_string_field(output, "base_color", &component.kind.material);
    writeln!(output).expect("write to string");
    write_workpiece_parent_poses(output, component, components);
    write_interactions(output, &component.source.id, interactions);
}

fn write_component_visual_node(
    output: &mut String,
    component: &ResolvedComponent<'_>,
    visual_node: &ComponentVisualNode,
    use_packaged_assets: bool,
) {
    let node_id = format!("{}.{}", component.source.id, visual_node.suffix);
    let parent = visual_node.parent_suffix.as_ref().map_or_else(
        || component.source.id.clone(),
        |suffix| format!("{}.{}", component.source.id, suffix),
    );
    let local_position = visual_node_local_position(component.kind, visual_node);
    writeln!(output, "[[node]]").expect("write to string");
    write_string_field(output, "id", &node_id);
    if use_packaged_assets {
        if let Some(asset) = visual_node.mesh_asset.as_deref() {
            write_string_field(output, "asset", asset);
        }
    }
    write_string_field(output, "primitive", &visual_node.primitive);
    write_string_field(
        output,
        "label",
        &format!("{} {}", component.source.id, visual_node.label_suffix),
    );
    write_node_frame(
        output,
        Some(&parent),
        local_position,
        visual_node.pivot.unwrap_or([0.0, 0.0, 0.0]),
    );
    if let Some(asset_rotation) = visual_node.asset_rotation {
        write_vec3_field(output, "asset_rotation", asset_rotation);
    }
    writeln!(output).expect("write to string");
    writeln!(output, "[node.transform]").expect("write to string");
    write_vec3_field(output, "position", local_position);
    write_vec3_field(output, "scale", visual_node.scale);
    writeln!(output).expect("write to string");
    writeln!(output, "[node.material]").expect("write to string");
    write_string_field(output, "base_color", &visual_node.material);
    if let Some(opacity) = visual_node.opacity {
        writeln!(output, "opacity = {}", number(opacity)).expect("write to string");
    }
    writeln!(output).expect("write to string");
}

fn write_signal_visual_node(
    output: &mut String,
    binding: &ResolvedBinding<'_>,
    visual_node: &SignalVisualNode,
    interactions: &[ResolvedInteraction<'_>],
) {
    let node_id = binding_node_id(binding);
    writeln!(output, "[[node]]").expect("write to string");
    write_string_field(output, "id", &node_id);
    write_string_field(output, "primitive", &visual_node.primitive);
    write_string_field(
        output,
        "label",
        &format!("{} {}", binding.component_id, visual_node.label_suffix),
    );
    write_node_frame(
        output,
        Some(&binding.component_id),
        visual_node.position_offset,
        [0.0, 0.0, 0.0],
    );
    writeln!(output).expect("write to string");
    writeln!(output, "[node.transform]").expect("write to string");
    write_vec3_field(output, "position", visual_node.position_offset);
    write_vec3_field(output, "scale", visual_node.scale);
    writeln!(output).expect("write to string");
    writeln!(output, "[node.material]").expect("write to string");
    write_string_field(output, "base_color", &visual_node.material);
    if let Some(opacity) = visual_node.opacity {
        writeln!(output, "opacity = {}", number(opacity)).expect("write to string");
    }
    writeln!(output).expect("write to string");
    write_interactions(output, &node_id, interactions);
}

fn write_pipe_node(
    output: &mut String,
    connection: &ResolvedConnection<'_>,
    interactions: &[ResolvedInteraction<'_>],
) {
    let node_id = format!("{}.pipe", connection.source.id);
    let from = add_vec3(connection.from_position, connection.from_port.origin);
    let to = add_vec3(connection.to_position, connection.to_port.origin);
    let midpoint = [
        (from[0] + to[0]) / 2.0,
        (from[1] + to[1]) / 2.0,
        (from[2] + to[2]) / 2.0,
    ];
    let length =
        ((to[0] - from[0]).powi(2) + (to[1] - from[1]).powi(2) + (to[2] - from[2]).powi(2)).sqrt();
    writeln!(output, "[[node]]").expect("write to string");
    write_string_field(output, "id", &node_id);
    write_string_field(output, "primitive", "box");
    write_string_field(output, "label", &connection.source.id);
    write_node_frame(output, None, midpoint, [0.0, 0.0, 0.0]);
    writeln!(output).expect("write to string");
    writeln!(output, "[node.transform]").expect("write to string");
    write_vec3_field(output, "position", midpoint);
    write_vec3_field(output, "scale", [length, 0.08, 0.08]);
    writeln!(output).expect("write to string");
    writeln!(output, "[node.material]").expect("write to string");
    write_string_field(output, "base_color", "#0ea5e9");
    writeln!(output).expect("write to string");
    write_interactions(output, &node_id, interactions);
}

fn write_robot_cell_floor_node(output: &mut String) {
    writeln!(output, "[[node]]").expect("write to string");
    write_string_field(output, "id", "CELL-FLOOR");
    write_string_field(output, "primitive", "plate");
    write_string_field(output, "label", "Robot cell floor");
    write_node_frame(output, None, [2.0, -0.12, 0.75], [0.0, 0.0, 0.0]);
    writeln!(output).expect("write to string");
    writeln!(output, "[node.transform]").expect("write to string");
    write_vec3_field(output, "position", [2.0, -0.12, 0.75]);
    write_vec3_field(output, "scale", [4.8, 0.03, 1.4]);
    writeln!(output).expect("write to string");
    writeln!(output, "[node.material]").expect("write to string");
    write_string_field(output, "base_color", "#dbe3ea");
    writeln!(output).expect("write to string");
}

fn write_depth_sentinel_node(output: &mut String) {
    writeln!(output, "[[node]]").expect("write to string");
    write_string_field(output, "id", "RENDER-DEPTH-SENTINEL");
    write_string_field(output, "primitive", "depth-sentinel-line");
    write_string_field(output, "label", "RENDER-DEPTH-SENTINEL");
    write_node_frame(output, None, [0.0, 0.02, 0.0], [0.0, 0.0, 0.0]);
    writeln!(output).expect("write to string");
    writeln!(output, "[node.transform]").expect("write to string");
    write_vec3_field(output, "position", [0.0, 0.02, 0.0]);
    write_vec3_field(output, "scale", [1.0, 1.0, 1.0]);
    writeln!(output).expect("write to string");
    writeln!(output, "[node.material]").expect("write to string");
    write_string_field(output, "base_color", "#f8fafc");
    writeln!(output).expect("write to string");
}

fn write_node_frame(
    output: &mut String,
    parent: Option<&str>,
    local_position: [f64; 3],
    pivot: [f64; 3],
) {
    write_string_field(output, "parent", parent.unwrap_or(""));
    write_vec3_field(output, "local_position", local_position);
    write_vec3_field(output, "pivot", pivot);
}

fn visual_node_local_position(kind: &ComponentKind, visual_node: &ComponentVisualNode) -> [f64; 3] {
    if let Some(parent_suffix) = visual_node.parent_suffix.as_deref() {
        if let Some(parent) = kind
            .visual_nodes
            .iter()
            .find(|candidate| candidate.suffix == parent_suffix)
        {
            return sub_vec3(visual_node.position_offset, parent.position_offset);
        }
    }
    visual_node.position_offset
}

fn write_workpiece_parent_poses(
    output: &mut String,
    component: &ResolvedComponent<'_>,
    components: &[ResolvedComponent<'_>],
) {
    if component.kind.name != "workpiece" {
        return;
    }
    if let Some(parent) = component.parent.as_deref() {
        write_parent_pose(output, parent, component.local_position);
    }
    for gripper in components
        .iter()
        .filter(|candidate| candidate.kind.name == "gripper")
    {
        let local_position = gripper
            .kind
            .mount("pick_target")
            .map_or([0.0, 0.0, 0.0], |mount| mount.origin);
        write_parent_pose(output, &gripper.source.id, local_position);
    }
    for drop_zone in components
        .iter()
        .filter(|candidate| candidate.kind.name == "drop_zone")
    {
        write_parent_pose(output, &drop_zone.source.id, [0.0, 0.35, 0.0]);
    }
}

fn write_parent_pose(output: &mut String, parent: &str, local_position: [f64; 3]) {
    writeln!(output, "[[node.parent_pose]]").expect("write to string");
    write_string_field(output, "parent", parent);
    write_vec3_field(output, "local_position", local_position);
    writeln!(output).expect("write to string");
}

fn write_interactions(
    output: &mut String,
    node_id: &str,
    interactions: &[ResolvedInteraction<'_>],
) {
    for interaction in interactions
        .iter()
        .filter(|interaction| interaction.node_id == node_id)
    {
        writeln!(output, "[[node.interaction]]").expect("write to string");
        write_string_field(
            output,
            "event",
            normalize_interaction_event(&interaction.source.event)
                .expect("validated interaction event")
                .as_str(),
        );
        write_string_field(output, "action", "hmi.write");
        write_string_field(output, "id", interaction.source.id.trim());
        writeln!(
            output,
            "value = {}",
            toml_interaction_value_literal(
                interaction
                    .source
                    .value
                    .as_ref()
                    .expect("validated interaction value")
            )
        )
        .expect("write to string");
        write_string_field(output, "required_role", "Engineer");
        if let Some(confirmation) = &interaction.source.confirmation {
            writeln!(
                output,
                "confirmation = {{ title = \"{}\", message = \"{}\" }}",
                escape_toml_string(confirmation.title.trim()),
                escape_toml_string(confirmation.message.trim())
            )
            .expect("write to string");
        }
        writeln!(output).expect("write to string");
    }
}

fn write_binding(output: &mut String, binding: &ResolvedBinding<'_>) {
    writeln!(output, "[[bind3d]]").expect("write to string");
    write_string_field(output, "node", &binding_node_id(binding));
    write_string_field(output, "property", &binding.signal.property);
    write_string_field(output, "source", &binding.source.source);
    if let Some(scale) = binding.signal.binding_scale {
        writeln!(
            output,
            "scale = {{ min = {}, max = {}, output_min = {}, output_max = {} }}",
            number(scale.min),
            number(scale.max),
            number(scale.output_min),
            number(scale.output_max)
        )
        .expect("write to string");
    }
    if !binding.signal.map.is_empty() {
        let values = binding
            .signal
            .map
            .iter()
            .map(|(key, value)| {
                format!(
                    "\"{}\" = \"{}\"",
                    escape_toml_string(key),
                    escape_toml_string(value)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(output, "map = {{ {values} }}").expect("write to string");
    }
}

fn binding_node_id(binding: &ResolvedBinding<'_>) -> String {
    binding.signal.node_suffix.as_ref().map_or_else(
        || binding.component_id.clone(),
        |suffix| format!("{}.{}", binding.component_id, suffix),
    )
}

fn generated_node_ids(validation: &TopologyValidation<'_>) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for component in &validation.components {
        ids.insert(component.source.id.clone());
        for visual_node in &component.kind.visual_nodes {
            ids.insert(format!("{}.{}", component.source.id, visual_node.suffix));
        }
    }
    for binding in &validation.bindings {
        if binding.signal.visual_node.is_some() {
            ids.insert(binding_node_id(binding));
        }
    }
    for connection in &validation.connections {
        ids.insert(format!("{}.pipe", connection.source.id));
    }
    ids
}

fn camera_target(components: &[ResolvedComponent<'_>]) -> [f64; 3] {
    if components.is_empty() {
        return [0.0, 0.0, 0.0];
    }
    let min_x = components
        .iter()
        .map(|component| component.position[0])
        .fold(f64::INFINITY, f64::min);
    let max_x = components
        .iter()
        .map(|component| component.position[0])
        .fold(f64::NEG_INFINITY, f64::max);
    [(min_x + max_x) / 2.0, 0.0, 0.0]
}

fn camera_profile(metadata: &BTreeMap<String, toml::Value>) -> CameraProfile {
    if is_robot_cell_view(metadata) {
        return CameraProfile::ROBOT_CELL;
    }
    CameraProfile::DEFAULT
}

fn is_robot_cell_view(metadata: &BTreeMap<String, toml::Value>) -> bool {
    matches!(
        metadata.get("asset_state").and_then(toml::Value::as_str),
        Some("procedural_robot" | "packaged_asset")
    )
}

fn is_packaged_asset_view(metadata: &BTreeMap<String, toml::Value>) -> bool {
    metadata.get("asset_state").and_then(toml::Value::as_str) == Some("packaged_asset")
}

fn parse_endpoint(endpoint: &str) -> Option<(String, String)> {
    let (component, port) = endpoint.split_once('.')?;
    if component.is_empty() || port.is_empty() || port.contains('.') {
        return None;
    }
    Some((component.to_owned(), port.to_owned()))
}

fn param_number(params: &BTreeMap<String, toml::Value>, name: &str) -> Option<f64> {
    match params.get(name)? {
        toml::Value::Float(value) => Some(*value),
        toml::Value::Integer(value) => Some(*value as f64),
        _ => None,
    }
}

fn param_vec3(params: &BTreeMap<String, toml::Value>, name: &str) -> Option<[f64; 3]> {
    let values = params.get(name)?.as_array()?;
    toml_array_to_vec3(values)
}

fn toml_vec3(value: &toml::Value) -> Option<[f64; 3]> {
    let values = value.as_array()?;
    toml_array_to_vec3(values)
}

fn toml_array_to_vec3(values: &[toml::Value]) -> Option<[f64; 3]> {
    if values.len() != 3 {
        return None;
    }
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        result[index] = match value {
            toml::Value::Float(value) => *value,
            toml::Value::Integer(value) => *value as f64,
            _ => return None,
        };
    }
    Some(result)
}

fn grid_position(cell: &str, origin: [f64; 3], cell_size: f64) -> Option<[f64; 3]> {
    let split = cell
        .char_indices()
        .find(|(_, character)| character.is_ascii_digit())
        .map(|(index, _)| index)?;
    let (row_text, column_text) = cell.split_at(split);
    if row_text.is_empty() || column_text.is_empty() {
        return None;
    }
    let mut row = 0_u32;
    for character in row_text.chars() {
        if !character.is_ascii_alphabetic() {
            return None;
        }
        row = row
            .checked_mul(26)?
            .checked_add(u32::from(character.to_ascii_uppercase() as u8 - b'A') + 1)?;
    }
    let column = column_text.parse::<u32>().ok()?;
    if column == 0 || row == 0 {
        return None;
    }
    Some([
        origin[0] + f64::from(column - 1) * cell_size,
        origin[1],
        origin[2] + f64::from(row - 1) * cell_size,
    ])
}

fn add_vec3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn sub_vec3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn write_string_field(output: &mut String, name: &str, value: &str) {
    writeln!(output, "{name} = \"{}\"", escape_toml_string(value)).expect("write to string");
}

fn write_vec3_field(output: &mut String, name: &str, value: [f64; 3]) {
    writeln!(
        output,
        "{name} = [{}, {}, {}]",
        number(value[0]),
        number(value[1]),
        number(value[2])
    )
    .expect("write to string");
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn normalize_interaction_event(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if matches!(normalized.as_str(), "click" | "touch" | "toggle") {
        Some(normalized)
    } else {
        None
    }
}

fn normalize_interaction_action(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized == "hmi.write" {
        Some(normalized)
    } else {
        None
    }
}

fn normalize_interaction_required_role(value: &str) -> Option<String> {
    if value.trim().eq_ignore_ascii_case("engineer") {
        Some("Engineer".to_string())
    } else {
        None
    }
}

fn supported_interaction_value(value: &toml::Value) -> bool {
    matches!(
        value,
        toml::Value::String(_)
            | toml::Value::Integer(_)
            | toml::Value::Float(_)
            | toml::Value::Boolean(_)
    )
}

fn toml_interaction_value_literal(value: &toml::Value) -> String {
    toml_value_literal(value)
}

fn toml_value_literal(value: &toml::Value) -> String {
    match value {
        toml::Value::String(value) => format!("\"{}\"", escape_toml_string(value)),
        toml::Value::Integer(value) => value.to_string(),
        toml::Value::Float(value) => number(*value),
        toml::Value::Boolean(value) => value.to_string(),
        toml::Value::Datetime(value) => value.to_string(),
        toml::Value::Array(_) | toml::Value::Table(_) => "\"unsupported\"".to_string(),
    }
}

fn number(value: f64) -> String {
    let value = if value.abs() < 0.000_000_1 {
        0.0
    } else {
        value
    };
    let mut text = format!("{value:.6}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.push('0');
    }
    text
}

fn diagnostic(
    code: &str,
    severity: TopologyDiagnosticSeverity,
    message: String,
) -> TopologyDiagnostic {
    TopologyDiagnostic {
        code: code.to_owned(),
        severity,
        message,
    }
}

fn doctor(rule: &str, message: &str) -> DoctorRuleResult {
    DoctorRuleResult {
        rule: rule.to_owned(),
        passed: true,
        message: message.to_owned(),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut text, "{byte:02x}").expect("write to string");
    }
    text
}

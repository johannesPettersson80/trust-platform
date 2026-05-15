use sha2::{Digest, Sha256};

const TRUST_TWIN_SCENA_VERSION: &str = "1.0.1";
const TRUST_TWIN_PROOF_WIDTH: u32 = 160;
const TRUST_TWIN_PROOF_HEIGHT: u32 = 120;
const TRUST_TWIN_BACKGROUND_RGB: [u8; 3] = [7, 10, 18];
const TRUST_TWIN_P1_REFERENCE_FRAME_SHA256: &str =
    "98f36226810d1c738028172ebb817ff8035817850bf6b388c042ac51ef500e1f";

#[derive(Debug)]
pub struct TrustTwinScene {
    scene: scena::Scene,
    assets: scena::Assets,
    nodes: BTreeMap<String, TrustTwinSceneNodeRuntime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrustTwinSceneNodeState {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    pub visible: bool,
    pub material: TrustTwinMaterialState,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrustTwinMaterialState {
    pub base_color: String,
    pub emissive: String,
    pub opacity: f32,
}

#[derive(Debug)]
struct TrustTwinSceneNodeRuntime {
    key: scena::NodeKey,
    material: Option<scena::MaterialHandle>,
    state: TrustTwinSceneNodeState,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrustTwinBindingApplyReport {
    pub applied: Vec<TrustTwinAppliedBinding>,
    pub missing_sources: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrustTwinAppliedBinding {
    pub node: String,
    pub property: String,
    pub source: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrustTwinStaticViewArtifact {
    pub driver: &'static str,
    pub driver_version: &'static str,
    pub proof_image: String,
    pub visual: TrustTwinVisualProof,
    pub baseline: TrustTwinBaselineMetadata,
    pub capability_report: TrustTwinCapabilityReport,
    pub binding: TrustTwinBindingArtifact,
    pub evidence_blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrustTwinVisualProof {
    pub width: u32,
    pub height: u32,
    pub frame_sha256: String,
    pub non_background_pixels: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrustTwinBaselineMetadata {
    pub reference_image: String,
    pub reference_frame_sha256: &'static str,
    pub backend_profile: &'static str,
    pub browser_or_driver_version: String,
    pub fixture: BTreeMap<String, serde_json::Value>,
    pub tolerance: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrustTwinCapabilityReport {
    pub backend: String,
    pub hardware_tier: String,
    pub gpu_device: bool,
    pub surface_attached: bool,
    pub forward_pbr: String,
    pub directional_shadows: String,
    pub point_shadows: String,
    pub spot_shadows: String,
    pub readback_headless_screenshots: String,
    pub diagnostics: Vec<TrustTwinCapabilityDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrustTwinCapabilityDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub help: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrustTwinBindingArtifact {
    pub applied_count: usize,
    pub missing_sources: Vec<String>,
    pub errors: Vec<String>,
}

pub fn build_trust_twin_scene(view: &HmiSceneViewPayload) -> anyhow::Result<TrustTwinScene> {
    let assets = scena::Assets::new();
    let mut scene = scena::Scene::new();
    let mut nodes = BTreeMap::new();

    for node in &view.nodes {
        let id = node.id.trim();
        if id.is_empty() {
            anyhow::bail!("scene3d node id must not be empty");
        }
        if nodes.contains_key(id) {
            anyhow::bail!("duplicate scene3d node id '{id}'");
        }
        let state = node_state_from_schema(node)?;
        let transform = scene_transform(&state);
        let (key, material) = if let Some(primitive) = node.primitive.as_deref() {
            let geometry = assets.create_geometry(geometry_for_primitive(primitive)?);
            let material = assets.create_material(material_desc_from_state(&state.material)?);
            let key = scene.mesh(geometry, material).transform(transform).add()?;
            (key, Some(material))
        } else if node.asset.is_some() {
            anyhow::bail!(
                "scene3d node '{id}' uses an asset reference; P1 renderer bridge supports primitive nodes only"
            );
        } else {
            (scene.add_empty(scene.root(), transform)?, None)
        };
        scene.set_visible(key, state.visible)?;
        scene.add_tag(key, id)?;
        nodes.insert(
            id.to_string(),
            TrustTwinSceneNodeRuntime {
                key,
                material,
                state,
            },
        );
    }

    if view.cameras.is_empty() {
        let camera = scene.add_default_camera()?;
        if view.nodes.iter().any(|node| node.primitive.is_some()) {
            scene.frame_all_with_assets(camera, &assets)?;
        }
    } else {
        let mut active_camera = None;
        for camera in &view.cameras {
            let position = camera.position.unwrap_or([0.0, 0.0, 4.0]);
            let mut descriptor = scena::PerspectiveCamera::default();
            if let Some(fov_degrees) = camera.fov_degrees {
                if fov_degrees.is_finite() && fov_degrees > 0.0 {
                    descriptor.vertical_fov = scena::Angle::from_degrees(fov_degrees as f32);
                }
            }
            let key = scene.add_perspective_camera(
                scene.root(),
                descriptor,
                scena::Transform::at(vec3(position)),
            )?;
            active_camera.get_or_insert(key);
        }
        if let Some(camera) = active_camera {
            scene.set_active_camera(camera)?;
        }
    }

    for light in &view.lights {
        let color = parse_color(light.color.as_deref().unwrap_or("#ffffff"))?;
        let intensity = light.intensity.unwrap_or(1.0).max(0.0) as f32;
        let transform =
            scena::Transform::at(vec3(light.position.unwrap_or([0.0, 2.0, 2.0])))
                .rotate_x_deg(-30.0)
                .rotate_y_deg(20.0);
        match light.kind.as_deref().unwrap_or("directional") {
            "point" => {
                scene
                    .point_light(
                        scena::PointLight::default()
                            .with_color(color)
                            .with_intensity_candela(intensity * 100.0),
                    )
                    .transform(transform)
                    .add()?;
            }
            "spot" => {
                scene
                    .spot_light(
                        scena::SpotLight::default()
                            .with_color(color)
                            .with_intensity_candela(intensity * 100.0),
                    )
                    .transform(transform)
                    .add()?;
            }
            _ => {
                scene
                    .directional_light(
                        scena::DirectionalLight::default()
                            .with_color(color)
                            .with_illuminance_lux(intensity * 10_000.0),
                    )
                    .transform(transform)
                    .add()?;
            }
        }
    }

    Ok(TrustTwinScene {
        scene,
        assets,
        nodes,
    })
}

pub fn write_trust_twin_static_view_proof(
    view: &HmiSceneViewPayload,
    values: &BTreeMap<String, Value>,
    artifact_dir: &std::path::Path,
) -> anyhow::Result<TrustTwinStaticViewArtifact> {
    std::fs::create_dir_all(artifact_dir)?;
    let artifact_dir = artifact_dir.canonicalize()?;
    let mut trust_twin_scene = build_trust_twin_scene(view)?;
    let binding_report = trust_twin_scene.apply_bindings(&view.bindings3d, values)?;
    let options = scena::RendererOptions::default()
        .with_profile(scena::Profile::Industrial)
        .with_quality(scena::Quality::Low)
        .with_render_mode(scena::RenderMode::Manual);
    let mut renderer =
        scena::Renderer::headless_with_options(TRUST_TWIN_PROOF_WIDTH, TRUST_TWIN_PROOF_HEIGHT, options)?;
    renderer.set_background_color(scena::Color::from_srgb_u8(
        TRUST_TWIN_BACKGROUND_RGB[0],
        TRUST_TWIN_BACKGROUND_RGB[1],
        TRUST_TWIN_BACKGROUND_RGB[2],
    ));
    renderer.prepare_with_assets(&mut trust_twin_scene.scene, &trust_twin_scene.assets)?;
    renderer.render_active(&trust_twin_scene.scene)?;

    let frame = renderer.frame_rgba8();
    let frame_sha256 = sha256_hex(frame);
    let non_background_pixels = count_non_background_pixels(frame, TRUST_TWIN_BACKGROUND_RGB);
    let proof_image_path = artifact_dir.join("trust-twin-p1-static-view.ppm");
    write_ppm_rgb(
        &proof_image_path,
        TRUST_TWIN_PROOF_WIDTH,
        TRUST_TWIN_PROOF_HEIGHT,
        frame,
    )?;
    let evidence_blockers =
        evidence_blockers(non_background_pixels, &frame_sha256, &binding_report);

    let artifact = TrustTwinStaticViewArtifact {
        driver: "scena",
        driver_version: TRUST_TWIN_SCENA_VERSION,
        proof_image: proof_image_path.display().to_string(),
        visual: TrustTwinVisualProof {
            width: TRUST_TWIN_PROOF_WIDTH,
            height: TRUST_TWIN_PROOF_HEIGHT,
            frame_sha256,
            non_background_pixels,
        },
        baseline: baseline_metadata(view),
        capability_report: capability_report(&renderer),
        binding: TrustTwinBindingArtifact {
            applied_count: binding_report.applied.len(),
            missing_sources: binding_report.missing_sources.clone(),
            errors: binding_report.errors.clone(),
        },
        evidence_blockers,
    };

    let artifact_path = artifact_dir.join("trust-twin-p1-static-view.json");
    let payload = serde_json::to_string_pretty(&artifact)?;
    std::fs::write(artifact_path, payload)?;
    Ok(artifact)
}

impl TrustTwinScene {
    pub fn scena_scene(&self) -> &scena::Scene {
        &self.scene
    }

    pub fn node_key(&self, id: &str) -> Option<scena::NodeKey> {
        self.nodes.get(id).map(|node| node.key)
    }

    pub fn node_state(&self, id: &str) -> Option<&TrustTwinSceneNodeState> {
        self.nodes.get(id).map(|node| &node.state)
    }

    pub fn apply_bindings(
        &mut self,
        bindings: &[HmiSceneBindingSchema],
        values: &BTreeMap<String, Value>,
    ) -> anyhow::Result<TrustTwinBindingApplyReport> {
        let mut report = TrustTwinBindingApplyReport {
            applied: Vec::new(),
            missing_sources: Vec::new(),
            errors: Vec::new(),
        };
        for binding in bindings {
            let Some(value) = values.get(&binding.source) else {
                report.missing_sources.push(binding.source.clone());
                continue;
            };
            match self.apply_binding(binding, value) {
                Ok(()) => report.applied.push(TrustTwinAppliedBinding {
                    node: binding.node.clone(),
                    property: binding.property.clone(),
                    source: binding.source.clone(),
                    value: value_to_json(value),
                }),
                Err(error) => report.errors.push(error.to_string()),
            }
        }
        Ok(report)
    }

    fn apply_binding(&mut self, binding: &HmiSceneBindingSchema, value: &Value) -> anyhow::Result<()> {
        if !self.nodes.contains_key(&binding.node) {
            anyhow::bail!("unknown scene3d node '{}'", binding.node);
        }
        match binding.property.as_str() {
            "visible" => self.set_visible(&binding.node, binding_bool_value(binding, value)?),
            "transform.position" => self.set_position(&binding.node, binding_vec3_value(binding, value)?),
            "transform.position.x" => {
                self.set_position_axis(&binding.node, 0, binding_numeric_value(binding, value)?)
            }
            "transform.position.y" => {
                self.set_position_axis(&binding.node, 1, binding_numeric_value(binding, value)?)
            }
            "transform.position.z" => {
                self.set_position_axis(&binding.node, 2, binding_numeric_value(binding, value)?)
            }
            "transform.rotation.x" => {
                self.set_rotation_axis(&binding.node, 0, binding_numeric_value(binding, value)?)
            }
            "transform.rotation.y" => {
                self.set_rotation_axis(&binding.node, 1, binding_numeric_value(binding, value)?)
            }
            "transform.rotation.z" => {
                self.set_rotation_axis(&binding.node, 2, binding_numeric_value(binding, value)?)
            }
            "transform.scale" => self.set_scale(&binding.node, binding_scale_value(binding, value)?),
            "transform.scale.x" => {
                self.set_scale_axis(&binding.node, 0, binding_numeric_value(binding, value)?)
            }
            "transform.scale.y" => {
                self.set_scale_axis(&binding.node, 1, binding_numeric_value(binding, value)?)
            }
            "transform.scale.z" => {
                self.set_scale_axis(&binding.node, 2, binding_numeric_value(binding, value)?)
            }
            "material.base_color" => {
                self.set_material_base_color(&binding.node, binding_text_value(binding, value)?)
            }
            "material.emissive" => {
                self.set_material_emissive(&binding.node, binding_text_value(binding, value)?)
            }
            "material.opacity" => {
                self.set_material_opacity(&binding.node, binding_numeric_value(binding, value)?)
            }
            "text.value" => self.set_text(&binding.node, binding_text_value(binding, value)?),
            other => anyhow::bail!("unsupported bind3d property '{other}'"),
        }
    }

    fn set_visible(&mut self, node: &str, visible: bool) -> anyhow::Result<()> {
        let runtime = self.nodes.get_mut(node).expect("node existence checked");
        runtime.state.visible = visible;
        self.scene.set_visible(runtime.key, visible)?;
        Ok(())
    }

    fn set_position(&mut self, node: &str, position: [f32; 3]) -> anyhow::Result<()> {
        self.update_transform(node, |state| state.position = position)
    }

    fn set_position_axis(&mut self, node: &str, axis: usize, value: f32) -> anyhow::Result<()> {
        self.update_transform(node, |state| state.position[axis] = value)
    }

    fn set_rotation_axis(&mut self, node: &str, axis: usize, value: f32) -> anyhow::Result<()> {
        self.update_transform(node, |state| state.rotation[axis] = value)
    }

    fn set_scale(&mut self, node: &str, scale: [f32; 3]) -> anyhow::Result<()> {
        self.update_transform(node, |state| state.scale = scale)
    }

    fn set_scale_axis(&mut self, node: &str, axis: usize, value: f32) -> anyhow::Result<()> {
        self.update_transform(node, |state| state.scale[axis] = value)
    }

    fn update_transform(
        &mut self,
        node: &str,
        update: impl FnOnce(&mut TrustTwinSceneNodeState),
    ) -> anyhow::Result<()> {
        let runtime = self.nodes.get_mut(node).expect("node existence checked");
        update(&mut runtime.state);
        let key = runtime.key;
        let transform = scene_transform(&runtime.state);
        self.scene.set_transform(key, transform)?;
        Ok(())
    }

    fn set_material_base_color(&mut self, node: &str, color: String) -> anyhow::Result<()> {
        self.update_material(node, |state| state.base_color = color)
    }

    fn set_material_emissive(&mut self, node: &str, color: String) -> anyhow::Result<()> {
        self.update_material(node, |state| state.emissive = color)
    }

    fn set_material_opacity(&mut self, node: &str, opacity: f32) -> anyhow::Result<()> {
        self.update_material(node, |state| state.opacity = opacity.clamp(0.0, 1.0))
    }

    fn update_material(
        &mut self,
        node: &str,
        update: impl FnOnce(&mut TrustTwinMaterialState),
    ) -> anyhow::Result<()> {
        let runtime = self.nodes.get_mut(node).expect("node existence checked");
        if runtime.material.is_none() {
            anyhow::bail!("scene3d node '{node}' has no mesh material");
        }
        update(&mut runtime.state.material);
        let key = runtime.key;
        let material = material_desc_from_state(&runtime.state.material)?;
        let handle = self.assets.create_material(material);
        runtime.material = Some(handle);
        self.scene.set_mesh_material(key, handle)?;
        Ok(())
    }

    fn set_text(&mut self, node: &str, text: String) -> anyhow::Result<()> {
        let runtime = self.nodes.get_mut(node).expect("node existence checked");
        runtime.state.text = Some(text);
        Ok(())
    }
}

fn node_state_from_schema(node: &HmiSceneNodeSchema) -> anyhow::Result<TrustTwinSceneNodeState> {
    let transform = node.transform.clone().unwrap_or_default();
    let material = node.material.clone().unwrap_or_default();
    Ok(TrustTwinSceneNodeState {
        position: transform.position.map(vec3_to_f32).unwrap_or([0.0, 0.0, 0.0]),
        rotation: transform.rotation.map(vec3_to_f32).unwrap_or([0.0, 0.0, 0.0]),
        scale: transform.scale.map(vec3_to_f32).unwrap_or([1.0, 1.0, 1.0]),
        visible: true,
        material: TrustTwinMaterialState {
            base_color: material
                .base_color
                .unwrap_or_else(|| "#3b82f6".to_string()),
            emissive: material.emissive.unwrap_or_else(|| "#000000".to_string()),
            opacity: material.opacity.unwrap_or(1.0).clamp(0.0, 1.0) as f32,
        },
        text: node.label.clone(),
    })
}

fn geometry_for_primitive(value: &str) -> anyhow::Result<scena::GeometryDesc> {
    match value.trim().to_ascii_lowercase().as_str() {
        "box" | "cube" | "motor" | "shaft" => Ok(scena::GeometryDesc::box_xyz(1.0, 1.0, 1.0)),
        "plate" => Ok(scena::GeometryDesc::box_xyz(1.4, 0.12, 0.9)),
        "grid" | "floor" => Ok(scena::GeometryDesc::grid(2.0, 8)),
        other => anyhow::bail!("unsupported scene3d primitive '{other}'"),
    }
}

fn scene_transform(state: &TrustTwinSceneNodeState) -> scena::Transform {
    scena::Transform {
        translation: scena::Vec3::new(state.position[0], state.position[1], state.position[2]),
        rotation: scena::Quat::from_rotation_x(state.rotation[0])
            * scena::Quat::from_rotation_y(state.rotation[1])
            * scena::Quat::from_rotation_z(state.rotation[2]),
        scale: scena::Vec3::new(state.scale[0], state.scale[1], state.scale[2]),
    }
}

fn material_desc_from_state(state: &TrustTwinMaterialState) -> anyhow::Result<scena::MaterialDesc> {
    let mut base_color = parse_color(&state.base_color)?;
    base_color.a = state.opacity.clamp(0.0, 1.0);
    let material = scena::MaterialDesc::unlit(base_color)
        .with_emissive(parse_color(&state.emissive)?)
        .with_alpha_mode(if state.opacity < 1.0 {
            scena::AlphaMode::Blend
        } else {
            scena::AlphaMode::Opaque
        });
    Ok(material)
}

fn binding_numeric_value(binding: &HmiSceneBindingSchema, value: &Value) -> anyhow::Result<f32> {
    let raw = mapped_text(binding, value)
        .map(|text| {
            text.parse::<f64>()
                .map_err(|err| anyhow::anyhow!("bind3d mapped value '{text}' is not numeric: {err}"))
        })
        .unwrap_or_else(|| value_to_f64(value))?;
    let scaled = if let Some(scale) = &binding.scale {
        let ratio = ((raw - scale.min) / (scale.max - scale.min)).clamp(0.0, 1.0);
        scale.output_min + ratio * (scale.output_max - scale.output_min)
    } else {
        raw
    };
    if !scaled.is_finite() {
        anyhow::bail!("bind3d source '{}' produced non-finite value", binding.source);
    }
    Ok(scaled as f32)
}

fn binding_bool_value(binding: &HmiSceneBindingSchema, value: &Value) -> anyhow::Result<bool> {
    if let Some(mapped) = mapped_text(binding, value) {
        return parse_bool(mapped)
            .ok_or_else(|| anyhow::anyhow!("bind3d mapped value '{mapped}' is not boolean"));
    }
    match value {
        Value::Bool(value) => Ok(*value),
        value => Ok(value_to_f64(value)? != 0.0),
    }
}

fn binding_text_value(binding: &HmiSceneBindingSchema, value: &Value) -> anyhow::Result<String> {
    if let Some(mapped) = mapped_text(binding, value) {
        return Ok(mapped.to_string());
    }
    match value {
        Value::Bool(value) => Ok(value.to_string()),
        Value::String(value) => Ok(value.to_string()),
        Value::WString(value) => Ok(value.clone()),
        Value::Char(value) => Ok(char::from_u32(u32::from(*value)).unwrap_or('?').to_string()),
        Value::WChar(value) => Ok(char::from_u32(u32::from(*value)).unwrap_or('?').to_string()),
        other => value_to_f64(other).map(|value| value.to_string()),
    }
}

fn binding_vec3_value(binding: &HmiSceneBindingSchema, value: &Value) -> anyhow::Result<[f32; 3]> {
    if let Some(mapped) = mapped_text(binding, value) {
        return parse_vec3_text(mapped);
    }
    match value {
        Value::Array(elements) if elements.elements().len() == 3 => {
            let mut result = [0.0; 3];
            for (index, element) in elements.elements().iter().enumerate() {
                result[index] = value_to_f64(element)? as f32;
            }
            Ok(result)
        }
        _ => parse_vec3_text(&binding_text_value(binding, value)?),
    }
}

fn binding_scale_value(binding: &HmiSceneBindingSchema, value: &Value) -> anyhow::Result<[f32; 3]> {
    if let Ok(vector) = binding_vec3_value(binding, value) {
        return Ok(vector);
    }
    let scale = binding_numeric_value(binding, value)?;
    Ok([scale, scale, scale])
}

fn mapped_text<'a>(binding: &'a HmiSceneBindingSchema, value: &Value) -> Option<&'a str> {
    if binding.map.is_empty() {
        return None;
    }
    let key = map_key(value);
    binding
        .map
        .get(&key)
        .or_else(|| binding.map.get(&key.to_ascii_lowercase()))
        .map(String::as_str)
}

fn map_key(value: &Value) -> String {
    match value {
        Value::Bool(value) => value.to_string(),
        Value::String(value) => value.to_string(),
        Value::WString(value) => value.clone(),
        Value::Char(value) => char::from_u32(u32::from(*value)).unwrap_or('?').to_string(),
        Value::WChar(value) => char::from_u32(u32::from(*value)).unwrap_or('?').to_string(),
        value => value_to_f64(value)
            .map(|value| value.to_string())
            .unwrap_or_else(|_| format!("{value:?}")),
    }
}

fn value_to_f64(value: &Value) -> anyhow::Result<f64> {
    match value {
        Value::Byte(value) => Ok(f64::from(*value)),
        Value::Word(value) => Ok(f64::from(*value)),
        Value::DWord(value) => Ok(f64::from(*value)),
        Value::LWord(value) => Ok(*value as f64),
        value => crate::numeric::to_f64(value).map_err(|err| anyhow::anyhow!("{err}")),
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "on" | "yes" => Some(true),
        "false" | "0" | "off" | "no" => Some(false),
        _ => None,
    }
}

fn parse_vec3_text(value: &str) -> anyhow::Result<[f32; 3]> {
    let trimmed = value.trim().trim_start_matches('[').trim_end_matches(']');
    let parts = trimmed
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() != 3 {
        anyhow::bail!("expected a 3D vector formatted as x,y,z");
    }
    let mut result = [0.0; 3];
    for (index, part) in parts.into_iter().enumerate() {
        result[index] = part.parse::<f32>()?;
    }
    Ok(result)
}

fn parse_color(value: &str) -> anyhow::Result<scena::Color> {
    scena::Color::from_hex_srgb(value.trim())
        .map_err(|err| anyhow::anyhow!("invalid scene3d color '{}': {err}", value.trim()))
}

fn vec3(value: [f64; 3]) -> scena::Vec3 {
    let value = vec3_to_f32(value);
    scena::Vec3::new(value[0], value[1], value[2])
}

fn vec3_to_f32(value: [f64; 3]) -> [f32; 3] {
    [value[0] as f32, value[1] as f32, value[2] as f32]
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn count_non_background_pixels(frame: &[u8], background_rgb: [u8; 3]) -> usize {
    frame
        .chunks_exact(4)
        .filter(|pixel| pixel[0..3] != background_rgb)
        .count()
}

fn write_ppm_rgb(
    path: &std::path::Path,
    width: u32,
    height: u32,
    frame: &[u8],
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
    bytes.reserve((width as usize) * (height as usize) * 3);
    for pixel in frame.chunks_exact(4) {
        bytes.extend_from_slice(&pixel[0..3]);
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

fn baseline_metadata(view: &HmiSceneViewPayload) -> TrustTwinBaselineMetadata {
    let mut fixture = BTreeMap::new();
    fixture.insert("nodes".to_string(), serde_json::json!(view.nodes.len()));
    fixture.insert("bindings3d".to_string(), serde_json::json!(view.bindings3d.len()));
    fixture.insert(
        "primary_binding".to_string(),
        view.bindings3d
            .first()
            .map(|binding| {
                serde_json::json!({
                    "node": binding.node,
                    "property": binding.property,
                    "source": binding.source,
                })
            })
            .unwrap_or(serde_json::Value::Null),
    );

    let mut tolerance = BTreeMap::new();
    tolerance.insert("non_background_pixels_min".to_string(), serde_json::json!(1));
    tolerance.insert("pixel_channel_tolerance".to_string(), serde_json::json!(0));
    tolerance.insert("backend".to_string(), serde_json::json!("Headless"));

    TrustTwinBaselineMetadata {
        reference_image: "target/gate-artifacts/trust-twin-p1-static-view.ppm".to_string(),
        reference_frame_sha256: TRUST_TWIN_P1_REFERENCE_FRAME_SHA256,
        backend_profile: "Industrial/Low/Headless",
        browser_or_driver_version: format!("scena {TRUST_TWIN_SCENA_VERSION}"),
        fixture,
        tolerance,
    }
}

fn capability_report(renderer: &scena::Renderer) -> TrustTwinCapabilityReport {
    let report = renderer.capability_report();
    let capabilities = report.capabilities();
    TrustTwinCapabilityReport {
        backend: format!("{:?}", capabilities.backend),
        hardware_tier: format!("{:?}", capabilities.hardware_tier),
        gpu_device: capabilities.gpu_device,
        surface_attached: capabilities.surface_attached,
        forward_pbr: format!("{:?}", capabilities.forward_pbr),
        directional_shadows: format!("{:?}", capabilities.directional_shadows),
        point_shadows: format!("{:?}", capabilities.point_shadows),
        spot_shadows: format!("{:?}", capabilities.spot_shadows),
        readback_headless_screenshots: format!("{:?}", capabilities.readback_headless_screenshots),
        diagnostics: report
            .diagnostics()
            .iter()
            .map(|diagnostic| TrustTwinCapabilityDiagnostic {
                code: format!("{:?}", diagnostic.code),
                severity: format!("{:?}", diagnostic.severity),
                message: diagnostic.message.clone(),
                help: diagnostic.help.clone(),
            })
            .collect(),
    }
}

fn evidence_blockers(
    non_background_pixels: usize,
    frame_sha256: &str,
    binding_report: &TrustTwinBindingApplyReport,
) -> Vec<String> {
    let mut blockers = Vec::new();
    if non_background_pixels == 0 {
        blockers.push("rendered frame contains no non-background pixels".to_string());
    }
    if frame_sha256 != TRUST_TWIN_P1_REFERENCE_FRAME_SHA256 {
        blockers.push("rendered frame hash does not match the P1 static-view reference".to_string());
    }
    if !binding_report.missing_sources.is_empty() {
        blockers.push("one or more bind3d sources were missing".to_string());
    }
    if !binding_report.errors.is_empty() {
        blockers.push("one or more bind3d updates failed".to_string());
    }
    blockers
}

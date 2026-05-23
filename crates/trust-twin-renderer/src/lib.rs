#![forbid(unsafe_code)]
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

const CONTRACT_VERSION: u32 = 2;

#[must_use]
pub fn trust_twin_renderer_contract_version() -> u32 {
    CONTRACT_VERSION
}

#[derive(Debug)]
struct RendererScene {
    scene: scena::Scene,
    assets: scena::Assets,
    scene_assets: BTreeMap<String, scena::SceneAsset>,
    solid_textures: BTreeMap<String, scena::TextureHandle>,
    nodes: BTreeMap<String, RuntimeNode>,
    cameras: Vec<CameraPayload>,
    lights: Vec<LightPayload>,
    bindings: Vec<BindingPayload>,
    offline: bool,
}

#[derive(Debug)]
struct RuntimeNode {
    key: Option<scena::NodeKey>,
    mesh_key: Option<scena::NodeKey>,
    asset_root_key: Option<scena::NodeKey>,
    asset_mesh_keys: Vec<scena::NodeKey>,
    attachment_key: Option<scena::NodeKey>,
    material: Option<scena::MaterialHandle>,
    material_overridden: bool,
    primitive: Option<String>,
    asset: Option<String>,
    parent_poses: BTreeMap<String, [f32; 3]>,
    state: NodeState,
}

#[derive(Debug, Clone)]
struct NodeState {
    parent: Option<String>,
    position: [f32; 3],
    rotation: [f32; 3],
    scale: [f32; 3],
    asset_rotation: [f32; 3],
    pivot: [f32; 3],
    visible: bool,
    material: MaterialState,
}

#[derive(Debug, Clone)]
struct MaterialState {
    base_color: String,
    emissive: String,
    opacity: f32,
}

#[derive(Debug, Default, Deserialize)]
struct ScenePayload {
    #[serde(default, rename = "asset")]
    assets: Vec<AssetPayload>,
    #[serde(default, rename = "node")]
    nodes: Vec<NodePayload>,
    #[serde(default, rename = "camera")]
    cameras: Vec<CameraPayload>,
    #[serde(default, rename = "light")]
    lights: Vec<LightPayload>,
    #[serde(default, rename = "bind3d")]
    bindings: Vec<BindingPayload>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct AssetPayload {
    id: String,
    #[serde(default)]
    uri: Option<String>,
    #[serde(default)]
    kind: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct NodePayload {
    id: String,
    #[serde(default)]
    parent: Option<String>,
    #[serde(default)]
    local_position: Option<[f64; 3]>,
    #[serde(default)]
    pivot: Option<[f64; 3]>,
    #[serde(default)]
    asset_rotation: Option<[f64; 3]>,
    #[serde(default)]
    asset: Option<String>,
    #[serde(default)]
    primitive: Option<String>,
    #[serde(default)]
    transform: Option<TransformPayload>,
    #[serde(default)]
    material: Option<MaterialPayload>,
    #[serde(default, rename = "parent_pose")]
    parent_poses: Vec<ParentPosePayload>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TransformPayload {
    #[serde(default)]
    position: Option<[f64; 3]>,
    #[serde(default)]
    rotation: Option<[f64; 3]>,
    #[serde(default)]
    scale: Option<[f64; 3]>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct MaterialPayload {
    #[serde(default)]
    base_color: Option<String>,
    #[serde(default)]
    emissive: Option<String>,
    #[serde(default)]
    opacity: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct ParentPosePayload {
    parent: String,
    local_position: [f64; 3],
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CameraPayload {
    #[serde(default)]
    position: Option<[f64; 3]>,
    #[serde(default)]
    target: Option<[f64; 3]>,
    #[serde(default)]
    fov_degrees: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct LightPayload {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    position: Option<[f64; 3]>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    intensity: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct BindingPayload {
    node: String,
    property: String,
    source: String,
    #[serde(default)]
    map: BTreeMap<String, String>,
    #[serde(default)]
    scale: Option<ScalePayload>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ScalePayload {
    min: f64,
    max: f64,
    output_min: f64,
    output_max: f64,
}

impl RendererScene {
    #[cfg(test)]
    fn from_json(payload_json: &str) -> anyhow::Result<Self> {
        let payload: ScenePayload = serde_json::from_str(payload_json)?;
        Self::from_payload(
            payload,
            scena::Assets::new(),
            BTreeMap::new(),
            BTreeMap::new(),
        )
    }

    async fn from_json_with_assets(
        payload_json: &str,
        assets: scena::Assets,
        solid_textures: BTreeMap<String, scena::TextureHandle>,
    ) -> anyhow::Result<Self> {
        let payload: ScenePayload = serde_json::from_str(payload_json)?;
        let scene_assets = load_scene_assets(&payload.assets, &assets).await?;
        Self::from_payload(payload, assets, solid_textures, scene_assets)
    }

    fn from_payload(
        payload: ScenePayload,
        assets: scena::Assets,
        solid_textures: BTreeMap<String, scena::TextureHandle>,
        scene_assets: BTreeMap<String, scena::SceneAsset>,
    ) -> anyhow::Result<Self> {
        let mut nodes = BTreeMap::new();
        for node in payload.nodes {
            let id = node.id.trim();
            if id.is_empty() {
                anyhow::bail!("scene node id must not be empty");
            }
            if nodes.contains_key(id) {
                anyhow::bail!("duplicate scene node id '{id}'");
            }
            let parent_poses = node
                .parent_poses
                .iter()
                .map(|pose| {
                    (
                        pose.parent.trim().to_string(),
                        vec3_to_f32(pose.local_position),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let material_overridden = node.asset.is_none() && node.material.is_some();
            let state = node_state(&node);
            nodes.insert(
                id.to_string(),
                RuntimeNode {
                    key: None,
                    mesh_key: None,
                    asset_root_key: None,
                    asset_mesh_keys: Vec::new(),
                    attachment_key: None,
                    material: None,
                    material_overridden,
                    primitive: node.primitive,
                    asset: node.asset,
                    parent_poses,
                    state,
                },
            );
        }

        let mut renderer_scene = Self {
            scene: scena::Scene::new(),
            assets,
            scene_assets,
            solid_textures,
            nodes,
            cameras: payload.cameras,
            lights: payload.lights,
            bindings: payload.bindings,
            offline: false,
        };
        renderer_scene.rebuild_scene_graph()?;
        Ok(renderer_scene)
    }

    fn apply_values_json(&mut self, values_json: &str) -> anyhow::Result<()> {
        let values: BTreeMap<String, serde_json::Value> = serde_json::from_str(values_json)?;
        let bindings = self.bindings.clone();
        for binding in &bindings {
            let Some(value) = values.get(&binding.source) else {
                continue;
            };
            self.apply_binding(binding, value)?;
        }
        Ok(())
    }

    fn apply_binding(
        &mut self,
        binding: &BindingPayload,
        value: &serde_json::Value,
    ) -> anyhow::Result<()> {
        if !self.nodes.contains_key(&binding.node) {
            anyhow::bail!("unknown scene node '{}'", binding.node);
        }
        match binding.property.trim().to_ascii_lowercase().as_str() {
            "visible" => self.set_visible(&binding.node, bool_value(binding, value)?),
            "parent" => {
                let parent = text_value(binding, value)?;
                self.set_parent(&binding.node, &parent)
            }
            "transform.position" => self.set_position(&binding.node, vec3_value(binding, value)?),
            "transform.position.x" => {
                self.set_position_axis(&binding.node, 0, number_value(binding, value)?)
            }
            "transform.position.y" => {
                self.set_position_axis(&binding.node, 1, number_value(binding, value)?)
            }
            "transform.position.z" => {
                self.set_position_axis(&binding.node, 2, number_value(binding, value)?)
            }
            "transform.rotation.x" => {
                self.set_rotation_axis(&binding.node, 0, number_value(binding, value)?)
            }
            "transform.rotation.y" => {
                self.set_rotation_axis(&binding.node, 1, number_value(binding, value)?)
            }
            "transform.rotation.z" => {
                self.set_rotation_axis(&binding.node, 2, number_value(binding, value)?)
            }
            "transform.scale" => self.set_scale(&binding.node, scale_value(binding, value)?),
            "transform.scale.x" => {
                self.set_scale_axis(&binding.node, 0, number_value(binding, value)?)
            }
            "transform.scale.y" => {
                self.set_scale_axis(&binding.node, 1, number_value(binding, value)?)
            }
            "transform.scale.z" => {
                self.set_scale_axis(&binding.node, 2, number_value(binding, value)?)
            }
            "material.base_color" => {
                self.set_material_base_color(&binding.node, text_value(binding, value)?)
            }
            "material.emissive" => {
                self.set_material_emissive(&binding.node, text_value(binding, value)?)
            }
            "material.opacity" => {
                self.set_material_opacity(&binding.node, number_value(binding, value)?)
            }
            other => anyhow::bail!("unsupported bind3d property '{other}'"),
        }
    }

    fn rebuild_scene_graph(&mut self) -> anyhow::Result<()> {
        self.scene = scena::Scene::new();
        for runtime in self.nodes.values_mut() {
            runtime.key = None;
            runtime.mesh_key = None;
            runtime.asset_root_key = None;
            runtime.asset_mesh_keys.clear();
            runtime.attachment_key = None;
            runtime.material = None;
        }

        let mut pending = self.nodes.keys().cloned().collect::<BTreeSet<_>>();
        while !pending.is_empty() {
            let before = pending.len();
            for id in pending.iter().cloned().collect::<Vec<_>>() {
                let parent_id = self
                    .nodes
                    .get(&id)
                    .and_then(|runtime| runtime.state.parent.as_deref());
                let parent_key = match parent_id {
                    Some(parent) => {
                        let Some(parent_runtime) = self.nodes.get(parent) else {
                            anyhow::bail!("scene node '{id}' references missing parent '{parent}'");
                        };
                        match parent_runtime.key {
                            Some(key) => key,
                            None => continue,
                        }
                    }
                    None => self.scene.root(),
                };
                let key = self.insert_node(&id, parent_key)?;
                self.scene.add_tag(key, &id)?;
                self.scene
                    .set_visible(key, self.nodes.get(&id).expect("node exists").state.visible)?;
                self.nodes.get_mut(&id).expect("node exists").key = Some(key);
                pending.remove(&id);
            }
            if pending.len() == before {
                anyhow::bail!("scene parent graph contains a cycle: {pending:?}");
            }
        }

        self.install_cameras()?;
        self.install_lights()?;
        Ok(())
    }

    fn insert_node(
        &mut self,
        id: &str,
        parent_key: scena::NodeKey,
    ) -> anyhow::Result<scena::NodeKey> {
        let runtime = self.nodes.get(id).expect("node exists");
        let group_key = self
            .scene
            .add_empty(parent_key, node_group_transform(&runtime.state))?;
        let mut mesh_key = None;
        let mut asset_root_key = None;
        let mut asset_mesh_keys = Vec::new();
        let mut material_handle = None;
        if let Some(asset) = runtime.asset.as_deref() {
            let scene_asset = self.scene_assets.get(asset).ok_or_else(|| {
                anyhow::anyhow!(
                    "scene node '{id}' references asset '{asset}', but that asset was not loaded"
                )
            })?;
            let asset_key = self
                .scene
                .add_empty(group_key, node_mesh_transform(&runtime.state))?;
            let mut asset_material_override = None;
            if self.offline || runtime.material_overridden {
                let texture = material_texture_handle(
                    &runtime.state.material,
                    self.offline,
                    &self.solid_textures,
                );
                let material = self.assets.create_material(material_desc(
                    &runtime.state.material,
                    self.offline,
                    texture,
                )?);
                material_handle = Some(material);
                asset_material_override = Some(material);
            }
            instantiate_asset_under(
                &mut self.scene,
                scene_asset,
                asset_key,
                asset_material_override,
                &mut asset_mesh_keys,
            )?;
            asset_root_key = Some(asset_key);
        } else if let Some(primitive) = runtime.primitive.as_deref() {
            let geometry = self
                .assets
                .create_geometry(geometry_for_primitive(primitive)?);
            let material = self.assets.create_material(material_desc_for_primitive(
                primitive,
                &runtime.state.material,
                self.offline,
                &self.solid_textures,
            )?);
            let created_mesh_key = self
                .scene
                .mesh(geometry, material)
                .parent(group_key)
                .transform(node_mesh_transform(&runtime.state))
                .add()?;
            mesh_key = Some(created_mesh_key);
            material_handle = Some(material);
        }
        let attachment_key = self
            .scene
            .add_empty(group_key, scena::Transform::IDENTITY)?;
        let runtime = self.nodes.get_mut(id).expect("node exists");
        runtime.mesh_key = mesh_key;
        runtime.asset_root_key = asset_root_key;
        runtime.asset_mesh_keys = asset_mesh_keys;
        runtime.attachment_key = Some(attachment_key);
        runtime.material = material_handle;
        Ok(group_key)
    }

    fn install_cameras(&mut self) -> anyhow::Result<()> {
        if self.cameras.is_empty() {
            let camera = self.scene.add_default_camera()?;
            self.scene.frame_all_with_assets(camera, &self.assets)?;
            return Ok(());
        }

        let mut active = None;
        for camera in &self.cameras {
            let mut descriptor = scena::PerspectiveCamera::standard();
            if let Some(fov) = camera.fov_degrees {
                if fov.is_finite() && fov > 0.0 {
                    descriptor.vertical_fov = scena::Angle::from_degrees(f32_from_f64(fov));
                }
            }
            let key = self.scene.add_perspective_camera(
                self.scene.root(),
                descriptor,
                scena::Transform::at(vec3(camera.position.unwrap_or([0.0, 0.0, 4.0]))),
            )?;
            if let Some(target) = camera.target {
                self.scene.look_at_point(key, vec3(target))?;
            }
            active.get_or_insert(key);
        }
        if let Some(camera) = active {
            self.scene.set_active_camera(camera)?;
        }
        Ok(())
    }

    fn install_lights(&mut self) -> anyhow::Result<()> {
        if self.lights.is_empty() {
            self.scene.add_studio_lighting()?;
        }
        for light in &self.lights {
            let color = parse_color(light.color.as_deref().unwrap_or("#ffffff"))?;
            let intensity = f32_from_f64(light.intensity.unwrap_or(1.0).max(0.0));
            let transform = scena::Transform::at(vec3(light.position.unwrap_or([0.0, 2.0, 2.0])))
                .rotate_x_deg(-30.0)
                .rotate_y_deg(20.0);
            match light.kind.as_deref().unwrap_or("directional") {
                "point" => {
                    self.scene
                        .point_light(
                            scena::PointLight::softbox()
                                .with_color(color)
                                .with_intensity_candela(intensity * 100.0),
                        )
                        .transform(transform)
                        .add()?;
                }
                "spot" => {
                    self.scene
                        .spot_light(
                            scena::SpotLight::default()
                                .with_color(color)
                                .with_intensity_candela(intensity * 100.0),
                        )
                        .transform(transform)
                        .add()?;
                }
                _ => {
                    self.scene
                        .directional_light(
                            scena::DirectionalLight::key_light()
                                .with_color(color)
                                .with_illuminance_lux(intensity * 10_000.0),
                        )
                        .transform(transform)
                        .add()?;
                }
            }
        }
        Ok(())
    }

    fn set_visible(&mut self, node: &str, visible: bool) -> anyhow::Result<()> {
        let runtime = self.nodes.get_mut(node).expect("node checked");
        runtime.state.visible = visible;
        self.scene
            .set_visible(runtime.key.expect("scene graph built"), visible)?;
        Ok(())
    }

    fn set_parent(&mut self, node: &str, parent: &str) -> anyhow::Result<()> {
        let parent = normalize_parent(parent);
        if parent.as_deref() == Some(node) {
            anyhow::bail!("scene node '{node}' cannot parent itself");
        }
        if let Some(parent_id) = parent.as_deref() {
            if !self.nodes.contains_key(parent_id) {
                anyhow::bail!("scene node '{node}' references missing parent '{parent_id}'");
            }
        }
        let local_position = parent.as_deref().and_then(|parent_id| {
            self.nodes
                .get(node)
                .expect("node checked")
                .parent_poses
                .get(parent_id)
                .copied()
        });
        let source_key = self.nodes.get(node).expect("node checked").key;
        let target_attachment_key = parent
            .as_deref()
            .and_then(|parent_id| self.nodes.get(parent_id))
            .and_then(|runtime| runtime.attachment_key);
        let runtime = self.nodes.get_mut(node).expect("node checked");
        if let Some(local_position) = local_position {
            runtime.state.position = local_position;
        }
        if parent.is_some() {
            let source = scena::ConnectorFrame::new(
                source_key.expect("scene graph built"),
                scena::Transform::IDENTITY,
            );
            let target = scena::ConnectorFrame::new(
                target_attachment_key.expect("parent attachment exists"),
                scena::Transform::at(scena::Vec3::new(
                    runtime.state.position[0],
                    runtime.state.position[1],
                    runtime.state.position[2],
                )),
            );
            self.scene.connect(
                source,
                target,
                scena::ConnectOptions::default().reparent_source_to_target_parent(),
            )?;
        }
        runtime.state.parent = parent;
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
        update: impl FnOnce(&mut NodeState),
    ) -> anyhow::Result<()> {
        let runtime = self.nodes.get_mut(node).expect("node checked");
        update(&mut runtime.state);
        self.scene.set_transform(
            runtime.key.expect("scene graph built"),
            node_group_transform(&runtime.state),
        )?;
        if let Some(mesh_key) = runtime.mesh_key {
            self.scene
                .set_transform(mesh_key, node_mesh_transform(&runtime.state))?;
        }
        if let Some(asset_root_key) = runtime.asset_root_key {
            self.scene
                .set_transform(asset_root_key, node_mesh_transform(&runtime.state))?;
        }
        Ok(())
    }

    fn set_material_base_color(&mut self, node: &str, color: String) -> anyhow::Result<()> {
        self.update_material(node, |material| material.base_color = color)
    }

    fn set_material_emissive(&mut self, node: &str, color: String) -> anyhow::Result<()> {
        self.update_material(node, |material| material.emissive = color)
    }

    fn set_material_opacity(&mut self, node: &str, opacity: f32) -> anyhow::Result<()> {
        self.update_material(node, |material| material.opacity = opacity.clamp(0.0, 1.0))
    }

    fn update_material(
        &mut self,
        node: &str,
        update: impl FnOnce(&mut MaterialState),
    ) -> anyhow::Result<()> {
        let runtime = self.nodes.get_mut(node).expect("node checked");
        update(&mut runtime.state.material);
        runtime.material_overridden = true;
        let texture =
            material_texture_handle(&runtime.state.material, self.offline, &self.solid_textures);
        let handle = self.assets.create_material(material_desc(
            &runtime.state.material,
            self.offline,
            texture,
        )?);
        runtime.material = Some(handle);
        if let Some(mesh_key) = runtime.mesh_key {
            self.scene.set_mesh_material(mesh_key, handle)?;
            return Ok(());
        }
        if !runtime.asset_mesh_keys.is_empty() {
            for mesh_key in &runtime.asset_mesh_keys {
                self.scene.set_mesh_material(*mesh_key, handle)?;
            }
            return Ok(());
        }
        anyhow::bail!("scene node '{node}' has no mesh material");
    }

    fn set_offline(&mut self, offline: bool) -> anyhow::Result<()> {
        if self.offline != offline {
            self.offline = offline;
            self.rebuild_scene_graph()?;
        }
        Ok(())
    }
}

async fn load_scene_assets(
    payload_assets: &[AssetPayload],
    assets: &scena::Assets,
) -> anyhow::Result<BTreeMap<String, scena::SceneAsset>> {
    let mut loaded = BTreeMap::new();
    for asset in payload_assets {
        let id = asset.id.trim();
        if id.is_empty() {
            anyhow::bail!("scene asset id must not be empty");
        }
        if loaded.contains_key(id) {
            anyhow::bail!("duplicate scene asset id '{id}'");
        }
        if asset
            .kind
            .as_deref()
            .is_some_and(|kind| !kind.eq_ignore_ascii_case("gltf"))
        {
            anyhow::bail!("scene asset '{id}' must use kind = \"gltf\"");
        }
        let uri = asset.uri.as_deref().unwrap_or(id).trim();
        if uri.is_empty() {
            anyhow::bail!("scene asset '{id}' uri must not be empty");
        }
        let report = assets
            .load_scene_with_report_options(
                uri,
                scena::AssetLoadOptions::new().with_strict_textures(true),
            )
            .await?;
        if !report.warnings().is_empty() {
            anyhow::bail!(
                "scene asset '{id}' loaded with warnings under strict texture policy: {:?}",
                report.warnings()
            );
        }
        let scene_asset = report.into_asset();
        loaded.insert(id.to_string(), scene_asset);
    }
    Ok(loaded)
}

fn instantiate_asset_under(
    scene: &mut scena::Scene,
    scene_asset: &scena::SceneAsset,
    parent: scena::NodeKey,
    material_override: Option<scena::MaterialHandle>,
    mesh_keys: &mut Vec<scena::NodeKey>,
) -> anyhow::Result<()> {
    let mut child_indices = BTreeSet::new();
    for node in scene_asset.nodes() {
        child_indices.extend(node.children().iter().copied());
    }
    for source_index in
        (0..scene_asset.nodes().len()).filter(|index| !child_indices.contains(index))
    {
        instantiate_asset_node(
            scene,
            scene_asset,
            source_index,
            parent,
            material_override,
            mesh_keys,
        )?;
    }
    Ok(())
}

fn instantiate_asset_node(
    scene: &mut scena::Scene,
    scene_asset: &scena::SceneAsset,
    source_index: usize,
    parent: scena::NodeKey,
    material_override: Option<scena::MaterialHandle>,
    mesh_keys: &mut Vec<scena::NodeKey>,
) -> anyhow::Result<scena::NodeKey> {
    let source = scene_asset.nodes().get(source_index).ok_or_else(|| {
        anyhow::anyhow!("scene asset references missing node index {source_index}")
    })?;
    let key = match source.meshes() {
        [] => scene.add_empty(parent, source.transform())?,
        [mesh] => {
            let material = material_override.unwrap_or_else(|| mesh.material());
            let key = scene
                .mesh(mesh.geometry(), material)
                .parent(parent)
                .transform(source.transform())
                .add()?;
            mesh_keys.push(key);
            key
        }
        meshes => {
            let key = scene.add_empty(parent, source.transform())?;
            for mesh in meshes {
                let material = material_override.unwrap_or_else(|| mesh.material());
                let mesh_key = scene
                    .mesh(mesh.geometry(), material)
                    .parent(key)
                    .transform(scena::Transform::IDENTITY)
                    .add()?;
                mesh_keys.push(mesh_key);
            }
            key
        }
    };
    for child in source.children() {
        instantiate_asset_node(
            scene,
            scene_asset,
            *child,
            key,
            material_override,
            mesh_keys,
        )?;
    }
    Ok(key)
}

fn node_state(node: &NodePayload) -> NodeState {
    let transform = node.transform.clone().unwrap_or_default();
    let material = node.material.clone().unwrap_or_default();
    NodeState {
        parent: node.parent.as_deref().and_then(normalize_parent),
        position: node
            .local_position
            .or(transform.position)
            .map_or([0.0, 0.0, 0.0], vec3_to_f32),
        rotation: transform.rotation.map_or([0.0, 0.0, 0.0], vec3_to_f32),
        scale: transform.scale.map_or([1.0, 1.0, 1.0], vec3_to_f32),
        asset_rotation: node.asset_rotation.map_or([0.0, 0.0, 0.0], vec3_to_f32),
        pivot: node.pivot.map_or([0.0, 0.0, 0.0], vec3_to_f32),
        visible: true,
        material: MaterialState {
            base_color: material.base_color.unwrap_or_else(|| "#3b82f6".to_string()),
            emissive: material.emissive.unwrap_or_else(|| "#000000".to_string()),
            opacity: f32_from_f64(material.opacity.unwrap_or(1.0).clamp(0.0, 1.0)),
        },
    }
}

fn normalize_parent(parent: &str) -> Option<String> {
    let parent = parent.trim();
    if parent.is_empty() {
        None
    } else {
        Some(parent.to_string())
    }
}

fn node_group_transform(state: &NodeState) -> scena::Transform {
    scena::Transform {
        translation: scena::Vec3::new(state.position[0], state.position[1], state.position[2]),
        rotation: scena::Quat::from_rotation_x(state.rotation[0])
            * scena::Quat::from_rotation_y(state.rotation[1])
            * scena::Quat::from_rotation_z(state.rotation[2]),
        scale: scena::Vec3::new(1.0, 1.0, 1.0),
    }
}

fn node_mesh_transform(state: &NodeState) -> scena::Transform {
    scena::Transform {
        translation: scena::Vec3::new(-state.pivot[0], -state.pivot[1], -state.pivot[2]),
        rotation: scena::Quat::from_rotation_x(state.asset_rotation[0])
            * scena::Quat::from_rotation_y(state.asset_rotation[1])
            * scena::Quat::from_rotation_z(state.asset_rotation[2]),
        scale: scena::Vec3::new(state.scale[0], state.scale[1], state.scale[2]),
    }
}

fn geometry_for_primitive(value: &str) -> anyhow::Result<scena::GeometryDesc> {
    match value.trim().to_ascii_lowercase().as_str() {
        "box" | "cube" | "motor" | "shaft" => Ok(scena::GeometryDesc::box_xyz(1.0, 1.0, 1.0)),
        "plate" => Ok(scena::GeometryDesc::box_xyz(1.4, 0.12, 0.9)),
        "grid" | "floor" => Ok(scena::GeometryDesc::grid(2.0, 8)),
        "depth-sentinel-line" => Ok(scena::GeometryDesc::line(
            scena::Vec3::new(0.0, 0.0, 0.0),
            scena::Vec3::new(0.001, 0.0, 0.0),
        )),
        other => anyhow::bail!("unsupported primitive '{other}'"),
    }
}

fn material_desc_for_primitive(
    primitive: &str,
    state: &MaterialState,
    offline: bool,
    solid_textures: &BTreeMap<String, scena::TextureHandle>,
) -> anyhow::Result<scena::MaterialDesc> {
    if primitive.trim().eq_ignore_ascii_case("depth-sentinel-line") {
        return Ok(scena::MaterialDesc::line(
            parse_color(if offline {
                "#7c8491"
            } else {
                &state.base_color
            })?,
            1.0,
        ));
    }
    let texture = material_texture_handle(state, offline, solid_textures);
    material_desc(state, offline, texture)
}

fn material_desc(
    state: &MaterialState,
    offline: bool,
    base_color_texture: Option<scena::TextureHandle>,
) -> anyhow::Result<scena::MaterialDesc> {
    let mut base = parse_color(if offline {
        "#7c8491"
    } else {
        &state.base_color
    })?;
    base.a = if offline {
        0.68
    } else {
        state.opacity.clamp(0.0, 1.0)
    };
    let emissive = if offline { "#000000" } else { &state.emissive };
    let material = if let Some(texture) = base_color_texture {
        scena::MaterialDesc::plastic(scena::Color::WHITE).with_base_color_texture(texture)
    } else if offline {
        scena::MaterialDesc::matte(base)
    } else {
        scena::MaterialDesc::plastic(base)
    };
    Ok(material
        .with_emissive(parse_color(emissive)?)
        .with_double_sided(true)
        .with_alpha_mode(if base.a < 1.0 {
            scena::AlphaMode::Blend
        } else {
            scena::AlphaMode::Opaque
        }))
}

fn material_texture_handle(
    state: &MaterialState,
    offline: bool,
    solid_textures: &BTreeMap<String, scena::TextureHandle>,
) -> Option<scena::TextureHandle> {
    let color = if offline {
        "#7c8491"
    } else {
        &state.base_color
    };
    solid_textures.get(&normalize_color_key(color)).copied()
}

fn normalize_color_key(color: &str) -> String {
    color.trim().to_ascii_lowercase()
}

fn number_value(binding: &BindingPayload, value: &serde_json::Value) -> anyhow::Result<f32> {
    let raw = mapped_text(binding, value).map_or_else(
        || json_to_f64(value),
        |mapped| {
            mapped
                .parse::<f64>()
                .map_err(|err| anyhow::anyhow!("mapped value '{mapped}' is not numeric: {err}"))
        },
    )?;
    let scaled = if let Some(scale) = binding.scale {
        let ratio = ((raw - scale.min) / (scale.max - scale.min)).clamp(0.0, 1.0);
        scale.output_min + ratio * (scale.output_max - scale.output_min)
    } else {
        raw
    };
    Ok(f32_from_f64(scaled))
}

fn bool_value(binding: &BindingPayload, value: &serde_json::Value) -> anyhow::Result<bool> {
    if let Some(mapped) = mapped_text(binding, value) {
        return Ok(matches!(
            mapped.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "on" | "yes"
        ));
    }
    Ok(value.as_bool().unwrap_or(json_to_f64(value)? != 0.0))
}

fn text_value(binding: &BindingPayload, value: &serde_json::Value) -> anyhow::Result<String> {
    if let Some(mapped) = mapped_text(binding, value) {
        return Ok(mapped.to_string());
    }
    match value {
        serde_json::Value::String(value) => Ok(value.clone()),
        serde_json::Value::Bool(value) => Ok(value.to_string()),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        other => anyhow::bail!("cannot convert value {other:?} to text"),
    }
}

fn vec3_value(binding: &BindingPayload, value: &serde_json::Value) -> anyhow::Result<[f32; 3]> {
    if let Some(mapped) = mapped_text(binding, value) {
        let parts = mapped
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.len() != 3 {
            anyhow::bail!("mapped vector must have 3 elements");
        }
        return Ok([parts[0].parse()?, parts[1].parse()?, parts[2].parse()?]);
    }
    let values = value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("expected vector array"))?;
    if values.len() != 3 {
        anyhow::bail!("expected vector array with 3 elements");
    }
    Ok([
        f32_from_f64(json_to_f64(&values[0])?),
        f32_from_f64(json_to_f64(&values[1])?),
        f32_from_f64(json_to_f64(&values[2])?),
    ])
}

fn scale_value(binding: &BindingPayload, value: &serde_json::Value) -> anyhow::Result<[f32; 3]> {
    vec3_value(binding, value).or_else(|_| {
        let value = number_value(binding, value)?;
        Ok([value, value, value])
    })
}

fn mapped_text<'a>(binding: &'a BindingPayload, value: &serde_json::Value) -> Option<&'a str> {
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

fn map_key(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Number(value) => value.to_string(),
        other => other.to_string(),
    }
}

fn json_to_f64(value: &serde_json::Value) -> anyhow::Result<f64> {
    value
        .as_f64()
        .ok_or_else(|| anyhow::anyhow!("expected numeric JSON value, got {value:?}"))
}

fn parse_color(value: &str) -> anyhow::Result<scena::Color> {
    scena::Color::from_hex_srgb(value.trim())
        .map_err(|err| anyhow::anyhow!("invalid color '{}': {err}", value.trim()))
}

fn vec3(value: [f64; 3]) -> scena::Vec3 {
    let value = vec3_to_f32(value);
    scena::Vec3::new(value[0], value[1], value[2])
}

fn vec3_to_f32(value: [f64; 3]) -> [f32; 3] {
    [
        f32_from_f64(value[0]),
        f32_from_f64(value[1]),
        f32_from_f64(value[2]),
    ]
}

#[allow(clippy::cast_possible_truncation)]
fn f32_from_f64(value: f64) -> f32 {
    value as f32
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use wasm_bindgen::prelude::*;
    use web_sys::HtmlCanvasElement;

    #[wasm_bindgen]
    pub struct RendererHandle {
        scene: Option<RendererScene>,
        renderer: scena::Renderer,
        origin: String,
        assets: scena::Assets,
        solid_textures: BTreeMap<String, scena::TextureHandle>,
    }

    #[wasm_bindgen]
    pub async fn init(canvas: HtmlCanvasElement) -> Result<RendererHandle, JsValue> {
        console_error_panic_hook::set_once();
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let surface = scena::PlatformSurface::browser_webgl2_canvas_element(canvas, width, height);
        let renderer = scena::Renderer::from_surface_async(surface)
            .await
            .map_err(js_error)?;
        let origin = renderer_origin_from_capabilities(&renderer);
        let assets = scena::Assets::new();
        let solid_textures = load_solid_textures(&assets).await?;
        Ok(RendererHandle {
            scene: None,
            renderer,
            origin,
            assets,
            solid_textures,
        })
    }

    #[wasm_bindgen]
    pub async fn apply_scene(
        handle: &mut RendererHandle,
        scene_payload_json: &str,
    ) -> Result<(), JsValue> {
        handle.scene = Some(
            RendererScene::from_json_with_assets(
                scene_payload_json,
                handle.assets.clone(),
                handle.solid_textures.clone(),
            )
            .await
            .map_err(js_error)?,
        );
        Ok(())
    }

    #[wasm_bindgen]
    pub fn apply_values(handle: &mut RendererHandle, values_json: &str) -> Result<(), JsValue> {
        if let Some(scene) = handle.scene.as_mut() {
            scene.apply_values_json(values_json).map_err(js_error)?;
        }
        Ok(())
    }

    #[wasm_bindgen]
    pub fn render_frame(handle: &mut RendererHandle) -> Result<(), JsValue> {
        let Some(scene) = handle.scene.as_mut() else {
            return Ok(());
        };
        handle
            .renderer
            .set_background(scena::Background::DarkStudio);
        handle
            .renderer
            .set_auto_exposure(scena::AutoExposureConfig::product_studio());
        handle
            .renderer
            .prepare_with_assets(&mut scene.scene, &scene.assets)
            .map_err(js_error)?;
        let stats = handle.renderer.stats();
        if stats.material_textures_missing_decoded_pixels > 0 {
            return Err(js_error(format!(
                "scena prepared {} material texture bindings without decoded pixels",
                stats.material_textures_missing_decoded_pixels
            )));
        }
        handle
            .renderer
            .render_active(&scene.scene)
            .map_err(js_error)?;
        Ok(())
    }

    #[wasm_bindgen]
    pub fn set_offline(handle: &mut RendererHandle, offline: bool) -> Result<(), JsValue> {
        if let Some(scene) = handle.scene.as_mut() {
            scene.set_offline(offline).map_err(js_error)?;
        }
        Ok(())
    }

    #[wasm_bindgen]
    pub fn dispose(handle: &mut RendererHandle) {
        handle.scene = None;
    }

    #[wasm_bindgen]
    pub fn renderer_origin(handle: &RendererHandle) -> String {
        handle.origin.clone()
    }

    async fn load_solid_textures(
        assets: &scena::Assets,
    ) -> Result<BTreeMap<String, scena::TextureHandle>, JsValue> {
        let mut textures = BTreeMap::new();
        for (color, uri) in SOLID_TEXTURE_DATA_URIS {
            let handle = assets
                .load_texture(*uri, scena::TextureColorSpace::Srgb)
                .await
                .map_err(js_error)?;
            textures.insert((*color).to_string(), handle);
        }
        Ok(textures)
    }

    const SOLID_TEXTURE_DATA_URIS: &[(&str, &str)] = &[
        (
            "#000000",
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGD4DwABBAEAX+XDSwAAAABJRU5ErkJggg==",
        ),
        (
            "#22c55e",
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNQOhr3HwAElwJF/XRl9gAAAABJRU5ErkJggg==",
        ),
        (
            "#2563eb",
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNQTX79HwAElwJzVt2vfAAAAABJRU5ErkJggg==",
        ),
        (
            "#38bdf8",
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGOw2PvjPwAGCwLtgxz7OwAAAABJRU5ErkJggg==",
        ),
        (
            "#475569",
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNwD838DwAD8QIFfNcEZAAAAABJRU5ErkJggg==",
        ),
        (
            "#64748b",
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNIKen+DwAFBgJjLYq+4AAAAABJRU5ErkJggg==",
        ),
        (
            "#7c8491",
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGOoaZn4HwAFogKRhW5wKwAAAABJRU5ErkJggg==",
        ),
        (
            "#dbe3ea",
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGO4/fjVfwAI7QOoQNS+uwAAAABJRU5ErkJggg==",
        ),
        (
            "#ef4444",
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGN47+LyHwAGFAJ3AHU+LgAAAABJRU5ErkJggg==",
        ),
        (
            "#f59e0b",
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4Oo/7PwAGyAKePH9QXgAAAABJRU5ErkJggg==",
        ),
        (
            "#f97316",
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4WSz2HwAGbQKC0JqqYgAAAABJRU5ErkJggg==",
        ),
    ];

    fn renderer_origin_from_capabilities(renderer: &scena::Renderer) -> String {
        let backend = format!("{:?}", renderer.capability_report().capabilities().backend);
        if backend.contains("WebGpu") {
            "scena_webgpu".to_string()
        } else {
            "scena_webgl".to_string()
        }
    }

    fn js_error(error: impl std::fmt::Display) -> JsValue {
        JsValue::from_str(&error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_version_matches_parented_view_contract() {
        assert_eq!(trust_twin_renderer_contract_version(), 2);
    }

    #[test]
    fn renderer_scene_builds_parented_scena_graph_and_reparents_box() {
        let payload = r#"
{
  "node": [
    {
      "id": "PICKUP-1",
      "primitive": "box",
      "parent": "",
      "local_position": [0.0, 0.0, 0.0],
      "pivot": [0.0, 0.0, 0.0],
      "transform": { "scale": [1.4, 0.3, 1.0] }
    },
    {
      "id": "GRIPPER-1",
      "primitive": "box",
      "parent": "",
      "local_position": [1.0, 1.0, 0.0],
      "pivot": [0.0, 0.0, 0.0],
      "transform": { "scale": [0.2, 0.2, 0.2] }
    },
    {
      "id": "BOX-1",
      "primitive": "box",
      "parent": "PICKUP-1",
      "local_position": [0.0, 0.35, 0.0],
      "pivot": [0.0, 0.0, 0.0],
      "transform": { "scale": [0.4, 0.4, 0.4] },
      "parent_pose": [
        { "parent": "PICKUP-1", "local_position": [0.0, 0.35, 0.0] },
        { "parent": "GRIPPER-1", "local_position": [0.0, -0.28, 0.0] }
      ]
    }
  ],
  "bind3d": [
    {
      "node": "BOX-1",
      "property": "parent",
      "source": "Main.RobotBoxParentState",
      "map": { "0": "PICKUP-1", "1": "GRIPPER-1" }
    }
  ]
}
"#;
        let mut scene = RendererScene::from_json(payload).expect("scene builds");
        let box_key = scene.nodes["BOX-1"].key.expect("box key");
        let initial = scene
            .scene
            .world_transform(box_key)
            .expect("box world transform")
            .translation;
        assert!((initial.y - 0.35).abs() < 0.000_01);

        scene
            .apply_values_json(r#"{ "Main.RobotBoxParentState": 1 }"#)
            .expect("parent binding applies");
        assert_eq!(
            scene.nodes["BOX-1"].state.parent.as_deref(),
            Some("GRIPPER-1")
        );
        let box_position = scene.nodes["BOX-1"].state.position;
        assert!((box_position[0] - 0.0).abs() < 0.000_01);
        assert!((box_position[1] - -0.28).abs() < 0.000_01);
        assert!((box_position[2] - 0.0).abs() < 0.000_01);
        assert_eq!(scene.nodes["BOX-1"].key, Some(box_key));
    }
}

use super::*;

#[derive(Debug)]
pub(super) struct UrdfArmModel {
    pub(super) chain: k::Chain<f32>,
    pub(super) link_specs: BTreeMap<&'static str, LinkSpec>,
    pub(super) joints: [UrdfRevoluteJoint; 2],
    pub(super) urdf_trace: WorldUrdfTrace,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LinkSpec {
    pub(super) half_extents: [f32; 3],
    pub(super) collision_origin: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
pub(super) struct UrdfRevoluteJoint {
    pub(super) name: &'static str,
    pub(super) lower: f32,
    pub(super) upper: f32,
}

impl UrdfArmModel {
    pub(super) fn load(
        fixture_path: &'static str,
        absolute_path: &Path,
        allow_missing_limits: bool,
    ) -> anyhow::Result<Self> {
        let robot = urdf_rs::read_file(absolute_path)
            .with_context(|| format!("failed to parse URDF {}", absolute_path.display()))?;
        let chain = k::Chain::<f32>::from_urdf_file(absolute_path)
            .with_context(|| format!("failed to build k chain from {}", absolute_path.display()))?;
        let link_specs = load_link_specs(&robot)?;
        let joints = load_revolute_joints(&robot, allow_missing_limits)?;
        let urdf_trace = WorldUrdfTrace {
            fixture_path: fixture_path.to_string(),
            instances: Vec::new(),
            links_loaded: vec![
                "base".to_string(),
                "link_1".to_string(),
                "link_2".to_string(),
                "tool".to_string(),
            ],
            joints_loaded: vec![
                WorldUrdfJointTrace {
                    name: "base_to_link_1".to_string(),
                    joint_type: "revolute".to_string(),
                    axis: Some([0.0, 0.0, 1.0]),
                    limit_lower: Some(joints[0].lower),
                    limit_upper: Some(joints[0].upper),
                    offset: None,
                },
                WorldUrdfJointTrace {
                    name: "link_1_to_link_2".to_string(),
                    joint_type: "revolute".to_string(),
                    axis: Some([0.0, 0.0, 1.0]),
                    limit_lower: Some(joints[1].lower),
                    limit_upper: Some(joints[1].upper),
                    offset: None,
                },
                WorldUrdfJointTrace {
                    name: "link_2_to_tool".to_string(),
                    joint_type: "fixed".to_string(),
                    axis: None,
                    limit_lower: None,
                    limit_upper: None,
                    offset: Some([0.65, 0.0, 0.0]),
                },
            ],
            parsed_once: true,
            consulted_in_tick_loop: false,
        };
        Ok(Self {
            chain,
            link_specs,
            joints,
            urdf_trace,
        })
    }

    pub(super) fn link_spec(&self, name: &str) -> anyhow::Result<LinkSpec> {
        self.link_specs
            .get(name)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("missing URDF link spec for {name}"))
    }
}

pub(super) fn load_link_specs(
    robot: &urdf_rs::Robot,
) -> anyhow::Result<BTreeMap<&'static str, LinkSpec>> {
    let mut specs = BTreeMap::new();
    for name in ["base", "link_1", "link_2", "tool"] {
        let link = robot
            .links
            .iter()
            .find(|link| link.name == name)
            .ok_or_else(|| anyhow::anyhow!("URDF missing link {name}"))?;
        let collision = link
            .collision
            .first()
            .ok_or_else(|| anyhow::anyhow!("URDF link {name} missing collision"))?;
        let urdf_rs::Geometry::Box { size } = &collision.geometry else {
            anyhow::bail!("URDF link {name} must use box collision");
        };
        let half_extents = [
            (size[0] as f32) / 2.0,
            (size[1] as f32) / 2.0,
            (size[2] as f32) / 2.0,
        ];
        let collision_origin = [
            collision.origin.xyz[0] as f32,
            collision.origin.xyz[1] as f32,
            collision.origin.xyz[2] as f32,
        ];
        specs.insert(
            stable_link_name(name),
            LinkSpec {
                half_extents,
                collision_origin,
            },
        );
    }
    Ok(specs)
}

pub(super) fn load_revolute_joints(
    robot: &urdf_rs::Robot,
    allow_missing_limits: bool,
) -> anyhow::Result<[UrdfRevoluteJoint; 2]> {
    let mut joints = Vec::new();
    for name in ["base_to_link_1", "link_1_to_link_2"] {
        let joint = robot
            .joints
            .iter()
            .find(|joint| joint.name == name)
            .ok_or_else(|| anyhow::anyhow!("URDF missing joint {name}"))?;
        if joint.joint_type != urdf_rs::JointType::Revolute {
            anyhow::bail!("URDF joint {name} must be revolute");
        }
        let (lower, upper) =
            if allow_missing_limits && joint.limit.lower == 0.0 && joint.limit.upper == 0.0 {
                (-PI, PI)
            } else {
                (joint.limit.lower as f32, joint.limit.upper as f32)
            };
        joints.push(UrdfRevoluteJoint {
            name: stable_joint_name(name),
            lower,
            upper,
        });
    }
    let [first, second]: [UrdfRevoluteJoint; 2] = joints
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected exactly two revolute joints"))?;
    Ok([first, second])
}

pub(super) fn validate_revolute_limits_in_xml(
    path: &Path,
    allow_missing_limits: bool,
) -> anyhow::Result<()> {
    if allow_missing_limits {
        return Ok(());
    }
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read URDF XML {}", path.display()))?;
    let doc = roxmltree::Document::parse(&source)
        .with_context(|| format!("failed to parse URDF XML {}", path.display()))?;
    for joint in doc
        .descendants()
        .filter(|node| node.has_tag_name("joint") && node.attribute("type") == Some("revolute"))
    {
        let name = joint.attribute("name").unwrap_or("<unnamed>");
        let has_limit = joint.children().any(|node| node.has_tag_name("limit"));
        if !has_limit {
            anyhow::bail!("URDF revolute joint {name} is missing a <limit> block");
        }
    }
    Ok(())
}

pub(super) fn stable_link_name(name: &str) -> &'static str {
    match name {
        "base" => "base",
        "link_1" => "link_1",
        "link_2" => "link_2",
        "tool" => "tool",
        _ => unreachable!("unexpected P3 link name"),
    }
}

pub(super) fn stable_joint_name(name: &str) -> &'static str {
    match name {
        "base_to_link_1" => "base_to_link_1",
        "link_1_to_link_2" => "link_1_to_link_2",
        _ => unreachable!("unexpected P3 joint name"),
    }
}

pub(super) fn resolve_repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate has workspace parent")
        .parent()
        .expect("workspace root exists")
        .join(relative)
}

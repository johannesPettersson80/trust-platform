use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const ROBOT_FB_HASH_DOMAIN: &str = "trust-twin-robot-fb:v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GeneratedRobotFunctionBlock {
    pub function_block_name: String,
    pub source_urdf: String,
    pub manifest_hash: String,
    pub source_hash: String,
    pub source: String,
}

#[derive(Debug, Error)]
pub enum RobotFunctionBlockGenerateError {
    #[error("invalid robot manifest TOML: {0}")]
    ManifestToml(#[from] toml::de::Error),
    #[error("robot manifest must set robot.model")]
    MissingModel,
    #[error("robot manifest must set robot.function_block")]
    MissingFunctionBlock,
    #[error("robot manifest must set robot.urdf")]
    MissingUrdf,
    #[error("robot manifest must set robot.native_bridge")]
    MissingNativeBridge,
    #[error("robot manifest function_block must start with Robot_")]
    InvalidFunctionBlockName,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RobotFbManifest {
    robot: RobotFbManifestRobot,
    #[serde(default)]
    joint: Vec<RobotFbManifestJoint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RobotFbManifestRobot {
    model: String,
    function_block: String,
    urdf: String,
    native_bridge: String,
    base_frame: String,
    tool_frame: String,
    grip_frame: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RobotFbManifestJoint {
    name: String,
    output: String,
}

pub fn generate_robot_function_block_from_manifest_toml(
    manifest_toml: &str,
) -> Result<GeneratedRobotFunctionBlock, RobotFunctionBlockGenerateError> {
    let manifest: RobotFbManifest = toml::from_str(manifest_toml)?;
    generate_robot_function_block(&manifest, manifest_toml)
}

fn generate_robot_function_block(
    manifest: &RobotFbManifest,
    manifest_toml: &str,
) -> Result<GeneratedRobotFunctionBlock, RobotFunctionBlockGenerateError> {
    validate_manifest(manifest)?;

    let manifest_hash = hash_text(ROBOT_FB_HASH_DOMAIN, manifest_toml);
    let source = render_robot_fb_source(manifest, &manifest_hash);
    let source_hash = hash_text(ROBOT_FB_HASH_DOMAIN, &source);

    Ok(GeneratedRobotFunctionBlock {
        function_block_name: manifest.robot.function_block.clone(),
        source_urdf: manifest.robot.urdf.clone(),
        manifest_hash,
        source_hash,
        source,
    })
}

fn validate_manifest(manifest: &RobotFbManifest) -> Result<(), RobotFunctionBlockGenerateError> {
    if manifest.robot.model.trim().is_empty() {
        return Err(RobotFunctionBlockGenerateError::MissingModel);
    }
    if manifest.robot.function_block.trim().is_empty() {
        return Err(RobotFunctionBlockGenerateError::MissingFunctionBlock);
    }
    if !manifest.robot.function_block.starts_with("Robot_") {
        return Err(RobotFunctionBlockGenerateError::InvalidFunctionBlockName);
    }
    if manifest.robot.urdf.trim().is_empty() {
        return Err(RobotFunctionBlockGenerateError::MissingUrdf);
    }
    if manifest.robot.native_bridge.trim().is_empty() {
        return Err(RobotFunctionBlockGenerateError::MissingNativeBridge);
    }
    Ok(())
}

fn render_robot_fb_source(manifest: &RobotFbManifest, manifest_hash: &str) -> String {
    let mut source = String::new();
    source.push_str("(* trust-twin-generated: robot function block v1 *)\n");
    source.push_str(&format!("(* model: {} *)\n", manifest.robot.model));
    source.push_str(&format!("(* manifest_hash: sha256:{manifest_hash} *)\n"));
    source.push_str(&format!("(* urdf: {} *)\n", manifest.robot.urdf));
    source.push_str(&format!(
        "(* base_frame: {}, tool_frame: {}, grip_frame: {} *)\n",
        manifest.robot.base_frame, manifest.robot.tool_frame, manifest.robot.grip_frame
    ));
    if !manifest.joint.is_empty() {
        let joints = manifest
            .joint
            .iter()
            .map(|joint| format!("{}->{}", joint.name, joint.output))
            .collect::<Vec<_>>()
            .join(", ");
        source.push_str(&format!("(* joints: {joints} *)\n"));
    }
    source.push_str(&format!(
        "(* native_bridge: {} *)\n",
        manifest.robot.native_bridge
    ));
    source.push_str("(* FK and URDF-derived poses are verified in trust_runtime::world::arm; this FB does not write transforms. *)\n\n");
    source.push_str(&format!(
        "FUNCTION_BLOCK {}\n",
        manifest.robot.function_block
    ));
    source.push_str("VAR_INPUT\n");
    source.push_str("    Enable : BOOL;\n");
    source.push_str("    Command : INT;\n");
    source.push_str("END_VAR\n");
    source.push_str("VAR_OUTPUT\n");
    source.push_str("    EnabledOut : BOOL;\n");
    source.push_str("    Busy : BOOL;\n");
    source.push_str("    Done : BOOL;\n");
    source.push_str("    Fault : BOOL;\n");
    source.push_str("    State : INT;\n");
    source.push_str("    Owner : INT;\n");
    source.push_str("    HasWorkpiece : BOOL;\n");
    source.push_str("    GripperOpen : BOOL;\n");
    source.push_str("    StatusLight : BOOL;\n");
    for joint in &manifest.joint {
        source.push_str(&format!("    {} : REAL;\n", joint.output));
    }
    source.push_str("    ToolYaw : REAL;\n");
    source.push_str("    ToolX : REAL;\n");
    source.push_str("    ToolY : REAL;\n");
    source.push_str("    ToolZ : REAL;\n");
    source.push_str("    WorkpieceX : REAL;\n");
    source.push_str("    WorkpieceY : REAL;\n");
    source.push_str("    WorkpieceZ : REAL;\n");
    source.push_str("END_VAR\n");
    source.push_str("END_FUNCTION_BLOCK\n");
    source
}

fn hash_text(domain: &str, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update(b"\n");
    hasher.update(text.replace("\r\n", "\n").as_bytes());
    format!("{:x}", hasher.finalize())
}

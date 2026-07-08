use super::*;

/// P3 scenario variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldUrdfArmScenario {
    /// Canonical URDF-arm pick/place proof.
    Canonical,
    /// Test-only permissive run for missing joint-limit URDFs.
    MissingLimitsPermissive,
    /// Test-only run that perturbs one link to prove FK drift is detected.
    FkDrift,
}

/// Configuration for the P3 URDF-arm proof.
#[derive(Debug, Clone, Copy)]
pub struct WorldUrdfArmSmokeConfig {
    /// Fixed tick delta in seconds.
    pub tick_dt_seconds: f32,
    /// Number of fixed ticks to run.
    pub tick_count: u32,
    /// Whether to register the static floor collider.
    pub include_floor: bool,
    /// Relative fixture URDF path.
    pub fixture_path: &'static str,
    /// Scenario variant.
    pub scenario: WorldUrdfArmScenario,
}

impl Default for WorldUrdfArmSmokeConfig {
    fn default() -> Self {
        Self {
            tick_dt_seconds: ARM_TICK_DT_SECONDS,
            tick_count: ARM_TICK_COUNT,
            include_floor: true,
            fixture_path: P3_MINIMAL_ARM_URDF,
            scenario: WorldUrdfArmScenario::Canonical,
        }
    }
}

/// P4 multi-URDF scenario variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldMultiUrdfArmScenario {
    /// Canonical two-arm handoff proof.
    CanonicalHandoff,
    /// Two arms contend for an unowned workpiece without a handoff plan.
    SimultaneousGripNoHandoff,
    /// Receiver attempts to grip while the offerer still owns the workpiece.
    SecondGripWhileOwned,
    /// Receiver-arm FK drift after handoff.
    FkDriftReceiver,
}

/// Configuration for the P4 multi-URDF proof.
#[derive(Debug, Clone, Copy)]
pub struct WorldMultiUrdfArmSmokeConfig {
    /// Fixed tick delta in seconds.
    pub tick_dt_seconds: f32,
    /// Number of fixed ticks to run.
    pub tick_count: u32,
    /// Whether to register the static floor collider.
    pub include_floor: bool,
    /// Whether actuator registration is reversed before id-ordered stepping.
    pub reverse_arm_registration: bool,
    /// Scenario variant.
    pub scenario: WorldMultiUrdfArmScenario,
}

impl Default for WorldMultiUrdfArmSmokeConfig {
    fn default() -> Self {
        Self {
            tick_dt_seconds: ARM_TICK_DT_SECONDS,
            tick_count: ARM_TICK_COUNT,
            include_floor: true,
            reverse_arm_registration: false,
            scenario: WorldMultiUrdfArmScenario::CanonicalHandoff,
        }
    }
}

/// P3 URDF load trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldUrdfTrace {
    /// Relative fixture path loaded during setup.
    pub fixture_path: String,
    /// URDF arm instances loaded from this fixture.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<WorldUrdfArmInstanceTrace>,
    /// URDF links loaded from the fixture.
    pub links_loaded: Vec<String>,
    /// URDF joints loaded from the fixture.
    pub joints_loaded: Vec<WorldUrdfJointTrace>,
    /// Whether parsing happened once during setup.
    pub parsed_once: bool,
    /// Whether the tick loop consulted the URDF text.
    pub consulted_in_tick_loop: bool,
}

/// One URDF arm instance loaded into the shared world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldUrdfArmInstanceTrace {
    /// Stable arm id.
    pub id: String,
    /// Base position used to instantiate the arm.
    pub base_position: [f32; 3],
    /// Whether this instance parsed the fixture once during setup.
    pub parsed_once: bool,
    /// Whether the tick loop consulted the URDF text for this instance.
    pub consulted_in_tick_loop: bool,
}

/// One URDF joint loaded into the P3 proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldUrdfJointTrace {
    /// Joint name.
    pub name: String,
    /// Joint type.
    pub joint_type: String,
    /// Joint axis, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<[f32; 3]>,
    /// Lower joint limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_lower: Option<f32>,
    /// Upper joint limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_upper: Option<f32>,
    /// Fixed-joint offset, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<[f32; 3]>,
}

/// P3 FK verifier trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldFkVerifierTrace {
    /// Maximum FK/Rapier distance in meters.
    pub max_consistency_distance_m: f32,
    /// Per-arm FK verifier results for multi-URDF proofs.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub per_arm: BTreeMap<String, WorldFkArmVerifierTrace>,
    /// Dynamic links checked by FK.
    pub checked_links: Vec<String>,
    /// Number of ticks checked.
    pub checked_ticks: u32,
    /// Consistency tolerance in meters.
    pub consistency_tolerance: f32,
}

/// Per-arm FK verifier trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldFkArmVerifierTrace {
    /// Maximum FK/Rapier distance in meters for this arm.
    pub max_consistency_distance_m: f32,
    /// Dynamic links checked by FK.
    pub checked_links: Vec<String>,
    /// Number of link-tick samples checked.
    pub checked_samples: u32,
}

/// P3 link sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldArmLinkTrace {
    /// URDF link name.
    pub name: String,
    /// Rapier-owned body-center position.
    pub rapier_position: [f32; 3],
    /// FK-predicted body-center position.
    pub fk_predicted_position: [f32; 3],
    /// Euclidean distance between Rapier and FK positions.
    pub fk_consistency_distance: f32,
    /// Link bottom Y for above-floor checks.
    pub bottom_y: f32,
    /// Rapier yaw about the URDF Z axis.
    pub rapier_yaw_z: f32,
}

/// P3 joint sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldArmJointTrace {
    /// URDF joint name.
    pub name: String,
    /// Current joint position read from Rapier state.
    pub position: f32,
    /// URDF lower limit.
    pub limit_lower: f32,
    /// URDF upper limit.
    pub limit_upper: f32,
    /// Whether the joint is at a limit tolerance.
    pub clamped: bool,
}

/// P3 URDF parse assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrdfParsedOnceAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Whether setup parsed the fixture.
    pub parsed_once: bool,
    /// Whether the tick loop consulted the URDF text.
    pub consulted_in_tick_loop: bool,
}

/// P3 arm-rendering handoff assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmRenderedThroughHandoffAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Number of dynamic visible bodies expected per tick.
    pub expected_dynamic_bodies_per_tick: u32,
    /// Number of trace ticks checked.
    pub checked_ticks: u32,
}

/// P3 FK consistency assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FkConsistencyAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Maximum FK/Rapier distance observed.
    pub max_consistency_distance_m: f32,
    /// Consistency tolerance.
    pub tolerance: f32,
    /// Number of link-tick samples checked.
    pub checked_samples: u32,
}

/// P3 joint-limit assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JointLimitAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Number of joint samples outside URDF limits.
    pub out_of_limit_samples: u32,
    /// Limit-clamp events observed.
    pub joint_clamped_events: Vec<String>,
}

/// P3 arm above-floor assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmAboveFloorAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Lowest link bottom Y observed.
    pub min_link_y: f32,
    /// Floor top Y.
    pub floor_y: f32,
    /// Link name for the lowest sample.
    pub min_link_name: String,
}

/// P4 multi-URDF load assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiUrdfArmsLoadedAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Arm ids loaded in the proof.
    pub arm_ids: Vec<String>,
    /// Number of URDF instances loaded.
    pub instance_count: u32,
    /// Whether every instance parsed once and was not consulted in the tick loop.
    pub parsed_once_per_instance: bool,
}

/// P4 per-arm FK consistency assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerArmFkConsistencyAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Consistency tolerance.
    pub tolerance: f32,
    /// Per-arm maximum FK/Rapier distance.
    pub max_consistency_distance_by_arm: BTreeMap<String, f32>,
}

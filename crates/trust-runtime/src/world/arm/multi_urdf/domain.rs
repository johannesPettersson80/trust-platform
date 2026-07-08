use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum P4ArmId {
    ArmA,
    ArmB,
}

impl P4ArmId {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::ArmA => "arm_a",
            Self::ArmB => "arm_b",
        }
    }

    pub(super) fn source(self) -> String {
        format!("{}@{}", ARM_SOURCE, self.name())
    }

    pub(super) fn joint_name(self) -> &'static str {
        match self {
            Self::ArmA => "fixed(arm_a.tool, workpiece_grip)",
            Self::ArmB => "fixed(arm_b.tool, workpiece_grip)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum P4ArmRole {
    Offerer,
    Receiver,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct P4EnvironmentBodies {
    pub(super) floor_collider: Option<ColliderHandle>,
    pub(super) fixture_collider: ColliderHandle,
    pub(super) transfer_collider: ColliderHandle,
    pub(super) workpiece_body: RigidBodyHandle,
    pub(super) workpiece_collider: ColliderHandle,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct P4ArmBodies {
    pub(super) link_1_body: RigidBodyHandle,
    pub(super) link_1_collider: ColliderHandle,
    pub(super) link_1_joint: MultibodyJointHandle,
    pub(super) link_2_body: RigidBodyHandle,
    pub(super) link_2_collider: ColliderHandle,
    pub(super) link_2_joint: MultibodyJointHandle,
    pub(super) tool_body: RigidBodyHandle,
    pub(super) tool_collider: ColliderHandle,
}

#[derive(Debug)]
pub(super) struct P4ArmInstance {
    pub(super) id: u8,
    pub(super) arm_id: P4ArmId,
    pub(super) role: P4ArmRole,
    pub(super) base_world: [f32; 3],
    pub(super) model: UrdfArmModel,
    pub(super) bodies: P4ArmBodies,
    pub(super) state: ActuatorState,
    pub(super) transitions: Vec<WorldActuatorTransitionTrace>,
    pub(super) faults: Vec<WorldOwnershipFaultTrace>,
}

impl P4ArmInstance {
    pub(super) fn new(
        id: u8,
        arm_id: P4ArmId,
        role: P4ArmRole,
        base_world: [f32; 3],
        model: UrdfArmModel,
        bodies: P4ArmBodies,
    ) -> Self {
        Self {
            id,
            arm_id,
            role,
            base_world,
            model,
            bodies,
            state: ActuatorState::Idle,
            transitions: Vec::new(),
            faults: Vec::new(),
        }
    }

    pub(super) fn transition(&mut self, tick: u32, to: ActuatorState, trigger: &str) -> String {
        if self.state == to {
            return format!(
                "state_transition({}: {:?} -> {:?})",
                self.arm_id.name(),
                self.state,
                to
            );
        }
        let from = if self.transitions.is_empty() && self.state == ActuatorState::Idle {
            None
        } else {
            Some(self.state)
        };
        let previous = self.state;
        self.transitions.push(WorldActuatorTransitionTrace {
            tick,
            from,
            to,
            trigger: Some(trigger.to_string()),
        });
        self.state = to;
        format!(
            "state_transition({}: {:?} -> {:?})",
            self.arm_id.name(),
            previous,
            to
        )
    }

    pub(super) fn trace(&self) -> WorldActuatorTrace {
        let mut states = vec![ActuatorState::Idle];
        for transition in &self.transitions {
            states.push(transition.to);
        }
        WorldActuatorTrace {
            id: Some(self.id),
            name: Some(self.arm_id.name().to_string()),
            type_name: "WorldActuator".to_string(),
            states_observed: states,
            state_transitions: self.transitions.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct P4Ownership {
    pub(super) owner: Option<P4ArmId>,
    pub(super) active_joint: Option<(P4ArmId, ImpulseJointHandle)>,
    pub(super) transitions: Vec<WorldOwnershipTransitionTrace>,
}

impl P4Ownership {
    pub(super) fn new() -> Self {
        Self {
            owner: None,
            active_joint: None,
            transitions: Vec::new(),
        }
    }

    pub(super) fn sample(&self) -> WorldOwnershipSample {
        WorldOwnershipSample {
            workpiece: "workpiece".to_string(),
            owner: self.owner.map(P4ArmId::name).map(str::to_string),
        }
    }

    pub(super) fn transition(&mut self, tick: u32, to: Option<P4ArmId>, trigger: &str) {
        let from = self.owner;
        self.transitions.push(WorldOwnershipTransitionTrace {
            tick,
            workpiece: "workpiece".to_string(),
            from: from.map(P4ArmId::name).map(str::to_string),
            to: to.map(P4ArmId::name).map(str::to_string),
            trigger: trigger.to_string(),
        });
        self.owner = to;
    }

    pub(super) fn trace(&self) -> WorldOwnershipTrace {
        WorldOwnershipTrace {
            transitions: self.transitions.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct P4HandoffPlan {
    pub(super) registered: bool,
    pub(super) atomic_tick: Option<u32>,
    pub(super) atomic_event_order: Vec<String>,
}

impl P4HandoffPlan {
    pub(super) fn new(registered: bool) -> Self {
        Self {
            registered,
            atomic_tick: None,
            atomic_event_order: p4_expected_handoff_event_order(),
        }
    }

    pub(super) fn trace(&self) -> WorldHandoffPlanTrace {
        WorldHandoffPlanTrace {
            registered_pairs: if self.registered {
                vec![WorldHandoffPairTrace {
                    offerer: "arm_a".to_string(),
                    receiver: "arm_b".to_string(),
                    transfer_zone: "transfer_zone".to_string(),
                }]
            } else {
                Vec::new()
            },
            atomic_tick: self.atomic_tick,
            atomic_event_order: self.atomic_event_order.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct P4SceneNodes {
    pub(super) arm_a_link_1: scena::NodeKey,
    pub(super) arm_a_link_2: scena::NodeKey,
    pub(super) arm_a_tool: scena::NodeKey,
    pub(super) arm_b_link_1: scena::NodeKey,
    pub(super) arm_b_link_2: scena::NodeKey,
    pub(super) arm_b_tool: scena::NodeKey,
    pub(super) workpiece: scena::NodeKey,
}

pub(super) fn p4_nodes(
    arm_a_link_1: scena::NodeKey,
    arm_a_link_2: scena::NodeKey,
    arm_a_tool: scena::NodeKey,
    arm_b_link_1: scena::NodeKey,
    arm_b_link_2: scena::NodeKey,
    arm_b_tool: scena::NodeKey,
    workpiece: scena::NodeKey,
) -> P4SceneNodes {
    P4SceneNodes {
        arm_a_link_1,
        arm_a_link_2,
        arm_a_tool,
        arm_b_link_1,
        arm_b_link_2,
        arm_b_tool,
        workpiece,
    }
}

pub(super) fn p4_initial_q(arm_id: P4ArmId, scenario: WorldMultiUrdfArmScenario) -> [f32; 2] {
    match (arm_id, scenario) {
        (P4ArmId::ArmA, WorldMultiUrdfArmScenario::SimultaneousGripNoHandoff) => {
            P4_ARM_A_CONTESTED_Q
        }
        (P4ArmId::ArmB, WorldMultiUrdfArmScenario::SimultaneousGripNoHandoff) => {
            P4_ARM_B_CONTESTED_Q
        }
        _ => ARM_HOME_Q,
    }
}

pub(super) fn p4_base_world(arm_id: P4ArmId, scenario: WorldMultiUrdfArmScenario) -> [f32; 3] {
    match (arm_id, scenario) {
        (P4ArmId::ArmB, WorldMultiUrdfArmScenario::SimultaneousGripNoHandoff) => {
            P4_ARM_A_BASE_WORLD
        }
        (P4ArmId::ArmA, _) => P4_ARM_A_BASE_WORLD,
        (P4ArmId::ArmB, _) => P4_ARM_B_BASE_WORLD,
    }
}

pub(super) fn p4_arm(arms: &[P4ArmInstance], arm_id: P4ArmId) -> &P4ArmInstance {
    arms.iter()
        .find(|arm| arm.arm_id == arm_id)
        .expect("P4 arm exists")
}

pub(super) fn p4_two_arms_mut(
    arms: &mut [P4ArmInstance],
    first: P4ArmId,
    second: P4ArmId,
) -> (&mut P4ArmInstance, &mut P4ArmInstance) {
    let first_index = arms
        .iter()
        .position(|arm| arm.arm_id == first)
        .expect("first P4 arm exists");
    let second_index = arms
        .iter()
        .position(|arm| arm.arm_id == second)
        .expect("second P4 arm exists");
    assert_ne!(first_index, second_index);
    if first_index < second_index {
        let (left, right) = arms.split_at_mut(second_index);
        (&mut left[first_index], &mut right[0])
    } else {
        let (left, right) = arms.split_at_mut(first_index);
        (&mut right[0], &mut left[second_index])
    }
}

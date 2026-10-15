//! The PID group (specification 35, C7). The decentralized design gives each member a PI by SIMC on its own path, for the gain it sees
//! when the other loops are closed: `Kᵢᵢ/λᵢᵢ`, with `λ` the relative gain array of the members' steady-state gains (N4.5). The static
//! decoupler `D = K⁻¹·diag(K)` makes each member move only its own measurement in steady state, and its members are then designed on the
//! model seen through it. The TypeScript it ports is `decentralizedPidDesign.ts` and `decoupledGroupDesign.ts`.
//!
//! The values that these designs write into the program's variables are mapped by the loop's seed, not here: the designs return the gains
//! and the matrix by member and channel.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::identify::gain::ControllerAction;
use trust_control_math::linalg::{DenseMatrix, NumericalTolerance};
use trust_control_math::model::{StateSpace, Timing};
use trust_control_math::multichannel::{
    pairing_by_relative_gain, relative_gain_array, static_decoupler, steady_state_gain,
};
use trust_control_math::simulate::DelayedSystem;
use trust_control_math::tuning::FirstOrderPlusDelay;

use super::path::{delayed_plant, open_loop_step_response, two_point_path, StepResponse};
use super::texts;
use crate::identification::refusal::Refusal;
use crate::model::{GeneralModel, ModelChannel};
use crate::tuning::formula::tune_pi;
use crate::tuning::objective::{TuningMethod, TuningObjective, AUTOMATIC_OBJECTIVE};
use crate::tuning::plant::TuningPlant;
use crate::tuning::structure::ParallelGains;

/// The most pairs whose pairing the search covers (N4.5): a larger group names no pairing, so it never enumerates the permutations.
pub const MAXIMUM_SEARCHED_PAIRS: usize = 8;

/// The tolerance of the group's gain matrices: the rank and conditioning rules of the numerical annex (N4.5).
const TOLERANCE: NumericalTolerance = NumericalTolerance {
    absolute: 1e-12,
    relative: 1e-10,
    condition_limit: 1e12,
};

/// A group member's pair: its input channel and its output channel, by their identifiers in the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecentralizedPair {
    /// The input channel's identifier.
    pub input_id: String,
    /// The output channel's identifier.
    pub output_id: String,
}

/// One member of a decentralized design (C7): its own path, its relative gain, the effective gain it sees with the other loops closed,
/// and its SIMC PI.
#[derive(Debug, Clone, PartialEq)]
pub struct DecentralizedMember {
    /// The member's pair.
    pub pair: DecentralizedPair,
    /// The steady-state gain of the member's own path, `Kᵢᵢ`.
    pub path_gain: f64,
    /// The relative gain of the pairing, `λᵢᵢ`.
    pub relative_gain: f64,
    /// `Kᵢᵢ/λᵢᵢ`: the gain the member sees when the other loops are closed.
    pub effective_gain: f64,
    /// The time constant of the member's own path in seconds (N5.15).
    pub time_constant_seconds: f64,
    /// The delay of the member's own path in seconds (N5.15).
    pub delay_seconds: f64,
    /// The member's SIMC PI, with its derivative gain zero.
    pub gains: ParallelGains,
}

/// A decentralized design (C7): the steady-state gain matrix, its relative gain array, and each member, in the pairs' order.
#[derive(Debug, Clone, PartialEq)]
pub struct DecentralizedPiDesign {
    /// The steady-state gains, by output (row) and input (column), in the pairs' order.
    pub gain_matrix: DenseMatrix,
    /// The relative gain array of that matrix.
    pub relative_gains: DenseMatrix,
    /// Each member's design, in the pairs' order.
    pub members: Vec<DecentralizedMember>,
}

/// A member of a PID group whose decoupler is designed (C7): its instance name and its channels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecouplerMember {
    /// The member's instance name.
    pub instance: String,
    /// The identifier of the member's input channel.
    pub input_channel_id: String,
    /// The identifier of the member's output channel.
    pub output_channel_id: String,
}

/// The relative determinant that a two-member decoupler's `Control.StaticDecoupler2x2` must exceed to be configured (spec 37 §4.2):
/// the block's `SingularityTolerance`, which Tune checks with this value. The numerics annex names it (N10).
pub const BLOCK_SINGULARITY_TOLERANCE: f64 = 1e-10;

/// The two-member decoupler as `Control.StaticDecoupler2x2` is configured (C7; spec 37 §4.2): the gain matrix `G = diag(K)⁻¹·K`, its
/// determinant and relative determinant, and its inverse, which equals `D = K⁻¹·diag(K)`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaticDecoupler2x2Design {
    /// `G`, by output (row) and member input (column): `G[i][j] = Kᵢⱼ / Kᵢᵢ`, so its diagonal is one.
    pub gain: [[f64; 2]; 2],
    /// The determinant of `G`.
    pub determinant: f64,
    /// `|det G|` over the block's matrix scale: the block's `RelativeDeterminant`.
    pub relative_determinant: f64,
    /// `G⁻¹` by the block's rule (`Inverse11` … `Inverse22`); its rows give `u = G⁻¹·v`.
    pub inverse: [[f64; 2]; 2],
}

/// A decoupled group (C7): the decoupler by plant input (row) and member (column), and each member's design through it.
#[derive(Debug, Clone, PartialEq)]
pub struct DecouplerDesign {
    /// The decoupler `D = K⁻¹·diag(K)`, by the plant input (row) and the member (column).
    pub matrix: DenseMatrix,
    /// The members' input channel identifiers, in order.
    pub input_ids: Vec<String>,
    /// The members' instance names, in order.
    pub member_ids: Vec<String>,
    /// Each member's design through the decoupler, in order.
    pub members: Vec<DecentralizedMember>,
    /// The two-member group's `Control.StaticDecoupler2x2` configuration (C7), accepted by the block's singularity check; `None` for a
    /// larger group, which uses the program's own variables only.
    pub block: Option<StaticDecoupler2x2Design>,
}

/// Configures a two-member decoupler as `Control.StaticDecoupler2x2` does (C7; spec 37 §4.2): `G = diag(K)⁻¹·K`, with the block's
/// determinant, relative determinant and inverse. A `G` whose relative determinant is not above [`BLOCK_SINGULARITY_TOLERANCE`] is
/// refused with the block's reason, as the block would refuse its configuration.
///
/// # Errors
/// Returns a refusal when a steady-state gain on the diagonal is zero, or when the block's singularity check refuses `G`.
pub fn static_decoupler_2x2(gains: &DenseMatrix) -> Result<StaticDecoupler2x2Design, Refusal> {
    let (k11, k12, k21, k22) = (gains[0][0], gains[0][1], gains[1][0], gains[1][1]);
    if k11 == 0.0 || k22 == 0.0 {
        return Err(Refusal::new(texts::DECOUPLER_GAINS_SINGULAR));
    }
    let gain = [[1.0, k12 / k11], [k21 / k22, 1.0]];
    let determinant = gain[0][0] * gain[1][1] - gain[0][1] * gain[1][0];
    let row_norm = (gain[0][0].abs() + gain[0][1].abs()).max(gain[1][0].abs() + gain[1][1].abs());
    let column_norm =
        (gain[0][0].abs() + gain[1][0].abs()).max(gain[0][1].abs() + gain[1][1].abs());
    let scale = row_norm * column_norm;
    let relative_determinant = determinant.abs() / scale;
    if !(scale > 0.0 && relative_determinant > BLOCK_SINGULARITY_TOLERANCE) {
        return Err(Refusal::new(block_refusal(relative_determinant)));
    }
    let inverse = [
        [gain[1][1] / determinant, -gain[0][1] / determinant],
        [-gain[1][0] / determinant, gain[0][0] / determinant],
    ];
    Ok(StaticDecoupler2x2Design {
        gain,
        determinant,
        relative_determinant,
        inverse,
    })
}

/// The block's reason for refusing a configuration whose relative determinant is not above its singularity tolerance.
fn block_refusal(relative_determinant: f64) -> String {
    format!(
        "Control.StaticDecoupler2x2 refuses the members' gain matrix G = diag(K)⁻¹·K: its relative determinant {} is not above its singularity tolerance {}.",
        significant(relative_determinant),
        significant(BLOCK_SINGULARITY_TOLERANCE)
    )
}

/// A number in two significant digits, for a sentence.
fn significant(value: f64) -> String {
    format!("{value:.2e}")
}

/// Designs each member's PI by SIMC on its own path, for the gain it sees when the other loops are closed (C7, steps 1 to 3).
///
/// # Errors
/// Returns a refusal when the pairs are fewer than two or do not pair each input and each output once, when the members' steady-state
/// gains are singular, when a diagonal relative gain is not positive (naming the pairing to use), or when a member's path or formula is
/// refused.
pub fn design_decentralized_pi(
    model: &GeneralModel,
    pairs: &[DecentralizedPair],
    objective: TuningObjective,
    action: ControllerAction,
) -> Result<DecentralizedPiDesign, Refusal> {
    let plant = delayed_plant(model)?;
    let identity = model.identity();
    let inputs: Option<Vec<usize>> = pairs
        .iter()
        .map(|pair| index_of(&identity.inputs, &pair.input_id))
        .collect();
    let outputs: Option<Vec<usize>> = pairs
        .iter()
        .map(|pair| index_of(&identity.outputs, &pair.output_id))
        .collect();
    let (Some(inputs), Some(outputs)) = (inputs, outputs) else {
        return Err(Refusal::new(texts::PAIR_EACH_MEMBER));
    };
    validate_pairs(pairs.len(), &inputs, &outputs)?;
    design_members(&plant, pairs, &inputs, &outputs, objective, action)
}

/// Designs the static decoupler `D = K⁻¹·diag(K)` of a PID group and the members' PIs through it (C7). The members' PIs are designed on
/// the model seen through the decoupler, where the plant input `u = D·v` takes the members' outputs `v`, so their relative gains are one.
///
/// # Errors
/// Returns a refusal when the group has no members, when its model is a transfer function, when a member's channels are not in the model,
/// when the group's steady-state gains are singular, or when the members' designs refuse (see [`design_decentralized_pi`]).
pub fn design_decoupled_group(
    model: &GeneralModel,
    members: &[DecouplerMember],
    objective: Option<TuningObjective>,
    action: ControllerAction,
) -> Result<DecouplerDesign, Refusal> {
    if members.is_empty() {
        return Err(Refusal::new(texts::DECOUPLER_MEMBERS));
    }
    if matches!(model, GeneralModel::TransferFunction { .. }) {
        return Err(Refusal::new(texts::DECOUPLER_STATE_MODEL));
    }
    let identity = model.identity();
    let inputs: Option<Vec<usize>> = members
        .iter()
        .map(|member| index_of(&identity.inputs, &member.input_channel_id))
        .collect();
    let outputs: Option<Vec<usize>> = members
        .iter()
        .map(|member| index_of(&identity.outputs, &member.output_channel_id))
        .collect();
    let (Some(inputs), Some(outputs)) = (inputs, outputs) else {
        return Err(Refusal::new(texts::DECOUPLER_CHANNELS));
    };
    let plant = delayed_plant(model)?;
    let discrete = matches!(plant.timing, Timing::Discrete { .. });
    let steady = steady_state_gain(&plant.system, discrete)
        .ok_or_else(|| Refusal::new(texts::DECOUPLER_STATE_SINGULAR))?;
    let gains: DenseMatrix = outputs
        .iter()
        .map(|&output| inputs.iter().map(|&input| steady[output][input]).collect())
        .collect();
    let matrix = static_decoupler(&gains, &TOLERANCE)
        .map_err(|_| Refusal::new(texts::DECOUPLER_GAINS_SINGULAR))?;
    let block = match gains.as_slice() {
        [_, _] => Some(static_decoupler_2x2(&gains)?),
        _ => None,
    };

    // The model seen through the decoupler: the members' inputs are the PIs' outputs, and the plant sees u = D·v.
    let decoupled = DelayedSystem {
        system: StateSpace {
            a: plant.system.a.clone(),
            b: mix_inputs(&plant.system.b, &inputs, &matrix),
            c: plant.system.c.clone(),
            d: mix_inputs(&plant.system.d, &inputs, &matrix),
        },
        timing: plant.timing,
        delay_seconds: plant.delay_seconds,
    };
    let pairs: Vec<DecentralizedPair> = members
        .iter()
        .map(|member| DecentralizedPair {
            input_id: member.input_channel_id.clone(),
            output_id: member.output_channel_id.clone(),
        })
        .collect();
    validate_pairs(pairs.len(), &inputs, &outputs)?;
    let design = design_members(
        &decoupled,
        &pairs,
        &inputs,
        &outputs,
        objective.unwrap_or(AUTOMATIC_OBJECTIVE),
        action,
    )?;
    Ok(DecouplerDesign {
        matrix,
        input_ids: members
            .iter()
            .map(|member| member.input_channel_id.clone())
            .collect(),
        member_ids: members
            .iter()
            .map(|member| member.instance.clone())
            .collect(),
        members: design.members,
        block,
    })
}

/// The decentralized design of the members on a plant whose members' inputs and outputs are given by index (C7).
fn design_members(
    plant: &DelayedSystem,
    pairs: &[DecentralizedPair],
    inputs: &[usize],
    outputs: &[usize],
    objective: TuningObjective,
    action: ControllerAction,
) -> Result<DecentralizedPiDesign, Refusal> {
    let responses = inputs
        .iter()
        .map(|&input| open_loop_step_response(plant, input, "group member"))
        .collect::<Result<Vec<StepResponse>, Refusal>>()?;
    let gain_matrix: DenseMatrix = outputs
        .iter()
        .map(|&output| {
            responses
                .iter()
                .map(|response| response.finals[output])
                .collect()
        })
        .collect();
    let relative_gains = relative_gain_array(&gain_matrix, &TOLERANCE)
        .map_err(|_| Refusal::new(texts::GROUP_SINGULAR_OUTPUTS))?;
    let conflicting = (0..pairs.len()).find(|&index| {
        let value = relative_gains[index][index];
        value.is_nan() || value <= 0.0
    });
    if let Some(conflicting) = conflicting {
        return Err(Refusal::new(conflict_sentence(
            pairs,
            &relative_gains,
            conflicting,
        )));
    }
    let members = pairs
        .iter()
        .enumerate()
        .map(|(index, pair)| -> Result<DecentralizedMember, Refusal> {
            let response = &responses[index];
            let path = two_point_path(
                &response.times,
                &response.outputs[outputs[index]],
                "group member",
            )?;
            // Tune checks the signed effective gain against the action (N6.3) and returns the signed Kp (N6.4).
            let effective_gain = gain_matrix[index][index] / relative_gains[index][index];
            let tuning = tune_pi(
                &TuningPlant::FirstOrder(FirstOrderPlusDelay {
                    gain: effective_gain,
                    time_constant_seconds: path.time_constant_seconds,
                    delay_seconds: path.delay_seconds,
                }),
                TuningMethod::Simc,
                objective,
                action,
                "",
            )?;
            Ok(DecentralizedMember {
                pair: pair.clone(),
                path_gain: gain_matrix[index][index],
                relative_gain: relative_gains[index][index],
                effective_gain,
                time_constant_seconds: path.time_constant_seconds,
                delay_seconds: path.delay_seconds,
                gains: tuning.gains(),
            })
        })
        .collect::<Result<Vec<DecentralizedMember>, Refusal>>()?;
    Ok(DecentralizedPiDesign {
        gain_matrix,
        relative_gains,
        members,
    })
}

/// The sentence of a conflicting diagonal relative gain (N4.5): the pair and its relative gain, and the pairing to use when every
/// pairing has positive relative gains, or the sentence that none has.
fn conflict_sentence(
    pairs: &[DecentralizedPair],
    relative_gains: &DenseMatrix,
    conflicting: usize,
) -> String {
    // The pairing search covers at most eight pairs (N4.5); a larger group names no pairing, and the declared one stands refused.
    if pairs.len() > MAXIMUM_SEARCHED_PAIRS {
        return texts::relative_gain_conflict(
            &pairs[conflicting].input_id,
            &pairs[conflicting].output_id,
            relative_gains[conflicting][conflicting],
            texts::PAIRING_LIMIT,
        );
    }
    let pairing = pairing_by_relative_gain(relative_gains);
    let every_positive = pairing
        .iter()
        .enumerate()
        .all(|(row, &column)| relative_gains[row][column] > 0.0);
    let better = if every_positive {
        let mut chosen: Vec<(&str, &str)> = pairing
            .iter()
            .enumerate()
            .map(|(row, &column)| {
                (
                    pairs[column].input_id.as_str(),
                    pairs[row].output_id.as_str(),
                )
            })
            .collect();
        chosen.sort_by(|left, right| left.0.cmp(right.0));
        let list = chosen
            .iter()
            .map(|(input, output)| format!("{input} → {output}"))
            .collect::<Vec<String>>()
            .join(" and ");
        texts::pairing_instead(&list)
    } else {
        texts::NO_POSITIVE_PAIRING.to_owned()
    };
    texts::relative_gain_conflict(
        &pairs[conflicting].input_id,
        &pairs[conflicting].output_id,
        relative_gains[conflicting][conflicting],
        &better,
    )
}

/// The rows of a matrix over the model's inputs, with each member's input column replaced by the plant's input through the decoupler:
/// `Σₖ row[inputₖ]·D[k][member]`, where `D` is the decoupler (C7). The other columns are kept.
fn mix_inputs(rows: &DenseMatrix, inputs: &[usize], matrix: &DenseMatrix) -> DenseMatrix {
    rows.iter()
        .map(|row| {
            (0..row.len())
                .map(
                    |column| match inputs.iter().position(|&input| input == column) {
                        Some(member) => {
                            inputs.iter().enumerate().fold(0.0, |sum, (index, &input)| {
                                sum + row[input] * matrix[index][member]
                            })
                        }
                        None => row[column],
                    },
                )
                .collect()
        })
        .collect()
}

/// The index of the channel with an identifier, if the model has it.
fn index_of(channels: &[ModelChannel], id: &str) -> Option<usize> {
    channels.iter().position(|channel| channel.id == id)
}

/// A group needs two or more pairs, and each input and each output may appear in one pair only (C7).
fn validate_pairs(count: usize, inputs: &[usize], outputs: &[usize]) -> Result<(), Refusal> {
    if count < 2 || !all_distinct(inputs) || !all_distinct(outputs) {
        return Err(Refusal::new(texts::PAIR_EACH_MEMBER));
    }
    Ok(())
}

/// Whether no index appears twice.
fn all_distinct(indices: &[usize]) -> bool {
    let mut sorted = indices.to_vec();
    sorted.sort_unstable();
    sorted.windows(2).all(|pair| pair[0] != pair[1])
}

#[cfg(test)]
#[path = "pid_group_tests.rs"]
mod tests;

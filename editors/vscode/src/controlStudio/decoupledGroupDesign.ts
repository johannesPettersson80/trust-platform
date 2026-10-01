import { designDecentralizedPi, type DecentralizedPiDesign } from "./decentralizedPidDesign";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import type { GeneralModel, GeneralStateSpaceModel } from "./generalModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import { invertMatrix } from "./numerics";
import { solveLinear } from "./controllerInitialState";

/** A PID group whose program applies u = D·v in Auto, with D a matrix of program variables (Specification 35 §17.15). */
export interface DecoupledPidSeed { kind: "decoupled-pid"; family: "PI"; matrixSymbols: readonly (readonly string[])[] }

/** What Tune shows: D by plant input and member, and each member's design through it. */
export interface DecouplerDesign { matrix: number[][]; inputIds: string[]; memberIds: string[]; members: DecentralizedPiDesign["members"] }

const tolerance = { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e12 };

/** D = K⁻¹·diag(K): through it each member moves only its own measurement in steady state, with its own path's gain. */
export function decoupledGroupDesign(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  seed: DecoupledPidSeed): DecouplerDesign & { values: Record<string, number> } {
  const members = loop.controller.group?.members;
  if (!members?.length || seed.matrixSymbols.length !== members.length) throw new Error("A decoupler needs one row and one column per group member.");
  if (model.kind === "transfer-function") throw new Error("A decoupler is designed on a state-space group model.");
  const inputs = members.map(member => model.inputs.findIndex(input => input.id === member.inputChannelId));
  const outputs = members.map(member => model.outputs.findIndex(output => output.id === member.outputChannelId));
  if (inputs.some(index => index < 0) || outputs.some(index => index < 0)) throw new Error("The model's channels do not match the group's members.");
  const steady = steadyStateGains(model);
  const gains = outputs.map(output => inputs.map(input => steady[output][input]));
  const inverse = invertMatrix(gains, tolerance) as { status: string; inverse?: number[][] };
  if (inverse.status !== "qualified" || !inverse.inverse) throw new Error("The members' steady-state gains are singular: no decoupler can separate them.");
  const matrix = inverse.inverse.map(row => row.map((value, column) => value * gains[column][column]));
  // The model seen through the decoupler: its members' inputs are the PIDs' outputs v, the plant sees u = D·v.
  const through = model.inputs.map((_, input) => {
    const member = inputs.indexOf(input);
    return member < 0 ? undefined : member;
  });
  const mix = (rows: readonly (readonly number[])[]) => rows.map(row => model.inputs.map((_, column) => {
    const member = through[column];
    return member === undefined ? row[column] : inputs.reduce((sum, input, index) => sum + row[input] * matrix[index][member], 0);
  }));
  const decoupled = { ...model, revision: `${model.revision}:decoupled`, b: mix(model.b), d: mix(model.d) } as GeneralModel;
  const design = designDecentralizedPi(decoupled, members.map(member => ({ inputId: member.inputChannelId, outputId: member.outputChannelId })),
    task.tuningObjective ?? "balanced");
  const values: Record<string, number> = {};
  members.forEach((member, index) => {
    const field = (name: string) => {
      const symbol = member.parameterSymbols.find(candidate => candidate.endsWith(`.${name}`));
      if (!symbol) throw new Error(`${member.instance} binds no ${name}.`);
      return symbol;
    };
    values[field("ProportionalGain")] = design.members[index].proportionalGain;
    values[field("IntegralGain")] = design.members[index].integralGain;
    values[field("DerivativeGain")] = 0;
  });
  seed.matrixSymbols.forEach((row, i) => row.forEach((symbol, j) => { values[symbol] = matrix[i][j]; }));
  return { matrix, inputIds: members.map(member => member.inputChannelId), memberIds: members.map(member => member.instance), members: design.members, values };
}

/** Exact steady-state gains: −C·A⁻¹·B + D for a continuous model, C·(I − A)⁻¹·B + D for a sampled one. */
function steadyStateGains(model: GeneralStateSpaceModel): number[][] {
  const discrete = model.kind === "discrete-state-space";
  const system = model.a.map((row, i) => row.map((value, j) => discrete ? Number(i === j) - value : -value));
  const states = model.inputs.map((_, input) => solveLinear(system, model.b.map(row => row[input])));
  return model.outputs.map((_, output) => model.inputs.map((_, input) =>
    model.c[output].reduce((sum, weight, state) => sum + weight * states[input][state], 0) + model.d[output][input]));
}

import type { GeneralStateSpaceModel } from "../generalModel";
import type { GeneralStateStructure } from "../generalStateStructure";
import { formatMatrixRows } from "./numberListText";

export interface StateStructureDetails { summary: string; order: string; matrices: [string, string][]; scope: string }

/** The state-space structure as the Tune analysis shows it: both ranks out of the state count, the declared orders,
 * the model and structure matrices, and what the ranks do not establish. */
export function stateStructureDetails(model: GeneralStateSpaceModel, structure: GeneralStateStructure): StateStructureDetails {
  const channels = (list: GeneralStateSpaceModel["states"]) => list.map(channel => `${channel.id} (${channel.unit})`).join(", ");
  const order = `State order: ${channels(model.states)}. Input order: ${channels(model.inputs)}. Output order: ${channels(model.outputs)}.`;
  if (structure.status === "unavailable") return { summary: "Structural analysis unavailable.", order, matrices: [], scope: structure.reason };
  const states = model.states.length;
  return {
    summary: `Controllability rank ${structure.controllability.rank} of ${states}; observability rank ${structure.observability.rank} of ${states}.`,
    order,
    matrices: [["A", formatMatrixRows(model.a)], ["B", formatMatrixRows(model.b)], ["C", formatMatrixRows(model.c)], ["D", formatMatrixRows(model.d)],
      ["Controllability [B, AB, …]", formatMatrixRows(structure.controllability.matrix)], ["Observability [C; CA; …]", formatMatrixRows(structure.observability.matrix)]],
    scope: structure.scope,
  };
}

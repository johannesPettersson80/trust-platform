import type { ControlLoopDefinition } from "./loopDefinition";
import { ParameterApplicationClient } from "./parameterApplication";
import type { ParameterConnection } from "./parameterSession";
import { bindRuntimeConnection } from "./runtimeConnection";

export async function connectRuntimeParameters(loop: ControlLoopDefinition): Promise<ParameterConnection> {
  if (!loop.parameterBinding) throw new Error("Declare the ST parameter binding before application.");
  const connection = await bindRuntimeConnection(loop.runtimeId);
  return { source: connection.source, client: new ParameterApplicationClient(loop.parameterBinding, connection.source, connection.request) };
}

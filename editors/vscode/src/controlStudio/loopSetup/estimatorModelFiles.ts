import * as fs from "fs";
import * as path from "path";
import { decodeGeneralModel } from "../generalModel";

/**
 * The project's model files an LQG estimator can implement (Specification 35 §17.1): state-space models with one input,
 * one output and at most three states, in the project folder or its `models` folder.
 */
export async function estimatorModelFiles(projectRoot: string): Promise<{ file: string; states: number }[]> {
  const found: { file: string; states: number }[] = [];
  for (const folder of ["", "models"]) {
    const directory = path.join(projectRoot, folder);
    const entries = await fs.promises.readdir(directory, { withFileTypes: true }).catch(() => []);
    for (const entry of entries.filter(entry => entry.isFile() && entry.name.toLowerCase().endsWith(".json")).sort((a, b) => a.name.localeCompare(b.name))) {
      let model;
      try { model = decodeGeneralModel(JSON.parse(await fs.promises.readFile(path.join(directory, entry.name), "utf8"))); } catch { continue; }
      if (!model || model.kind === "transfer-function" || model.inputs.length !== 1 || model.outputs.length !== 1 || model.states.length > 3) continue;
      found.push({ file: folder ? `${folder}/${entry.name}` : entry.name, states: model.states.length });
    }
  }
  return found;
}

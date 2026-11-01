import type { EquipmentDetails } from "../hmiBuilder/editor/equipmentDetails";
import { hardwareWords } from "../hmiBuilder/editor/hardwareStatus";

/** The connection summary of an instance, as the language server judges it. */
export function equipmentConnectionSummary(details: EquipmentDetails): string {
  return hardwareWords(details.hardware);
}

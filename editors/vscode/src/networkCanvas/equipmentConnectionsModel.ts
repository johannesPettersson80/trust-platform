import type { EquipmentDetails } from "../hmiBuilder/editor/equipmentDetails";
import { hardwareWords } from "../hmiBuilder/editor/hardwareStatus";

/** The connection summary of an instance, as the language server judges it. */
export function equipmentConnectionSummary(details: EquipmentDetails): string {
  return hardwareWords(details.hardware);
}

/** What to do when no PLC signal fits a terminal: declare one with an address in the program, as
 * an I/O list does. */
export function missingSignalHelp(
  instance: string,
  terminal: { member: string; direction: "input" | "output"; type?: string | null },
): string {
  const type = terminal.type ?? "BOOL";
  const area = terminal.direction === "input" ? "%I" : "%Q";
  // The address's size follows the type (IEC 61131-3, §6.5.5): X a bit, B a byte, W a word,
  // D a double word, L a long word.
  const size = /^BOOL$/i.test(type)
    ? "X0.0"
    : /^(SINT|USINT|BYTE|CHAR)$/i.test(type)
      ? "B0"
      : /^(INT|UINT|WORD)$/i.test(type)
        ? "W0"
        : /^(DINT|UDINT|DWORD|REAL)$/i.test(type)
          ? "D0"
          : "L0";
  const address = `${area}${size}`;
  const name = `${instance.split(".").pop()}_${terminal.member}`;
  return `No ${terminal.direction} of type ${type} is declared yet. Declare one with an address in your program, for example ${name} AT ${address} : ${type};, then press Refresh.`;
}
